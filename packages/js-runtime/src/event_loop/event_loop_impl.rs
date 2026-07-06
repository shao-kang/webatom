use std::any::Any;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken; // 引入标准的 Tokio 取消令牌
use rquickjs::{AsyncContext, AsyncRuntime, Ctx};

// ──────────────────────────────────────────────────────────────
// 1. 极致精简的任务分级与信封
// ──────────────────────────────────────────────────────────────

/// 任务的调度优先级分级。
///
/// 调度顺序：AfterMicro > Macro > Idle。
/// 同一 port 内部以此枚举控制 yield 行为和饥饿保护策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// 微任务检查点之后立即执行，不主动 yield，对应 `queueMicrotask` 后紧跟的回调。
    AfterMicro,
    /// 宏任务级别，执行前 yield 一次让出控制权，对应 `setTimeout` / `setInterval`。
    Macro,
    /// 空闲任务，仅在高优队列为空时消费，对应 `requestIdleCallback`。
    Idle,
}

/// 跨线程传递的事件信封。
///
/// `created_at` 记录入队时刻，用于饥饿检测：当 Idle 任务等待时长超过
/// `starvation_threshold` 时，调度器会强制绕过高优队列直接消费它。
pub struct EventEnvelope {
    pub created_at: Instant,
    pub payload: Box<dyn Any + Send>,
}

/// 轻量的跨线程事件发射端，可自由 Clone 后在任意线程调用 [`EventSender::send`]。
///
/// 内部持有两条独立通道：`macro_tx`（高优）和 `idle_tx`（低优），
/// 发送时根据构造时绑定的 `task_type` 自动路由，调用方无需感知通道细节。
#[derive(Clone)]
pub struct EventSender {
    macro_tx: mpsc::Sender<EventEnvelope>,
    idle_tx: mpsc::Sender<EventEnvelope>,
    task_type: TaskType,
}

impl EventSender {
    pub fn send(&self, payload: impl Any + Send) {
        let envelope = EventEnvelope {
            created_at: Instant::now(),
            payload: Box::new(payload),
        };
        match self.task_type {
            TaskType::Idle => { let _ = self.idle_tx.try_send(envelope); }
            TaskType::Macro | TaskType::AfterMicro => { let _ = self.macro_tx.try_send(envelope); }
        }
    }
}

// ──────────────────────────────────────────────────────────────
// 2. 纯净度 100% 的 Ctx 扩展方法
// ──────────────────────────────────────────────────────────────

/// 为 [`rquickjs::Ctx`] 提供事件端口的扩展方法。
///
/// 每次调用会在当前 QuickJS 上下文内部启动一个独立的异步调度泵，
/// 返回可跨线程复用的 [`EventSender`]。调用方只管 `send`，
/// 优先级调度、饥饿保护、让权逻辑均由泵内部处理。
pub trait EventPortExt {
    /// 通用入口，完整控制 `task_type` 和 `starvation_threshold`。
    ///
    /// `starvation_threshold` 为零时 Idle 任务永不因超时被强制提升优先级，
    /// 仅在 `skipped_count` 达到上限时才会被消费。
    fn spawn_event_port<F>(
        &self,
        task_type: TaskType,
        starvation_threshold: Duration,
        rust_handler: F,
    ) -> EventSender
    where
        F:  FnMut( &dyn Any) + Send + 'static;

    /// AfterMicro port — starvation_threshold 默认 10ms，紧跟微任务检查点执行。
    fn spawn_after_micro_port<F>(&self, rust_handler: F) -> EventSender
    where
        F:  FnMut( &dyn Any) + Send + 'static,
    {
        self.spawn_event_port(TaskType::AfterMicro, Duration::from_millis(10), rust_handler)
    }

    /// Macro port — starvation_threshold defaults to 16ms（一帧预算，对应 setTimeout 级别）
    fn spawn_macro_port<F>(&self, rust_handler: F) -> EventSender
    where
        F:  FnMut( &dyn Any) + Send + 'static,
    {
        self.spawn_event_port(TaskType::Macro, Duration::from_millis(16), rust_handler)
    }

    /// Idle port — starvation_threshold defaults to 50ms（空闲回调典型预算）
    fn spawn_idle_port<F>(&self, rust_handler: F, starvation_threshold: Duration) -> EventSender
    where
        F:  FnMut( &dyn Any) + Send + 'static,
    {
        self.spawn_event_port(TaskType::Idle, starvation_threshold, rust_handler)
    }
}

