use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use rquickjs::{Context, Ctx, JsLifetime, Runtime};

// ──────────────────────────────────────────────────────────────
// 任务优先级分级
// ──────────────────────────────────────────────────────────────

/// 任务的调度优先级。
///
/// 消费顺序：AfterMicro → Macro → Idle。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// 微任务检查点后立即执行，对应 `queueMicrotask` 后紧跟的回调。
    AfterMicro,
    /// 宏任务级别，对应 `setTimeout` / `setInterval`。
    Macro,
    /// 空闲任务，仅在高优队列为空时消费，对应 `requestIdleCallback`。
    Idle,
}

// ──────────────────────────────────────────────────────────────
// 事件信封与发送端
// ──────────────────────────────────────────────────────────────

pub type HandlerId = u32;

/// 跨线程传递的事件信封。
///
/// 只携带 payload 和 handler_id；handler 本体留在 EventLoop 线程，
/// 因此 handler 可持有 `Rc`、`RefCell` 等 `!Send` 数据。
pub struct EventEnvelope {
    pub created_at: Instant,
    pub handler_id: HandlerId,
    pub payload: Box<dyn Any + Send>,
}

// EventSender 的实际数据，Arc 包裹，最后一个 clone drop 时自动通知 EventLoop 清理 handler。
struct EventSenderInner {
    tx: mpsc::Sender<EventEnvelope>,
    handler_id: HandlerId,
    cleanup_tx: mpsc::UnboundedSender<HandlerId>,
}

impl Drop for EventSenderInner {
    fn drop(&mut self) {
        let _ = self.cleanup_tx.send(self.handler_id);
    }
}

/// 轻量的跨线程事件发射端，可自由 Clone 后在任意线程调用 [`EventSender::send`]。
///
/// 所有 clone 全部 drop 后，对应的 handler 会从 EventLoop 中自动移除。
#[derive(Clone)]
pub struct EventSender {
    inner: Arc<EventSenderInner>,
}

impl EventSender {
    pub fn send(&self, payload: impl Any + Send) {
        let _ = self.inner.tx.try_send(EventEnvelope {
            created_at: Instant::now(),
            handler_id: self.inner.handler_id,
            payload: Box::new(payload),
        });
    }
}

// ──────────────────────────────────────────────────────────────
// EventLoop
// ──────────────────────────────────────────────────────────────

/// 驱动 QuickJS 运行时的主事件循环。
///
/// 持有三条按优先级分级的 channel，统一消费所有插件推送的事件，
/// 保证跨插件的调度顺序：AfterMicro → Macro → Idle。
///
/// 退出策略：
/// - 主动退出：`cancel_token` 被取消时立即中止；
/// - 自然退出：所有 EventSender 全部 drop（handlers 为空）且无待处理事件时自动退出。
pub struct EventLoop {
    runtime: Runtime,
    cancel_token: CancellationToken,

    after_micro_rx: mpsc::Receiver<EventEnvelope>,
    macro_rx: mpsc::Receiver<EventEnvelope>,
    idle_rx: mpsc::Receiver<EventEnvelope>,

    after_micro_tx: mpsc::Sender<EventEnvelope>,
    macro_tx: mpsc::Sender<EventEnvelope>,
    idle_tx: mpsc::Sender<EventEnvelope>,

    // handler 永远在 EventLoop 线程消费，无需 Send 约束
    handlers: HashMap<HandlerId, Box<dyn FnMut(&dyn Any)>>,
    next_id: HandlerId,

    cleanup_tx: mpsc::UnboundedSender<HandlerId>,
    cleanup_rx: mpsc::UnboundedReceiver<HandlerId>,

    // 饥饿保护：Idle 连续被跳过超过此次数，强制优先执行
    idle_skipped_count: u32,
    // 事件在队列中等待时间小于此阈值时，Macro 任务执行前先 yield 让权
    starvation_threshold: Duration,
}

