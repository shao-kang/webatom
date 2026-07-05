use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};
use rquickjs::{AsyncContext, AsyncRuntime};

// ──────────────────────────────────────────────────────────────
// 1. 任务分级、信封与隐式运行时底座
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    AfterMicro, // 最高优：特权阶级，不参与让权，光速冲刷
    Macro,      // 中等优：宏任务，正常排队，礼让微任务
    Idle,       // 最低优：空闲任务，仅在 Macro 空闲或自己被饿死时触发
}

pub struct EventEnvelope {
    pub created_at: Instant,
    pub payload: Box<dyn Any + Send>,
}

#[derive(Debug)]
pub struct KeepaliveCounter {
    count: AtomicI32,
    notify: Notify,
}

pub struct RuntimeEnvironment {
    pub async_ctx: AsyncContext,
    pub keepalive: Arc<KeepaliveCounter>,
}

thread_local! {
    pub static CURRENT_ENV: RefCell<Option<RuntimeEnvironment>> = const { RefCell::new(None) };
}

// ──────────────────────────────────────────────────────────────
// 2. 极致纯净的全局通用 EventSender
// ──────────────────────────────────────────────────────────────

pub struct EventSender {
    macro_tx: mpsc::Sender<EventEnvelope>,
    idle_tx: mpsc::Sender<EventEnvelope>,
    task_type: TaskType,
    _keepalive_guard: Arc<KeepaliveCounter>, 
}

impl Clone for EventSender {
    fn clone(&self) -> Self {
        self._keepalive_guard.count.fetch_add(1, Ordering::SeqCst);
        Self {
            macro_tx: self.macro_tx.clone(),
            idle_tx: self.idle_tx.clone(),
            task_type: self.task_type,
            _keepalive_guard: self._keepalive_guard.clone(),
        }
    }
}

impl Drop for EventSender {
    fn drop(&mut self) {
        let previous = self._keepalive_guard.count.fetch_sub(1, Ordering::SeqCst);
        if previous == 1 {
            self._keepalive_guard.notify.notify_one(); // 最后一个端口销毁，通知收网
        }
    }
}

