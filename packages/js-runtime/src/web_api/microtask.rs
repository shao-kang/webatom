use rquickjs::Ctx;

use crate::event_loop::HostBridge;
use crate::extension::Extension;

pub struct MicrotaskExtension;

impl Extension for MicrotaskExtension {
    fn name(&self) -> &'static str {
        "queue-microtask"
    }

    fn install(&self, _ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        // Routes queueMicrotask into the QJS Promise job queue, ensuring
        // FIFO ordering with Promise.then callbacks.
        Some("globalThis.queueMicrotask = (cb) => { Promise.resolve().then(cb); };")
    }
}
