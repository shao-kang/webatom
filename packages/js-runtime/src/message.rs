use tokio::sync::{mpsc, oneshot};

// ──────────────────────────────────────────────────────────────
// Message trait
// ──────────────────────────────────────────────────────────────

/// 可携带响应的类型化消息。
///
/// 实现此 trait 表示该类型可作为请求发送，并期待一个 `Response` 类型的返回值。
/// 推荐用 [`message!`] 或 [`messages!`] 宏批量定义，也可手动实现。
pub trait Message: Send + 'static {
    type Response: Send + 'static;
}

// ──────────────────────────────────────────────────────────────
// Channel internals
// ──────────────────────────────────────────────────────────────

struct Envelope<M: Message> {
    msg: M,
    reply: oneshot::Sender<M::Response>,
}

// ──────────────────────────────────────────────────────────────
// MessageSender
// ──────────────────────────────────────────────────────────────

/// 可 Clone、可跨线程传递的请求发送端。
///
/// 通过 [`channel`] 创建，调用 [`send`](MessageSender::send) 发送请求并等待响应。
/// 所有 clone 全部 drop 后，对应的 [`MessageReceiver`] 的 `recv()` 将返回 `None`。
pub struct MessageSender<M: Message> {
    tx: mpsc::UnboundedSender<Envelope<M>>,
}

impl<M: Message> Clone for MessageSender<M> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

impl<M: Message> MessageSender<M> {
    /// 发送请求并异步等待响应。
    ///
    /// 如果接收端已被 drop，返回 `None`。
    pub async fn send(&self, msg: M) -> Option<M::Response> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Envelope { msg, reply: reply_tx }).ok()?;
        reply_rx.await.ok()
    }

    /// 发送请求，将回调 sender 暴露给调用方自行管理（用于集成到其他异步结构中）。
    ///
    /// 如果接收端已被 drop，返回 `false`。
    pub fn send_with_reply(&self, msg: M, reply: oneshot::Sender<M::Response>) -> bool {
        self.tx.send(Envelope { msg, reply }).is_ok()
    }
}

// ──────────────────────────────────────────────────────────────
// MessageReceiver
// ──────────────────────────────────────────────────────────────

/// 请求接收端，通常持有在 handler 侧（EventLoop 线程或 tokio task）。
///
/// 每次 `recv` / `try_recv` 返回 `(消息, oneshot::Sender<Response>)`，
/// 处理完成后通过 sender 回传结果。
pub struct MessageReceiver<M: Message> {
    rx: mpsc::UnboundedReceiver<Envelope<M>>,
}

impl<M: Message> MessageReceiver<M> {
    /// 异步等待下一条请求。所有发送端都 drop 后返回 `None`。
    pub async fn recv(&mut self) -> Option<(M, oneshot::Sender<M::Response>)> {
        self.rx.recv().await.map(|e| (e.msg, e.reply))
    }

    /// 非阻塞尝试取出一条请求。无消息时返回 `None`。
    pub fn try_recv(&mut self) -> Option<(M, oneshot::Sender<M::Response>)> {
        self.rx.try_recv().ok().map(|e| (e.msg, e.reply))
    }

    /// 持续处理请求，直到所有发送端 drop。
    ///
    /// `f` 的返回值自动作为响应发回发送方，无需手动调用 `reply.send`。
    ///
    /// # 示例
    /// ```rust
    /// rx.serve(|msg| compute_response(msg)).await;
    /// ```
    pub async fn serve<F>(&mut self, mut f: F)
    where
        F: FnMut(M) -> M::Response,
    {
        while let Some((msg, reply)) = self.recv().await {
            let _ = reply.send(f(msg));
        }
    }

    /// 处理单条请求（非阻塞），有请求时调用 `f` 并发回响应，无请求时返回 `false`。
    pub fn serve_one<F>(&mut self, mut f: F) -> bool
    where
        F: FnMut(M) -> M::Response,
    {
        match self.try_recv() {
            Some((msg, reply)) => { let _ = reply.send(f(msg)); true }
            None => false,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Factory
// ──────────────────────────────────────────────────────────────

/// 创建一对 `(MessageSender<M>, MessageReceiver<M>)`。
///
/// # 示例
/// ```rust
/// let (tx, mut rx) = js_runtime::message::channel::<MyRequest>();
///
/// tokio::spawn(async move {
///     while let Some((msg, reply)) = rx.recv().await {
///         let _ = reply.send(handle(msg));
///     }
/// });
///
/// let result = tx.send(MyRequest { ... }).await;
/// ```
pub fn channel<M: Message>() -> (MessageSender<M>, MessageReceiver<M>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (MessageSender { tx }, MessageReceiver { rx })
}

// ──────────────────────────────────────────────────────────────
// Macros
// ──────────────────────────────────────────────────────────────

/// 定义单个消息类型及其响应类型。
///
/// # 语法
/// ```rust
/// message!(pub MyMsg { field1: String, field2: u32 } -> bool);
/// ```
///
/// 等价于：
/// ```rust
/// pub struct MyMsg { pub field1: String, pub field2: u32 }
/// impl js_runtime::message::Message for MyMsg {
///     type Response = bool;
/// }
/// ```
#[macro_export]
macro_rules! message {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident { $($field:ident : $fty:ty),* $(,)? } -> $resp:ty
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(pub $field: $fty,)*
        }
        impl $crate::message::Message for $name {
            type Response = $resp;
        }
    };
}

/// 批量定义多个消息类型，条目之间用 `;` 分隔。
///
/// # 示例
/// ```rust
/// use js_runtime::messages;
///
/// messages! {
///     pub EvalScript  { source: String }              -> i32;
///     pub GetGlobal   { name: String }                -> Option<String>;
///     pub SetTimeout  { ms: u64, id: u32 }            -> ();
/// }
/// ```
#[macro_export]
macro_rules! messages {
    ($(
        $(#[$meta:meta])*
        $vis:vis $name:ident { $($field:ident : $fty:ty),* $(,)? } -> $resp:ty
    );+ $(;)?) => {
        $(
            $crate::message!{
                $(#[$meta])*
                $vis $name { $($field: $fty,)* } -> $resp
            }
        )+
    };
}
