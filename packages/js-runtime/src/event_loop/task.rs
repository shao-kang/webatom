use rquickjs::{Function, Persistent};

pub struct MacroTask {
    pub func: Persistent<Function<'static>>,
}

#[allow(dead_code)]
pub(crate) struct RafTask {
    pub id: u32,
    pub func: Persistent<Function<'static>>,
}