impl EventPortExt for AsyncContext {
    /// starvation_threshold 如果取值为零 永远没有超时时间
    fn spawn_event_port<F>(
        &self,
        task_type: TaskType,
        starvation_threshold: Duration,
        mut rust_handler: F,
    ) -> EventSender
    where
        F: FnMut( &dyn Any) + Send + 'static,
    {
        let (macro_tx, mut macro_rx) = mpsc::channel::<EventEnvelope>(128);
        let (idle_tx, mut idle_rx) = mpsc::channel::<EventEnvelope>(128);

        // 通过 self 无锁无依赖直接拿到房间底座
        let async_ctx = self.clone();

        tokio::task::spawn_local(async move {
            let mut handler = rust_handler;
            let mut skipped_count = 0;
            const MAX_SKIPPED_LIMIT: u32 = 100;

            loop {
                let mut active_envelope: Option<EventEnvelope> = None;
                let mut force_idle_green_light = false;

                // 盲摸低优，反饿死长矛
                if let Ok(peek_envelope) = idle_rx.try_recv() {
                    if (peek_envelope.created_at.elapsed() >= starvation_threshold && starvation_threshold > Duration::ZERO)|| skipped_count >= MAX_SKIPPED_LIMIT {
                        force_idle_green_light = true;
                        active_envelope = Some(peek_envelope);
                        skipped_count = 0;
                    } else {
                        active_envelope = Some(peek_envelope);
                    }
                }

                if !force_idle_green_light {
                    if active_envelope.is_none() {
                        tokio::select! {
                            biased;
                            Some(envelope) = macro_rx.recv() => {
                                active_envelope = Some(envelope);
                                skipped_count += 1;
                            }
                            Some(envelope) = idle_rx.recv() => {
                                active_envelope = Some(envelope);
                                skipped_count = 0;
                            }
                            else => {
                                // 🌟 外部 Sender 全部被 Drop 时，管道全断开，局部泵优雅自杀
                                break;
                            }
                        }
                    } else if let Ok(macro_env) = macro_rx.try_recv() {
                        active_envelope = Some(macro_env);
                        skipped_count += 1;
                    }

                    // 优先级控制与让权
                    if let Some(ref env) = active_envelope {
                        match task_type {
                            TaskType::AfterMicro => {}
                            TaskType::Macro | TaskType::Idle => {
                                if env.created_at.elapsed() < starvation_threshold {
                                    tokio::task::yield_now().await;
                                }
                            }
                        }
                    }
                }

                // 落地消费，动态由底层唤醒并传出精准房间的 qctx
                if let Some(envelope) = active_envelope {
                    async_ctx.with(|qctx| {
                        handler( envelope.payload.as_ref());
                    }).await;
                }
            }
        });

        EventSender { macro_tx, idle_tx, task_type }
    }
}

// ──────────────────────────────────────────────────────────────
// 3. 终极双保险大闸：PureEventLoop
// ──────────────────────────────────────────────────────────────

/// 驱动 QuickJS 运行时的主事件循环。
///
/// 采用双保险退出策略：
/// - 主动退出：`cancel_token` 被取消时立即中止；
/// - 自然退出：`runtime.idle()` 返回 `false`（JS 侧无任何挂起任务）时自动收网。
pub struct EventLoop {
    runtime: AsyncRuntime,
    /// 宿主可随时通过此令牌强制中止循环，优先级高于 JS 侧的自然空闲检测。
    cancel_token: CancellationToken,
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime, cancel_token: CancellationToken) -> Self {
        Self { runtime, cancel_token }
    }

    /// 阻塞运行事件循环直至退出条件触发，退出后刷完最后一批 pending job。
    pub async fn run(&mut self) -> rquickjs::Result<()> {
        loop {
            // 防线 1：随时检查宿主是否拉响了强制退出的警报
            if self.cancel_token.is_cancelled() {
                println!("🛑 [EventLoop] 收到 CancellationToken 强行中止信号，大循环紧急收网退出！");
                break;
            }

            tokio::select! {
                biased; // 强偏向模式

                // 防线 2：绝对优先倾听取消令牌的异步通知
                _ = self.cancel_token.cancelled() => {
                    continue; // 重新进循环，会在大循环顶部直接 break
                }
                _ = self.runtime.drive() => {
                    continue;
                }
                
                // 防线 3：JS 引擎底层的驱动核心。
                // runtime.idle() 具有决定生死的返回值：
                // 当它返回 Ok(false) 或者错误时，明确意味着：
                // “当前 Runtime 下所有房间里所有的脚本执行完了，没有任何挂起的定时器、没有挂起的 Promise，彻底没事干了”
                _idle_result = self.runtime.idle() => {

                      // 🎯 核心绝杀：JS 侧全空闲，且没有任何待处理任务，完美的自动全寿终正寝！
                    println!("🏁 [EventLoop] JS 侧所有任务、定时器、Promise 已全数执行完毕且环境空闲。完美全自动收网退出！");
                    break;
                }
            }

        }
        Ok(())
    }
}