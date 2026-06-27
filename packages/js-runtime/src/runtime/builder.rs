use std::collections::HashMap;

use rquickjs::AsyncContext;

use crate::event_loop::EventLoop;
use crate::extension::{Extension, ExtensionRegistry};
use crate::module::setup_module_system;
use crate::web_api::default_extensions;

use super::runtime::JsRuntime;

pub struct JsRuntimeBuilder {
    extensions: Vec<Box<dyn Extension>>,
    with_defaults: bool,
    import_map: HashMap<String, String>,
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            with_defaults: true,
            import_map: HashMap::new(),
        }
    }

    pub fn no_defaults(mut self) -> Self {
        self.with_defaults = false;
        self
    }

    pub fn import_map(mut self, map: HashMap<String, String>) -> Self {
        self.import_map = map;
        self
    }

    pub fn with_extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub async fn build(self) -> rquickjs::Result<JsRuntime> {
        let mut registry = ExtensionRegistry::new();

        if self.with_defaults {
            for ext in default_extensions() {
                registry.register_boxed(ext);
            }
        }
        for ext in self.extensions {
            // Unbox and re-register; Extension is object-safe so we go through the trait.
            registry.register_boxed(ext);
        }

        let runtime = rquickjs::AsyncRuntime::new()?;
        setup_module_system(&runtime, self.import_map).await;

        let event_loop = EventLoop::new(runtime);
        let host = event_loop.host.clone();

        let ctx = AsyncContext::full(event_loop.runtime()).await?;

        let extension_modules = registry.extension_modules.clone();
        ctx.with(|qctx| {
            qctx.store_userdata(extension_modules)?;
            qctx.store_userdata(host.clone())?;
            registry.apply(&qctx, &host)
        })
        .await?;

        Ok(JsRuntime::new(ctx, event_loop))
    }
}
