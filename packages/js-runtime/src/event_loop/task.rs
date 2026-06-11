use std::time::Duration;

use rquickjs::{Function, Persistent};

pub(crate) enum MacroTask {
    Timer {
        id: u32,
        func: Persistent<Function<'static>>,
        interval: Option<Duration>,
    },
}
