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

    /// 将一段 JS 代码作为宏任务投递到事件循环（fire & forget，不阻塞当前执行）。
    /// 适用于 defer / async 脚本、需要延迟到当前帧结束后执行的代码。
    pub fn schedule_eval(&self, source: impl Into<String>) -> Result<(), JsRuntimeError> {
        let source = source.into();
        self.event_loop
            .handle()
            .task_tx
            .try_send(Box::new(move |ctx: rquickjs::Ctx<'_>| {
                use rquickjs::CatchResultExt;
                ctx.eval::<(), _>(source.as_bytes())
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            }))
            .map_err(|_| JsRuntimeError::Shutdown)
    }

    /// 将一个 ES 模块作为宏任务投递到事件循环（fire & forget）。
    /// `specifier` 作为模块名（可为合成 URL），供 loader 解析模块内的相对 import。
    pub fn schedule_eval_module(
        &self,
        specifier: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<(), JsRuntimeError> {
        let specifier = specifier.into();
        let source = source.into();
        self.event_loop
            .handle()
            .task_tx
            .try_send(Box::new(move |ctx: rquickjs::Ctx<'_>| {
                use rquickjs::CatchResultExt;
                rquickjs::Module::evaluate(ctx.clone(), specifier.as_str(), source)
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))?
                    .finish::<()>()
                    .catch(&ctx)
                    .map_err(|e| e.throw(&ctx))
            }))
            .map_err(|_| JsRuntimeError::Shutdown)
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
