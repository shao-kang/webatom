use tokio_util::sync::CancellationToken;
use rquickjs::Context;

/// 可自由 Clone 的运行时引用（!Send，只能在 EventLoop 线程使用）。
///
/// Extension 在 `setup` 时将句柄存入 handler 闭包，在事件回调中通过
/// `handle.context.with(|ctx| { ... })` 调用 JS。
#[derive(Clone)]
pub struct RuntimeHandle {
    pub context: Context,
    pub cancel: CancellationToken,
}

impl RuntimeHandle {
    pub fn new(context: Context, cancel: CancellationToken) -> Self {
        Self { context, cancel }
    }
}
