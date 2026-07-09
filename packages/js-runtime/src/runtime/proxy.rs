use tokio_util::sync::CancellationToken;
use rquickjs::Context;

use crate::message::{self, Message, MessageSender, MessageReceiver};

// ── Request / Response ───────────────────────────────────────────────────────

pub enum JsRuntimeRequest {
    Eval(String),
    EvalModule { name: String, source: String },
}

pub enum JsRuntimeResponse {
    Eval(Result<String, String>),
    Module(Result<(), String>),
}

impl Message for JsRuntimeRequest {
    type Response = JsRuntimeResponse;
}

// ── Public proxy handle ──────────────────────────────────────────────────────

/// 可 Clone、可跨线程传递的 JsRuntime 访问句柄。
///
/// 通过 [`JsRuntime::proxy`] 创建。`eval` / `eval_module` 将请求发往运行时线程执行，
/// 并异步等待响应。运行时停止后，所有 `send` 调用返回 `Err`。
#[derive(Clone)]
pub struct JsRuntimeProxy {
    tx: MessageSender<JsRuntimeRequest>,
    cancel: CancellationToken,
}

impl JsRuntimeProxy {
    pub(super) fn new(tx: MessageSender<JsRuntimeRequest>, cancel: CancellationToken) -> Self {
        Self { tx, cancel }
    }

    /// 在运行时执行 JS 表达式，返回结果的 Debug 字符串。
    pub async fn eval(&self, source: impl Into<String>) -> Result<String, String> {
        match self.tx.send(JsRuntimeRequest::Eval(source.into())).await {
            Some(JsRuntimeResponse::Eval(r)) => r,
            _ => Err("runtime unavailable".into()),
        }
    }

    /// 在运行时声明并执行一个 ESM 模块。
    pub async fn eval_module(
        &self,
        name: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<(), String> {
        match self.tx.send(JsRuntimeRequest::EvalModule {
            name: name.into(),
            source: source.into(),
        }).await {
            Some(JsRuntimeResponse::Module(r)) => r,
            _ => Err("runtime unavailable".into()),
        }
    }

    /// 取消运行时（触发 CancellationToken）。
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

// ── Internal: channel factory ────────────────────────────────────────────────

pub(super) fn make_channel()
    -> (MessageSender<JsRuntimeRequest>, MessageReceiver<JsRuntimeRequest>)
{
    message::channel()
}

// ── Internal: request handler（在 JS 线程上调用）─────────────────────────────

pub(super) fn handle_request(ctx: &Context, req: JsRuntimeRequest) -> JsRuntimeResponse {
    match req {
        JsRuntimeRequest::Eval(src) => JsRuntimeResponse::Eval(
            ctx.with(|js| {
                js.eval::<rquickjs::Value<'_>, _>(src.as_bytes())
                    .map(|v| format!("{v:?}"))
            })
            .map_err(|e| e.to_string()),
        ),
        JsRuntimeRequest::EvalModule { name, source } => JsRuntimeResponse::Module(
            ctx.with(|js| {
                let module = rquickjs::Module::declare(js, name, source)?;
                let _ = module.eval()?;
                Ok::<(), rquickjs::Error>(())
            })
            .map_err(|e: rquickjs::Error| e.to_string()),
        ),
    }
}
