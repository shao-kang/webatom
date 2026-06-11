use std::collections::HashMap;

use rquickjs::{CatchResultExt, Context, Module, Runtime};

use crate::event_loop::EventLoop;
use crate::extension::{Extension, ExtensionRegistry};
use crate::module::setup_module_system;
use crate::web_api::{ConsoleExtension, TimerExtension};

pub struct JsRuntime {
    // Field order = drop order. event_loop (which holds Runtime) must be dropped last.
    context: Context,
    event_loop: EventLoop,
}

pub struct JsRuntimeBuilder {
    import_map: HashMap<String, String>,
    registry: ExtensionRegistry,
    with_default_extensions: bool,
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            import_map: HashMap::new(),
            registry: ExtensionRegistry::new(),
            with_default_extensions: true,
        }
    }

    /// Disable the built-in console + timer extensions (for custom setups).
    pub fn no_defaults(mut self) -> Self {
        self.with_default_extensions = false;
        self
    }

    pub fn import_map(mut self, map: HashMap<String, String>) -> Self {
        self.import_map = map;
        self
    }

    pub fn extension(mut self, ext: impl Extension + 'static) -> Self {
        self.registry.register(ext);
        self
    }

    pub fn build(mut self) -> rquickjs::Result<JsRuntime> {
        let runtime = Runtime::new()?;
        setup_module_system(&runtime, self.import_map);

        let event_loop = EventLoop::new(runtime);
        let context = Context::full(event_loop.runtime())?;

        // Register default extensions with access to internal handles.
        if self.with_default_extensions {
            self.registry.register(ConsoleExtension);
            self.registry.register(TimerExtension::new(event_loop.timers()));
        }

        // Apply all extensions once at build time.
        context.with(|ctx| self.registry.apply(&ctx))?;

        Ok(JsRuntime {
            context,
            event_loop,
        })
    }
}

impl JsRuntime {
    pub fn builder() -> JsRuntimeBuilder {
        JsRuntimeBuilder::new()
    }

    /// Evaluate an ES module. Can be called multiple times.
    pub fn eval_module(self, name: &str, source: impl Into<Vec<u8>>) -> rquickjs::Result<Self> {
        self.context.with(|ctx| {
            Module::evaluate(ctx.clone(), name, source)
                .catch(&ctx)
                .map_err(|e| e.throw(&ctx))?
                .finish::<()>()
                .catch(&ctx)
                .map_err(|e| e.throw(&ctx))
        })?;
        Ok(self)
    }

    /// Evaluate a JS expression and return the result.
    pub fn eval<T>(&self, source: &str) -> rquickjs::Result<T>
    where
        T: for<'js> rquickjs::FromJs<'js>,
    {
        self.context.with(|ctx| {
            ctx.eval::<T, _>(source)
                .catch(&ctx)
                .map_err(|e| e.throw(&ctx))
        })
    }

    /// Drive the event loop until all pending work is done.
    pub async fn run(mut self) -> rquickjs::Result<()> {
        self.event_loop.run(&self.context).await
    }
}

