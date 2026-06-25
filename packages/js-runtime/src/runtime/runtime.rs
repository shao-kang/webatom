use rquickjs::AsyncContext;

use crate::event_loop::{EventLoop, EventLoopHandle};

use super::builder::JsRuntimeBuilder;

#[derive(Debug)]
pub enum JsRuntimeError {
    Shutdown,
    Js(rquickjs::Error),
}

impl From<rquickjs::Error> for JsRuntimeError {
    fn from(e: rquickjs::Error) -> Self {
        Self::Js(e)
    }
}

impl std::fmt::Display for JsRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shutdown => write!(f, "runtime has already shut down"),
            Self::Js(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for JsRuntimeError {}

pub struct JsRuntime {
    // Drop order: context before event_loop (which owns the AsyncRuntime).
    ctx: AsyncContext,
    event_loop: EventLoop,
    shutdown: bool,
}

impl JsRuntime {
    pub(super) fn new(ctx: AsyncContext, event_loop: EventLoop) -> Self {
        Self { ctx, event_loop, shutdown: false }
    }

    pub fn builder() -> JsRuntimeBuilder {
        JsRuntimeBuilder::new()
    }

    pub fn handle(&self) -> EventLoopHandle {
        self.event_loop.handle()
    }

    pub async fn eval_module(
        &mut self,
        name: &str,
        source: impl Into<Vec<u8>>,
    ) -> Result<(), JsRuntimeError> {
        if self.shutdown {
            return Err(JsRuntimeError::Shutdown);
        }
        let bytes: Vec<u8> = source.into();
        let name = name.to_owned();
        self.ctx
            .with(|ctx| {
                use rquickjs::{CatchResultExt, Module};
                Module::evaluate(ctx.clone(), name.as_str(), bytes)
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))?
                    .finish::<()>()
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            })
            .await?;
        Ok(())
    }

    pub async fn eval<T>(&mut self, source: &str) -> Result<T, JsRuntimeError>
    where
        T: for<'js> rquickjs::FromJs<'js> + Send,
    {
        if self.shutdown {
            return Err(JsRuntimeError::Shutdown);
        }
        let result = self
            .ctx
            .with(|ctx| {
                use rquickjs::CatchResultExt;
                ctx.eval::<T, _>(source)
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            })
            .await?;
        Ok(result)
    }

    /// Run the event loop to completion. After this returns, `eval` will
    /// return `Err(JsRuntimeError::Shutdown)`.
    pub async fn run(&mut self) -> Result<(), JsRuntimeError> {
        if self.shutdown {
            return Err(JsRuntimeError::Shutdown);
        }
        struct ShutdownGuard<'a>(&'a mut bool);
        impl Drop for ShutdownGuard<'_> {
            fn drop(&mut self) {
                *self.0 = true;
            }
        }
        let _guard = ShutdownGuard(&mut self.shutdown);
        self.event_loop.run(&self.ctx).await?;
        Ok(())
    }
}
