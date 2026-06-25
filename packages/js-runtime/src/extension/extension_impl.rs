use rquickjs::Ctx;

use crate::event_loop::HostBridge;

pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    /// Native module specifiers this extension declares (e.g. `["@webatom/console"]`).
    /// The resolver uses this list to recognise them without hitting the filesystem.
    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &[]
    }

    /// Register native modules and context userdata.
    /// Called once per context, before any `js_glue` modules run.
    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()>;

    /// Optional ES module source injected after all `install` calls.
    /// Use this to bind globals: `import * as _x from '@webatom/x'; globalThis.x = _x;`
    fn js_glue(&self) -> Option<&'static str> {
        None
    }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
