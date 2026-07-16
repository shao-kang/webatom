use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use slab::Slab;
use tokio::sync::{Notify, mpsc};
use tokio_util::sync::CancellationToken;
use rquickjs::{Context, Ctx, JsLifetime, Runtime, runtime::UserDataGuard};

use super::render_scheduler::{RenderScheduler};

// ──────────────────────────────────────────────────────────────
// 任务优先级分级
// ──────────────────────────────────────────────────────────────

/// 任务的调度优先级。
///
/// 消费顺序：Macro → Idle。
/// RAF 由 RenderScheduler 在 VSync 时机外部推送，不通过此枚举路由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind {
    /// 宏任务级别，对应 `setTimeout` / `setInterval`。
    Macro,
    /// 空闲任务，仅在高优队列为空时消费，对应 `requestIdleCallback`。
    Idle,
}

// ──────────────────────────────────────────────────────────────
// 事件信封与发送端
// ──────────────────────────────────────────────────────────────

pub type HandlerId = usize;

/// 跨线程传递的事件信封。
///
/// 只携带 payload 和 handler_id；handler 本体留在 EventLoop 线程，
/// 因此 handler 可持有 `Rc`、`RefCell` 等 `!Send` 数据。
///
/// `_keep_alive` 持有 `EventPortGuard` 的引用，保证 envelope 还在 channel 时
/// handler 不会被提前清理。
pub struct EventEnvelope {
    pub queued_at: Instant,
    pub handler_id: HandlerId,
    pub payload: Box<dyn Any + Send>,
    _keep_alive: Arc<EventPortGuard>,
}

// 只负责生命周期守卫，不含 tx，避免 EventEnvelope → Arc<Guard> → Sender<EventEnvelope> 的循环引用。
// 最后一个 clone drop 时自动通知 EventLoop 清理 handler。
struct EventPortGuard {
    handler_id: HandlerId,
    cleanup_tx: mpsc::UnboundedSender<HandlerId>,
    notify: Arc<Notify>,
}

impl Drop for EventPortGuard {
    fn drop(&mut self) {
        let _ = self.cleanup_tx.send(self.handler_id);
        self.notify.notify_one();
    }
}

/// 轻量的跨线程事件注入端，可自由 Clone 后在任意线程调用 [`EventPort::send`]。
///
/// 所有 clone 全部 drop 后，对应的 handler 会从 EventLoop 中自动移除。
#[derive(Clone)]
pub struct EventPort {
    tx: mpsc::UnboundedSender<EventEnvelope>,
    guard: Arc<EventPortGuard>,
}

impl EventPort {
    pub fn send(&self, payload: impl Any + Send) {
        let _ = self.tx.send(EventEnvelope {
            queued_at: Instant::now(),
            handler_id: self.guard.handler_id,
            payload: Box::new(payload),
            _keep_alive: Arc::clone(&self.guard),
        });
        self.guard.notify.notify_one();
    }
}

// ──────────────────────────────────────────────────────────────
// EventLoop
// ──────────────────────────────────────────────────────────────

/// 驱动 QuickJS 运行时的主事件循环。
///
/// 持有两条按优先级分级的 channel（Macro / Idle），统一消费所有插件推送的事件。
/// RAF 由外部 `RenderScheduler` 在 VSync 时机以 `Vec<EventEnvelope>` 批次推送，
/// 不通过 `QueueKind` 路由，保证帧快照语义（rAF 内再注册不进入本帧）。
///
/// 退出策略：
/// - 主动退出：`cancel_token` 被取消时立即中止；
/// - 自然退出：所有 EventPort 全部 drop（handlers 为空）且无待处理事件时自动退出。
pub struct EventLoop {
    runtime: Runtime,
    cancel_token: CancellationToken,
    notify: Arc<Notify>,

    /// 接收 RenderScheduler 每帧推送的 RAF 批次（已快照，可直接 dispatch）。
    raf_batch_rx: mpsc::UnboundedReceiver<Vec<EventEnvelope>>,

    macro_rx: mpsc::UnboundedReceiver<EventEnvelope>,
    idle_rx: mpsc::UnboundedReceiver<EventEnvelope>,

    macro_tx: mpsc::UnboundedSender<EventEnvelope>,
    idle_tx: mpsc::UnboundedSender<EventEnvelope>,

    handlers: Slab<Box<dyn FnMut(&dyn Any)>>,

    cleanup_tx: mpsc::UnboundedSender<HandlerId>,
    cleanup_rx: mpsc::UnboundedReceiver<HandlerId>,

    idle_peeked: Option<EventEnvelope>,
    idle_promote_timeout: Duration,
}

impl EventLoop {
   

