pub mod registry;

pub use registry::ExtensionRegistry;

use rquickjs::{Ctx, Result};

pub trait Extension {
    fn name(&self) -> &'static str;

    /// Register globals onto globalThis (flat or namespaced).
    fn globals<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }

    /// Return a module name if this extension should be importable as an ES module.
    /// When `Some`, the module loader will invoke `module_init` during module evaluation.
    fn module_name(&self) -> Option<&'static str> {
        None
    }

    /// Populate module exports. Signature will be refined when built-in module
    /// loader integration is implemented.
    fn module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }
}
