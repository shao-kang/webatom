use std::collections::HashMap;

use rquickjs::{AsyncContext, AsyncRuntime, CatchResultExt, Module};

use crate::event_loop::EventLoop;
use crate::extension::{Extension, ExtensionRegistry};
use crate::module::setup_module_system;
use crate::web_api::{ConsoleExtension, };

pub struct JsRuntime {
    // Field order = drop order: context must be dropped before the runtime inside event_loop.
    context: AsyncContext,
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

    pub async fn build(mut self) -> rquickjs::Result<JsRuntime> {
        if self.with_default_extensions {
            self.registry.register(ConsoleExtension);
            // self.registry.register(TimerExtension);
        }

        let runtime = AsyncRuntime::new()?;
        setup_module_system(&runtime, self.import_map).await;

        let event_loop = EventLoop::new(runtime);
        let context = AsyncContext::full(event_loop.runtime()).await?;

        let handle = event_loop.handle();
        let extension_modules = self.registry.extension_modules.clone();
        context.with(|ctx| {
            ctx.store_userdata(handle)?;
            ctx.store_userdata(extension_modules)?;
            self.registry.apply(&ctx)
        }).await?;

        Ok(JsRuntime { context, event_loop })
    }
}

impl JsRuntime {
    pub fn builder() -> JsRuntimeBuilder {
        JsRuntimeBuilder::new()
    }

    pub async fn eval_module(self, name: &str, source: impl Into<Vec<u8>>) -> rquickjs::Result<Self> {
        let bytes: Vec<u8> = source.into();
        let name = name.to_owned();
        self.context
            .with(|ctx| {
                Module::evaluate(ctx.clone(), name.as_str(), bytes)
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))?
                    .finish::<()>()
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            })
            .await?;
        Ok(self)
    }

    pub async fn eval<T>(&self, source: &str) -> rquickjs::Result<T>
    where
        T: for<'js> rquickjs::FromJs<'js>,
    {
        self.context
            .with(|ctx| {
                ctx.eval::<T, _>(source)
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            })
            .await
    }

    pub async fn run(self) -> rquickjs::Result<()> {
        let Self { context, event_loop } = self;
        event_loop.run(&context).await
    }
}
