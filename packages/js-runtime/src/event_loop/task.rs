use rquickjs::{Function, Persistent};

#[allow(dead_code)]
pub struct RafTask {
    pub id: u32,
    pub func: Persistent<Function<'static>>,
}
