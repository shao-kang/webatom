use std::collections::HashMap;

use anyrender_vello::VelloWindowRenderer;
use blitz_shell::{BlitzApplication, BlitzShellEvent, BlitzShellProxy, WindowConfig};
use webatom_blitz_msg::{BlitzSide, DomMsg};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::renderer;

pub struct WebAtomApp {
    inner: BlitzApplication<VelloWindowRenderer>,
    blitz_side: BlitzSide,
    id_map: HashMap<usize, usize>,
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
            id_map: HashMap::new(),
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
        self.inner.window_event(event_loop, window_id, event);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Drain crossbeam dom_rx, apply updates to blitz document
        let mut latest_full: Option<webatom_blitz_msg::DomSnapshot> = None;
        let mut patches: Vec<webatom_blitz_msg::DomOp> = Vec::new();

        while let Ok(msg) = self.blitz_side.dom_rx.try_recv() {
            match msg {
                DomMsg::Full(snap) => {
                    latest_full = Some(snap);
                    patches.clear();
                }
                DomMsg::Patch(ops) => {
                    patches.extend(ops);
                }
            }
        }

        let has_updates = latest_full.is_some() || !patches.is_empty();

        if has_updates {
            for window in self.inner.windows.values_mut() {
                let mut doc = window.doc.inner_mut();
                if let Some(snap) = &latest_full {
                    renderer::apply_full(&mut *doc, &mut self.id_map, snap);
                }
                if !patches.is_empty() {
                    renderer::apply_patch(&mut *doc, &mut self.id_map, &patches);
                }
                drop(doc);
                window.request_redraw();
            }
        }

        self.inner.proxy_wake_up(event_loop);
    }
}