    pub fn new(
        runtime: Runtime,
        cancel_token: CancellationToken,
        mut scheduler: impl RenderScheduler,
    ) -> Self {
        let notify = Arc::new(Notify::new());
        let (raf_batch_tx, raf_batch_rx) = mpsc::unbounded_channel();
        let (macro_tx, macro_rx) = mpsc::unbounded_channel();
        let (idle_tx, idle_rx) = mpsc::unbounded_channel();
        let (cleanup_tx, cleanup_rx) = mpsc::unbounded_channel();

        scheduler.connect(raf_batch_tx);

        Self {
            runtime,
            cancel_token,
            notify,
            raf_batch_rx,
            macro_rx,
            idle_rx,
            macro_tx,
            idle_tx,
            handlers: Slab::new(),
            cleanup_tx,
            cleanup_rx,
            idle_peeked: None,
            idle_promote_timeout: Duration::from_millis(50),
        }
    }

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventPort`]。
    ///
    /// `handler` 不要求 `Send`，可持有 `Rc`、`RefCell` 等单线程数据。
    /// 所有 [`EventPort`] clone 全部 drop 后，handler 自动从注册表移除。
    pub fn register_event_port<F>(&mut self, queue: QueueKind, handler: F) -> EventPort
    where
        F: FnMut(&dyn Any) + 'static,
    {
        let id = self.handlers.insert(Box::new(handler));

        let tx = match queue {
            QueueKind::Macro => self.macro_tx.clone(),
            QueueKind::Idle  => self.idle_tx.clone(),
        };
        EventPort {
            tx,
            guard: Arc::new(EventPortGuard {
                handler_id: id,
                cleanup_tx: self.cleanup_tx.clone(),
                notify: self.notify.clone(),
            }),
        }
    }

    /// 驱动事件循环直至退出条件触发。
    pub async fn run(&mut self) -> rquickjs::Result<()> {
        loop {
            if self.cancel_token.is_cancelled() {
                break;
            }

            // ── 先执行已有的 JS jobs（规范：每个 task 前先 checkpoint）───
            self.drain_jobs();

            let mut progressed = false;

            // ── 1. RAF：每帧由 RenderScheduler 推送批次，保证快照语义 ───
            while let Ok(raf_batch) = self.raf_batch_rx.try_recv() {
                for env in raf_batch {
                    Self::dispatch(&mut self.handlers, env);
                    self.drain_jobs();
                }
                progressed = true;
            }

            // ── 2. 填充 idle peek buffer ─────────────────────────────────
            if self.idle_peeked.is_none() {
                self.idle_peeked = self.idle_rx.try_recv().ok();
            }

            let idle_timed_out = self.idle_peeked
                .as_ref()
                .map(|env| env.queued_at.elapsed() > self.idle_promote_timeout)
                .unwrap_or(false);

            if idle_timed_out {
                // ── 3a. Idle 超时：强制执行 ──────────────────────────────
                let env = self.idle_peeked.take().unwrap();
                Self::dispatch(&mut self.handlers, env);
                self.drain_jobs();
                progressed = true;
            } else if let Ok(env) = self.macro_rx.try_recv() {
                // ── 3b. Macro：一个 ──────────────────────────────────────
                Self::dispatch(&mut self.handlers, env);
                self.drain_jobs();
                progressed = true;
            } else if let Some(env) = self.idle_peeked.take() {
                // ── 3c. Idle：macro 为空时正常执行 ───────────────────────
                Self::dispatch(&mut self.handlers, env);
                self.drain_jobs();
                progressed = true;
            }
             // ── dispatch 后再 cleanup，保证已入队事件有机会执行 ─────────
            while let Ok(id) = self.cleanup_rx.try_recv() {
                self.handlers.try_remove(id);
            }
            if progressed {
                continue;
            }

            // ── 4. 全空：handlers 为空且队列无待处理事件时自然退出 ───────
            if self.handlers.is_empty() && !self.has_pending_events() {
                println!("🏁 [EventLoop] 所有任务完成，自动退出。");
                break;
            }

            // ── 5. 进入等待前再扫一次，避免 send 与 await 之间的窗口期 ──
            if self.has_pending_events() {
                continue;
            }

            // ── 6. 等待新事件或退出信号（不消费消息）────────────────────
            tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => break,
                _ = self.notify.notified() => {},
            }
        }
        Ok(())
    }

    fn has_pending_events(&self) -> bool {
        !self.raf_batch_rx.is_empty()
            || !self.macro_rx.is_empty()
            || self.idle_peeked.is_some()
            || !self.idle_rx.is_empty()
    }

    /// 循环执行所有待处理的 JS jobs（microtask checkpoint）。
    ///
    /// Job 抛出异常时记录日志并继续，不崩溃事件循环（对应浏览器的 unhandledrejection 行为）。
    fn drain_jobs(&self) {
        loop {
            match self.runtime.execute_pending_job() {
                Ok(true) => {}
                Ok(false) => break,
                Err(e) => eprintln!("[EventLoop] unhandled JS job exception: {:?}", e),
            }
        }
    }

    fn dispatch(
        handlers: &mut Slab<Box<dyn FnMut(&dyn Any)>>,
        env: EventEnvelope,
    ) {
        if let Some(handler) = handlers.get_mut(env.handler_id) {
            handler(env.payload.as_ref());
        }
    }
}

// ──────────────────────────────────────────────────────────────
// EventPortRegistrar
// ──────────────────────────────────────────────────────────────

/// 事件端口注册代理，在 Extension::setup 中使用。
///
/// 只暴露注册能力，Extension 无需感知完整的 [`EventLoop`] 接口。
#[derive(Clone, JsLifetime)]
pub struct EventPortRegistrar {
    event_loop: Rc<RefCell<EventLoop>>,
    context: Context,
}

impl EventPortRegistrar {
    pub fn new(event_loop: Rc<RefCell<EventLoop>>, context: Context) -> Self {
        Self { event_loop, context }
    }

    pub fn from_ctx<'c, 'js>(ctx: &'c Ctx<'js>) -> Option<UserDataGuard<'c, EventPortRegistrar>> {
        ctx.userdata::<EventPortRegistrar>()
    }

    pub fn register_js_event_port<F>(&mut self, queue: QueueKind, mut handler: F) -> EventPort
    where
        F: FnMut(Ctx<'_>, &dyn Any) -> rquickjs::Result<()> + 'static,
    {
        let context = self.context.clone();
        self.event_loop.borrow_mut().register_event_port(queue, move |payload: &dyn Any| {
            let _ = context.with(|ctx| handler(ctx, payload));
        })
    }

    pub fn register_event_port<F>(&mut self, queue: QueueKind, handler: F) -> EventPort
    where
        F: FnMut(&dyn Any) + 'static,
    {
        self.event_loop.borrow_mut().register_event_port(queue, handler)
    }
}