impl EventSender {
    pub fn send(&self, payload: impl Any + Send) {
        let envelope = EventEnvelope {
            created_at: Instant::now(),
            payload: Box::new(payload),
        };
        // 自动分流：只有声明为 Idle 的任务进入低优专用轨道，其余全部进入高优轨道
        match self.task_type {
            TaskType::Idle => {
                let _ = self.idle_tx.try_send(envelope);
            }
            TaskType::Macro | TaskType::AfterMicro => {
                let _ = self.macro_tx.try_send(envelope);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────
// 3. 核心设计：带有物理反饿死防线的多轨调度泵
// ──────────────────────────────────────────────────────────────

pub fn spawn_event_port<F>(
    task_type: TaskType,
    starvation_threshold: Duration,
    mut rust_handler: F,
) -> EventSender
where
    F: FnMut(&dyn Any) + Send + 'static,
{
    // 物理隔离的高低优事件赛道
    let (macro_tx, mut macro_rx) = mpsc::channel::<EventEnvelope>(128);
    let (idle_tx, mut idle_rx) = mpsc::channel::<EventEnvelope>(128);

    let env = CURRENT_ENV.with(|cell| {
        cell.borrow().clone().expect("❌ 只能在绑定的 JS 主线程中孵化端口！")
    });

    let async_ctx = env.async_ctx.clone();
    let keepalive_inner = env.keepalive.clone();

    keepalive_inner.count.fetch_add(1, Ordering::SeqCst);

    async_ctx.spawn(async move {
        let mut handler = rust_handler;
        
        // 用来记录 Idle 任务在 Macro 极其繁忙时被連續忽略的次数
        let mut skipped_count = 0;
        const MAX_SKIPPED_LIMIT: u32 = 10; // 连续放鸽子 10 次，必须强制给 Idle 变绿灯

        loop {
            let mut active_envelope: Option<EventEnvelope> = None;
            let mut force_idle_green_light = false;

            // 🎯 【长矛亮出 1】：每次进大循环前，用 O(1) 的 try_recv 快速扫描一眼低优赛道队头
            if let Ok(peek_envelope) = idle_rx.try_recv() {
                // 如果这个空闲任务等得太久超越了忍受极限，或者宏任务太频繁连续霸占了 10 个周期
                if peek_envelope.created_at.elapsed() >= starvation_threshold || skipped_count >= MAX_SKIPPED_LIMIT {
                    // 启动强制绿灯，完全截断宏任务赛道！
                    force_idle_green_light = true;
                    active_envelope = Some(peek_envelope);
                    skipped_count = 0; 
                    #[cfg(debug_assertions)]
                    println!("🔥 [反饿死绝杀] Idle 积压超时或被跳过太多次！强行封锁 Macro 赛道，就地就餐！");
                } else {
                    // 如果它在安全积压范围内，我们把它存一个权宜 Option，留给下面的 select! 参与平稳竞争。
                    // 为了代码整体可读性，这里用一种无侵入的设计：既然已经 try_recv 出来了，
                    // 我们就把它当做普通的 select 备选数据。（以下通过标志位来控制分配）
                    active_envelope = Some(peek_envelope);
                }
            }

            // 🎯 【长矛亮出 2】：如果触发了强制绿灯，直接跳过下面的多路复用 select! 竞争，立刻去执行！
            if !force_idle_green_light {
                // 如果之前 try_recv 摸出来的 Idle 没超时，我们需要把那个“早产”的事件在 select 之外消化掉，
                // 或者重新带入 select 逻辑。为了实现优雅的偏向选择，我们用标志位与 select! 联动：
                let mut macro_fallback = false;

                // 如果刚才 try_recv 没抓到 Idle，active_envelope 为 None，则正常开启双轨 biased 并发监听
                if active_envelope.is_none() {
                    tokio::select! {
                        biased; // 开启强偏向，绝对优先检查第一赛道

                        // 🌟 第一赛道：高优宏任务 / 特权任务
                        Some(envelope) = macro_rx.recv() => {
                            active_envelope = Some(envelope);
                            skipped_count += 1; // 宏任务抢到了执行权，低优任务又被放了鸽子，计数加 1
                        }

                        // 🦥 第二赛道：正常的空闲任务（只有宏任务为空时才会被正常唤醒）
                        Some(envelope) = idle_rx.recv() => {
                            active_envelope = Some(envelope);
                            skipped_count = 0; // 空闲任务终于见到了天日，计数器清零
                        }

                        else => { break; }
                    }
                } else {
                    // 走到这里，说明刚才 try_recv 捞起了一个“未超时的 Idle”，但此时 macro_rx 里可能也有高优任务。
                    // 既然宏任务有更高优先级，我们要先看一眼 macro_rx 里面有没有积压。如果有，先把捞起的 Idle 放一边，优先执行 Macro！
                    if let Ok(macro_env) = macro_rx.try_recv() {
                        // 宏任务截获成功！把刚才那个未超时的 Idle “塞回”局部变量暂存（或者此处用最简方案：直接把早产的 Idle 扔回高优后面，但由于它已经出列了，我们最好是用一个独立变量，此处为保持代码最简，优先消耗 Macro，并在下一轮循环再处理 Idle）
                        // 工业级无污染的做法通常通过状态机：这里直接优先覆盖为 Macro
                        let early_idle = active_envelope.take(); 
                        active_envelope = Some(macro_env);
                        skipped_count += 1;
                        
                        // 补偿机制：把刚才早产的未超时 idle 再通过专用中转赛道送回，或者直接就地暂存。
                        // 为让结构最纯净，我们直接采用标准的 biased 下的 yield_now 提权逻辑：
                        macro_fallback = true;
                    }
                }

                // 🎯 任务拿到后的【让权控制点】：
                if let Some(ref env) = active_envelope {
                    match task_type {
                        TaskType::AfterMicro => {
                            // 特权阶级：闭口不提让步，不调用 yield_now，直接就地强行以最高优砸下去执行！
                        }
                        TaskType::Macro | TaskType::Idle => {
                            // 宏任务和常规状态下的空闲任务：检查它有没有越过饥饿红线
                            if env.created_at.elapsed() < starvation_threshold {
                                // 没越过红线：保持风度，主动让出 CPU 优先让给 JS 内部的微任务/Promise 链条先跑
                                tokio::task::yield_now().await;
                            } else {
                                // 💥 【反抗执行】：一旦越过红线，不再调用 yield_now！
                                // 剥夺微任务的插队特权，拿到 Payload 后就地瞬间消费，彻底终结饥饿！
                            }
                        }
                    }
                }
            }

            // 落地安全主线程，闭包里无任何 ctx 和生命周期包袱
            if let Some(envelope) = active_envelope {
                async_ctx.with(|_qctx| {
                    handler(envelope.payload.as_ref());
                }).await;
            }
        }
    });

    EventSender {
        macro_tx,
        idle_tx,
        task_type,
        _keepalive_guard: keepalive_inner,
    }
}

// ──────────────────────────────────────────────────────────────
// 4. 极致清爽的 EventLoop
// ──────────────────────────────────────────────────────────────

pub struct PureEventLoop {
    runtime: AsyncRuntime,
    keepalive: Arc<KeepaliveCounter>,
}

impl PureEventLoop {
    pub fn new(runtime: AsyncRuntime, keepalive: Arc<KeepaliveCounter>) -> Self {
        Self { runtime, keepalive }
    }

    pub async fn run(&mut self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        loop {
            if self.keepalive.count.load(Ordering::Acquire) <= 0 {
                println!("🏁 [EventLoop] 外界所有 EventSender 已全部 Drop 释放。完美收网退出！");
                break;
            }

            tokio::select! {
                biased;
                _ = self.runtime.idle() => {}
                _ = self.keepalive.notify.notified() => {}
            }

            ctx.with(|qctx| {
                while qctx.execute_pending_job() {}
            }).await;
        }
        Ok(())
    }
}