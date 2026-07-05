use std::any::Any;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken; // 引入标准的 Tokio 取消令牌
use rquickjs::{AsyncContext, AsyncRuntime, Ctx};

// ──────────────────────────────────────────────────────────────
// 1. 极致精简的任务分级与信封
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    AfterMicro,
    Macro,
    Idle,
}

pub struct EventEnvelope {
    pub created_at: Instant,
    pub payload: Box<dyn Any + Send>,
}

/// 🚀 没有任何计数器污染、极度轻量的跨线程发射端
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

pub trait EventPortExt {
    fn spawn_event_port<F>(
        &self,
        task_type: TaskType,
        starvation_threshold: Duration,
        rust_handler: F,
    ) -> EventSender
    where
        F: for<'js> FnMut(&Ctx<'js>, &dyn Any) + Send + 'static;
}

impl<'js> EventPortExt for Ctx<'js> {
    fn spawn_event_port<F>(
        &self,
        task_type: TaskType,
        starvation_threshold: Duration,
        mut rust_handler: F,
    ) -> EventSender
    where
        F: for<'js> FnMut(&Ctx<'js>, &dyn Any) + Send + 'static,
    {
        let (macro_tx, mut macro_rx) = mpsc::channel::<EventEnvelope>(128);
        let (idle_tx, mut idle_rx) = mpsc::channel::<EventEnvelope>(128);

        // 通过 self 无锁无依赖直接拿到房间底座
        let async_ctx = self.async_context();

        async_ctx.spawn(async move {
            let mut handler = rust_handler;
            let mut skipped_count = 0;
            const MAX_SKIPPED_LIMIT: u32 = 10;

            loop {
                let mut active_envelope: Option<EventEnvelope> = None;
                let mut force_idle_green_light = false;

                // 盲摸低优，反饿死长矛
                if let Ok(peek_envelope) = idle_rx.try_recv() {
                    if peek_envelope.created_at.elapsed() >= starvation_threshold || skipped_count >= MAX_SKIPPED_LIMIT {
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
                        handler(qctx, envelope.payload.as_ref());
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

pub struct EventLoop {
    runtime: AsyncRuntime,
    cancel_token: CancellationToken, // 强制取消令牌
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime, cancel_token: CancellationToken) -> Self {
        Self { runtime, cancel_token }
    }

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
                
                // 防线 3：JS 引擎底层的驱动核心。
                // runtime.idle() 具有决定生死的返回值：
                // 当它返回 Ok(false) 或者错误时，明确意味着：
                // “当前 Runtime 下所有房间里所有的脚本执行完了，没有任何挂起的定时器、没有挂起的 Promise，彻底没事干了”
                idle_result = self.runtime.idle() => {
                    if let Ok(has_more_jobs) = idle_result {
                        if !has_more_jobs {
                            // 🎯 核心绝杀：JS 侧全空闲，且没有任何待处理任务，完美的自动全寿终正寝！
                            println!("🏁 [EventLoop] JS 侧所有任务、定时器、Promise 已全数执行完毕且环境空闲。完美全自动收网退出！");
                            break;
                        }
                    } else {
                        // 发生致命错误，直接退出
                        break;
                    }
                }
            }

            // 刷洗、清空主线程当前的微任务链条
            while self.runtime.execute_pending_job().is_some() {}
        }
        Ok(())
    }
}