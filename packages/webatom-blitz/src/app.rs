use anyrender_vello::VelloWindowRenderer;
use blitz_shell::{BlitzApplication, BlitzShellEvent, BlitzShellProxy, WindowConfig};
use webatom_blitz_msg::{BlitzSide, Event};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::node_id_map::NodeIdMap;
use crate::renderer;

pub struct WebAtomApp {
    inner: BlitzApplication<VelloWindowRenderer>,
    blitz_side: BlitzSide,
    id_map: NodeIdMap,
    cursor_pos: (f32, f32),
    /// Position and time of the most recent left-button press (for click detection).
    mouse_press: Option<(f32, f32, std::time::Instant)>,
    /// Time of the most recent confirmed click (for double-click detection).
    last_click_time: Option<std::time::Instant>,
}

impl WebAtomApp {
    pub fn new(
        proxy: BlitzShellProxy,
        event_queue: std::sync::mpsc::Receiver<BlitzShellEvent>,
        blitz_side: BlitzSide,
    ) -> Self {
        Self {
            inner: BlitzApplication::new(proxy, event_queue),
            blitz_side,
            id_map: NodeIdMap::new(),
            cursor_pos: (0.0, 0.0),
            mouse_press: None,
            last_click_time: None,
        }
    }

    pub fn add_window(&mut self, config: WindowConfig<VelloWindowRenderer>) {
        self.inner.add_window(config);
    }
}

