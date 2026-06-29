#[derive(Clone, Debug)]
pub enum Event {
    Click     { node_id: usize, x: f32, y: f32 },
    KeyDown   { key: String, modifiers: u32 },
    KeyUp     { key: String, modifiers: u32 },
    Resize    { width: u32, height: u32 },
    Focus     { node_id: usize },
    Blur      { node_id: usize },
    MouseMove { x: f32, y: f32 },
    MouseDown { node_id: usize, x: f32, y: f32, button: u8 },
    MouseUp   { node_id: usize, x: f32, y: f32, button: u8 },
    DblClick  { node_id: usize, x: f32, y: f32 },
    Scroll    { delta_x: f32, delta_y: f32 },
}
