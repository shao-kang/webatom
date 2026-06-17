#[derive(Clone, Debug)]
pub enum BlitzEvent {
    Click   { node_id: usize, x: f32, y: f32 },
    KeyDown { key: String, modifiers: u32 },
    KeyUp   { key: String, modifiers: u32 },
    Resize  { width: u32, height: u32 },
    Focus   { node_id: usize },
    Blur    { node_id: usize },
}
