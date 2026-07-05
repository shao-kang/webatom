use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{AsyncContext, AsyncRuntime};
use tokio::sync::{mpsc, watch};

use super::task::{MacroTask, RafTask};
use super::handle::{EventLoopHandle, HostBridge, KeepAliveCounter, RuntimeBridge, RuntimeIo, SchedulerBridge};
use crate::log_targets as target;
use super::idle::{IdleQueue, IdleScheduler};
use super::render_scheduler::{HeadlessRenderScheduler, RenderScheduler, VsyncSignal};
use super::event::{Event, EventType, EventHandler};


// ──────────────────────────────────────────────────────────────
// EventSender — cross-thread, cheap to clone
// ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EventSender {
    txs: HashMap<EventType, mpsc::Sender<Box<dyn Any + Send>>>,
}

impl EventSender {
    pub async fn send<E: Event>(&self, event: E) {
        let _ = self.txs.get(&E::TYPE).unwrap().send(Box::new(event)).await;
    }
}

pub struct EventLoop {
    event_sender: EventSender,
    runtime: AsyncRuntime,
    handlers: HashMap<EventType, Box<dyn Fn(&dyn Any)>>,
    rxs: HashMap<EventType, mpsc::Receiver<Box<dyn Any + Send>>>,
    keepalive_counter: KeepaliveCounter,
}
/// ==================== 核心平铺宏 ====================
/// 该宏负责在编译期直接平铺解构出 HashMap 里的多个 Receiver 引用，并 Pin 在栈上。
/// 这样做避开了 FuturesUnordered，保证了绝对的极致性能与控制权。
macro_rules! pin_receivers {
    ($rxs:expr => { $($variant:ident => $name:ident),+ }) => {
        $(
            // 从 HashMap 中安全取出 Receiver 的可变引用
            let $name = $rxs.get_mut(&EventType::$variant).unwrap();
            tokio::pin!($name);
        )+
    };
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime) -> Self {
        let buffer = 1024;
        let mut rxs = HashMap::new();
        let mut txs = HashMap::new();
        for event_type in EventType::iter() {
            let (tx, rx) = mpsc::channel(buffer);
            rxs.insert(event_type.clone(), rx);
            txs.insert(event_type.clone(), tx);
        }
        // let (tx, rx) = mpsc::channel(buffer);
        Self {
            runtime,
            handlers: HashMap::new(),
            rxs,
            event_sender: EventSender { txs },
            keepalive_counter: KeepaliveCounter::new(),
        }
    }

    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }


    pub async fn run(&mut self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        // Flush any microtasks queued before run() (e.g. from eval_module).// 1. 启动前先清空一次积压的微任务（例如 eval_module 注入的同步代码）
        self.microtask_checkpoint(ctx).await?;

        // 2. 利用黑魔法宏，直接在栈上绑定并 Pin 住所有通道的接收端
        // 这样它们就可以安全、合法地塞进下面的 tokio::select! 块中
        pin_receivers!(self.rxs => {
            AfterMicro => rx_after_micro,
            Raf        => rx_raf,
            Macro      => rx_macro,
            Idle       => rx_idle
        });
        // 2. 声明提权定时器（初始化为遥远的未来，即不激活状态）
        let macro_boost_timer = tokio::time::sleep_until(TokioInstant::now() + Duration::from_secs(99999));
        let idle_boost_timer = tokio::time::sleep_until(TokioInstant::now() + Duration::from_secs(99999));
        tokio::pin!(macro_boost_timer);
        tokio::pin!(idle_boost_timer);

        // 记录低优先队列是否已经开始积压（用于计算首次积压时间）
        let mut macro_waiting_since: Option<Instant> = None;
        let mut idle_waiting_since: Option<Instant> = None;

        let max_wait_limit = Duration::from_millis(200); // 允许积压的最大时间，超过即提权

        loop {
            if self.should_exit() { break; }

            // 3. 动态维护提权时钟状态机
            // 检查 Macro 队列是否有积压
            if !rx_macro.is_empty() {
                if macro_waiting_since.is_none() {
                    macro_waiting_since = Some(Instant::now());
                    macro_boost_timer.as_mut().reset(TokioInstant::now() + max_wait_limit);
                }
            } else {
                macro_waiting_since = None;
                macro_boost_timer.as_mut().reset(TokioInstant::now() + Duration::from_secs(99999));
            }

            // 检查 Idle 队列是否有积压
            if !rx_idle.is_empty() {
                if idle_waiting_since.is_none() {
                    idle_waiting_since = Some(Instant::now());
                    idle_boost_timer.as_mut().reset(TokioInstant::now() + max_wait_limit);
                }
            } else {
                idle_waiting_since = None;
                idle_boost_timer.as_mut().reset(TokioInstant::now() + Duration::from_secs(99999));
            }

            // 4. 计算当前是否处于“已被提权强制插队”的状态
            let macro_is_boosted = macro_waiting_since.map_or(false, |t| t.elapsed() >= max_wait_limit);
            let idle_is_boosted = idle_waiting_since.map_or(false, |t| t.elapsed() >= max_wait_limit);

            // 5. 带有提权防线控制的偏向选择
            tokio::select! {
                biased;

                // 【分支 A：宏提权插队】
                // 如果 Macro 触发了超时提权，它直接篡位到第一优先级！
                Some(event) = rx_macro.recv(), if macro_is_boosted => {
                    self.dispatch_event(event.as_ref());
                }

                // 【分支 B：闲置提权插队】
                // 如果 Idle 触发了超时提权，它也篡位到极高优先级
                Some(event) = rx_idle.recv(), if idle_is_boosted => {
                    self.dispatch_event(event.as_ref());
                }

                // 【正常高优先级分支 1】只有在 Macro 和 Idle 都没有发起“强行提权插队”时，才允许跑
                Some(event) = rx_after_micro.recv(), if !macro_is_boosted && !idle_is_boosted => {
                    self.dispatch_event(event.as_ref());
                }
                
                // 【正常高优先级分支 2】
                Some(event) = rx_raf.recv(), if !macro_is_boosted && !idle_is_boosted => {
                    self.dispatch_event(event.as_ref());
                }
                
                // 【正常常规分支 3】如果没有被提权（普通轮询状态），Macro 依然在这里乖乖排队
                Some(event) = rx_macro.recv() => {
                    self.dispatch_event(event.as_ref());
                }
                
                // 【正常常规分支 4】如果没有被提权，Idle 在这里排队
                Some(event) = rx_idle.recv() => {
                    self.dispatch_event(event.as_ref());
                }

                // -------------------------------------------------------------
                // 【影子定时器分支】这两个分支纯粹是为了给 select! 提供中断唤醒源。
                // 当高频高优任务霸占 CPU 时，到了 200ms 的阈值，这两个定时器会 Ready 唤醒 select，
                // 从而强行打破僵局，进入下一轮循环去计算并激活上面的 if 插队守卫！
                // -------------------------------------------------------------
                _ = &mut macro_boost_timer, if macro_waiting_since.is_some() && !macro_is_boosted => {
                    // 仅用于把 select 晃醒，不需要写业务逻辑
                }
                _ = &mut idle_boost_timer, if idle_waiting_since.is_some() && !idle_is_boosted => {
                    // 仅用于把 select 晃醒
                }

                // 彻底没事干且 JS 也没微任务，进入真正的休眠
                _ = self.runtime.idle() => {}
                _ = self.keepalive_inner.wait_for_notification() => {
                    // 被砸醒后直接落地，select! 结束，下一轮循环一抬头就会撞上最上面的 `self.should_exit()` 完美退出！
                }
            }

            self.microtask_checkpoint(ctx).await?;
        }

        Ok(())
      
    }

    /// Drive the QJS job queue until it is empty.
    async fn microtask_checkpoint(&self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        ctx.with(|qctx| {
            while qctx.execute_pending_job() {}
            Ok::<(), rquickjs::Error>(())
        })
        .await
    }
   
    fn should_exit(&self) -> bool {
        self.rxs.values().all(|rx|  rx.is_empty())
            && self.keepalive_counter.count() <= 0
    }
}