impl ApplicationHandler for WebAtomApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.can_create_surfaces(event_loop);
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event: ref key_event, .. } => {
                use winit::event::ElementState;
                use winit::keyboard::{Key, NamedKey};

                let key_str: String = match &key_event.logical_key {
                    Key::Named(n) => match n {
                        NamedKey::Enter      => "Enter",
                        NamedKey::Tab        => "Tab",
                        NamedKey::Escape     => "Escape",
                        NamedKey::Backspace  => "Backspace",
                        NamedKey::Delete     => "Delete",
                        NamedKey::ArrowLeft  => "ArrowLeft",
                        NamedKey::ArrowRight => "ArrowRight",
                        NamedKey::ArrowUp    => "ArrowUp",
                        NamedKey::ArrowDown  => "ArrowDown",
                        NamedKey::Home       => "Home",
                        NamedKey::End        => "End",
                        NamedKey::PageUp     => "PageUp",
                        NamedKey::PageDown   => "PageDown",
                        NamedKey::Insert     => "Insert",
                        NamedKey::F1  => "F1",  NamedKey::F2  => "F2",
                        NamedKey::F3  => "F3",  NamedKey::F4  => "F4",
                        NamedKey::F5  => "F5",  NamedKey::F6  => "F6",
                        NamedKey::F7  => "F7",  NamedKey::F8  => "F8",
                        NamedKey::F9  => "F9",  NamedKey::F10 => "F10",
                        NamedKey::F11 => "F11", NamedKey::F12 => "F12",
                        _ => "",
                    }.to_owned(),
                    Key::Character(c) => c.to_string(),
                    _ => String::new(),
                };

                if !key_str.is_empty() {
                    let evt = if key_event.state == ElementState::Pressed {
                        Event::KeyDown { key: key_str, modifiers: 0 }
                    } else {
                        Event::KeyUp { key: key_str, modifiers: 0 }
                    };
                    self.blitz_side.send_event(evt);
                }
            }
            WindowEvent::SurfaceResized(ref size) => {
                self.blitz_side.send_event(Event::Resize {
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::PointerMoved { position: ref pos, .. } => {
                let (cx, cy) = self.inner.windows.values_mut()
                    .next()
                    .map(|w| { let c = w.pointer_coords(*pos); (c.client_x, c.client_y) })
                    .unwrap_or((pos.x as f32, pos.y as f32));
                self.cursor_pos = (cx, cy);
                self.blitz_side.send_event(Event::MouseMove { x: cx, y: cy });
            }
            WindowEvent::PointerButton { ref button, ref state, ref position, .. } => {
                use std::time::{Duration, Instant};
                use winit::event::{ButtonSource, ElementState, MouseButton};

                const CLICK_DIST: f32 = 5.0;
                const CLICK_MS: Duration = Duration::from_millis(300);
                const DBLCLICK_MS: Duration = Duration::from_millis(300);

                let button_id: u8 = match button {
                    ButtonSource::Mouse(MouseButton::Left)   => 0,
                    ButtonSource::Mouse(MouseButton::Right)  => 1,
                    ButtonSource::Mouse(MouseButton::Middle) => 2,
                    _                                        => 3,
                };
                let (x, y, blitz_node) = self.inner.windows.values_mut()
                    .next()
                    .map(|w| {
                        let c = w.pointer_coords(*position);
                        let hit = w.doc.inner().hit(c.page_x, c.page_y);
                        (c.client_x, c.client_y, hit.map(|h| h.node_id))
                    })
                    .unwrap_or((position.x as f32, position.y as f32, None));
                self.cursor_pos = (x, y);
                let target_id = blitz_node
                    .and_then(|id| self.id_map.js_id(id))
                    .unwrap_or(0);

                if *state == ElementState::Pressed {
                    self.blitz_side.send_event(
                        Event::MouseDown { node_id: target_id, x, y, button: button_id },
                    );
                    if button_id == 0 {
                        self.mouse_press = Some((x, y, Instant::now()));
                    }
                } else {
                    self.blitz_side.send_event(
                        Event::MouseUp { node_id: target_id, x, y, button: button_id },
                    );
                    if button_id == 0 {
                        if let Some((px, py, press_time)) = self.mouse_press.take() {
                            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                            if dist <= CLICK_DIST && press_time.elapsed() <= CLICK_MS {
                                let now = Instant::now();
                                let is_dbl = self.last_click_time
                                    .map_or(false, |t| now.duration_since(t) <= DBLCLICK_MS);
                                if is_dbl {
                                    self.last_click_time = None;
                                    self.blitz_side.send_event(
                                        Event::DblClick { node_id: target_id, x, y },
                                    );
                                } else {
                                    self.last_click_time = Some(now);
                                    self.blitz_side.send_event(
                                        Event::Click { node_id: target_id, x, y },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta: ref d, .. } => {
                use winit::event::MouseScrollDelta;
                let (dx, dy) = match d {
                    MouseScrollDelta::LineDelta(x, y) => (*x * 20.0, *y * 20.0),
                    MouseScrollDelta::PixelDelta(p)   => (p.x as f32, p.y as f32),
                };
                self.blitz_side.send_event(Event::Scroll { delta_x: dx, delta_y: dy });
            }
            _ => {}
        }

        self.inner.window_event(event_loop, window_id, event);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(drain) = self.blitz_side.drain_dom_msgs() else {
            self.inner.proxy_wake_up(event_loop);
            return;
        };

        // ── Apply DOM update ──────────────────────────────────────────────
        use webatom_blitz_msg::DomUpdate;
        for window in self.inner.windows.values_mut() {
            let mut doc = window.doc.inner_mut();
            match &drain.dom_update {
                Some(DomUpdate::Full(snap)) => {
                    renderer::apply_full(&mut *doc, &mut self.id_map, snap);
                }
                Some(DomUpdate::Patch(ops)) => {
                    renderer::apply_patch(&mut *doc, &mut self.id_map, ops);
                }
                None => {}
            }
            // DocumentMutator already dropped → layout is flushed

            // ── Reply to layout queries (layout is now valid) ─────────────
            for &wa_id in &drain.layout_queries {
                if let Some(layout) = renderer::query_layout(&*doc, &self.id_map, wa_id) {
                    self.blitz_side.send_event(Event::LayoutResult(layout));
                }
            }

            // ── Fire nextTick callbacks ───────────────────────────────────
            for _ in 0..drain.notify_count {
                self.blitz_side.send_event(Event::LayoutNotify);
            }

            drop(doc);
            window.request_redraw();
        }

        // ── Signal JS that a new frame is ready (skippable) ──────────────
        self.blitz_side.try_send_raf_tick();

        self.inner.proxy_wake_up(event_loop);
    }
}