impl EventLoop {
    pub fn new(runtime: Runtime, cancel_token: CancellationToken) -> Self {
        let (after_micro_tx, after_micro_rx) = mpsc::channel(128);
        let (macro_tx, macro_rx) = mpsc::channel(128);
        let (idle_tx, idle_rx) = mpsc::channel(128);
        let (cleanup_tx, cleanup_rx) = mpsc::unbounded_channel();
        Self {
            runtime,
            cancel_token,
            after_micro_rx,
            macro_rx,
            idle_rx,
            after_micro_tx,
            macro_tx,
            idle_tx,
            handlers: HashMap::new(),
            next_id: 0,
            cleanup_tx,
            cleanup_rx,
            idle_skipped_count: 0,
            starvation_threshold: Duration::from_millis(5),
        }
    }

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventSender`]。
    ///
    /// `handler` 不要求 `Send`，可持有 `Rc`、`RefCell` 等单线程数据。
    /// 所有 [`EventSender`] clone 全部 drop 后，handler 自动从注册表移除。
    pub fn register_event_port<F>(&mut self, task_type: TaskType, handler: F) -> EventSender
    where
        F: FnMut(&dyn Any) + 'static,
    {
        let id = loop {
            let candidate = self.next_id;
            self.next_id = self.next_id.checked_add(1).expect("HandlerId exhausted");
            if let Entry::Vacant(e) = self.handlers.entry(candidate) {
                e.insert(Box::new(handler));
                break candidate;
            }
        };

        let tx = match task_type {
            TaskType::AfterMicro => self.after_micro_tx.clone(),
            TaskType::Macro      => self.macro_tx.clone(),
            TaskType::Idle       => self.idle_tx.clone(),
        };
        EventSender {
            inner: Arc::new(EventSenderInner {
                tx,
                handler_id: id,
                cleanup_tx: self.cleanup_tx.clone(),
            }),
        }
    }

    /// 驱动事件循环直至退出条件触发。
    pub async fn run(&mut self) -> rquickjs::Result<()> {
        loop {
            if self.cancel_token.is_cancelled() {
                break;
            }

            // 清理已全部 drop 的 sender 对应的 handler
            while let Ok(id) = self.cleanup_rx.try_recv() {
                self.handlers.remove(&id);
            }

            // ── drain JS microtask 队列（规范：先于宏任务）────────────────
            self.drain_jobs();

            // ── 1. AfterMicro：全部 drain ────────────────────────────────
            let mut after_micro_fired = false;
            while let Ok(env) = self.after_micro_rx.try_recv() {
                Self::dispatch(&mut self.handlers, env);
                after_micro_fired = true;
            }
            if after_micro_fired {
                self.drain_jobs();
                continue;
            }

            // ── 2. Macro（Idle 未饥饿时优先）────────────────────────────
            const MAX_IDLE_SKIPS: u32 = 100;
            let idle_starved = self.idle_skipped_count >= MAX_IDLE_SKIPS;

            if !idle_starved {
                if let Ok(env) = self.macro_rx.try_recv() {
                    if self.starvation_threshold > Duration::ZERO
                        && env.created_at.elapsed() < self.starvation_threshold
                    {
                        tokio::task::yield_now().await;
                    }
                    Self::dispatch(&mut self.handlers, env);
                    self.idle_skipped_count += 1;
                    self.drain_jobs();
                    continue;
                }
            }

            // ── 3. Idle：高优全空，或饥饿保护触发 ───────────────────────
            if let Ok(env) = self.idle_rx.try_recv() {
                Self::dispatch(&mut self.handlers, env);
                self.idle_skipped_count = 0;
                self.drain_jobs();
                continue;
            }

            // 饥饿保护触发但无 Idle 任务，回落执行 Macro
            if idle_starved {
                if let Ok(env) = self.macro_rx.try_recv() {
                    if self.starvation_threshold > Duration::ZERO
                        && env.created_at.elapsed() < self.starvation_threshold
                    {
                        tokio::task::yield_now().await;
                    }
                    Self::dispatch(&mut self.handlers, env);
                    self.idle_skipped_count = 0;
                    self.drain_jobs();
                    continue;
                }
            }

            // ── 4. 全空：检查自然退出 ────────────────────────────────────
            // handlers 为空意味着所有 EventSender 已 drop，不会再有新事件
            if self.handlers.is_empty() {
                println!("🏁 [EventLoop] 所有任务完成，自动退出。");
                break;
            }

            // ── 5. 等待任意 channel 有数据或退出信号 ─────────────────────
            tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => break,
                Some(_) = self.after_micro_rx.recv() => {},
                Some(_) = self.macro_rx.recv() => {},
                Some(_) = self.idle_rx.recv() => {},
            }
        }
        Ok(())
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
        handlers: &mut HashMap<HandlerId, Box<dyn FnMut(&dyn Any)>>,
        env: EventEnvelope,
    ) {
        if let Some(handler) = handlers.get_mut(&env.handler_id) {
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
    event_loop:Rc<RefCell<EventLoop>>,
    context: Context,
}



impl EventPortRegistrar {
    pub fn new(event_loop: Rc<RefCell<EventLoop>>, context: Context) -> Self {
        Self { event_loop, context }
    }
    pub fn register_js_event_port<F>(&mut self, task_type: TaskType, mut handler: F) -> EventSender
    where
        F: FnMut(Ctx<'_>, &dyn Any) -> rquickjs::Result<()> + 'static,
    {
        let context = self.context.clone();
        self.event_loop.borrow_mut().register_event_port(task_type, move |payload: &dyn Any| {
            let _ = context.with(|ctx| handler(ctx, payload));
        })
    }

    pub fn register_event_port<F>(&mut self, task_type: TaskType, handler: F) -> EventSender
    where
        F: FnMut(&dyn Any) + 'static,
    {
        // 运行时借用检查，获取可变引用
        self.event_loop.borrow_mut().register_event_port(task_type, handler)

    }
}
