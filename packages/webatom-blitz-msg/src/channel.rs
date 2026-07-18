use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use crate::event::Event;
use crate::msg::DomMsg;
use crate::patch::DomOp;
use crate::snapshot::DomSnapshot;

type WakeFn = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

/// JS 线程持有：发 DomMsg、收 Event、收 RafTick
pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,
    pub event_rx: Receiver<Event>,
    /// bounded(1)：可跳帧，只需知道"有新帧"，不需要知道有几帧
    pub raf_rx:   Receiver<()>,
    wake_fn:      WakeFn,
}

/// Blitz 主线程持有：收 DomMsg、发 Event、发 RafTick
pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>,
    pub event_tx: Sender<Event>,
    /// bounded(1)：try_send 满则静默丢弃，天然跳帧
    pub raf_tx:   Sender<()>,
    wake_fn:      WakeFn,
}

/// vsync 时 drain_dom_msgs 的结构化结果
pub struct DrainResult {
    /// 需要执行的 DOM 操作
    pub dom_update: Option<DomUpdate>,
    /// 本批次布局完成后需异步回复的 QueryLayout node_id 列表
    pub layout_queries: Vec<usize>,
    /// 本批次布局完成后需发出的 LayoutNotify 数量（nextTick 回调计数）
    pub notify_count: usize,
}

/// DOM 更新模式
pub enum DomUpdate {
    Full(DomSnapshot),
    Patch(Vec<DomOp>),
}

/// 创建一对 channel 端点
pub fn create_channels() -> (JsSide, BlitzSide) {
    let (dom_tx, dom_rx)     = unbounded::<DomMsg>();
    let (event_tx, event_rx) = unbounded::<Event>();
    let (raf_tx, raf_rx)     = bounded::<()>(1);
    let wake_fn: WakeFn = Arc::new(Mutex::new(None));
    (
        JsSide   { dom_tx, event_rx, raf_rx, wake_fn: Arc::clone(&wake_fn) },
        BlitzSide { dom_rx, event_tx, raf_tx, wake_fn },
    )
}

impl JsSide {
    /// 阻塞等待下一个 Event（供 spawn_blocking 使用）
    pub fn recv_event(&self) -> Result<Event, crossbeam_channel::RecvError> {
        self.event_rx.recv()
    }

    /// 发送 DomMsg 并自动唤醒 Blitz winit 事件循环
    pub fn send_dom(&self, msg: DomMsg) {
        let _ = self.dom_tx.send(msg);
        if let Ok(g) = self.wake_fn.try_lock() {
            if let Some(f) = g.as_ref() { f(); }
        }
    }

    /// 非阻塞取出所有待处理 Event
    pub fn drain_events(&self) -> impl Iterator<Item = Event> + '_ {
        std::iter::from_fn(|| self.event_rx.try_recv().ok())
    }

    /// 非阻塞检查是否有待处理的 RafTick，消费掉并返回 true
    pub fn take_raf_tick(&self) -> bool {
        self.raf_rx.try_recv().is_ok()
    }

    /// 阻塞等待下一个 RafTick（供 spawn_blocking watcher 使用）
    pub fn recv_raf_tick(&self) {
        let _ = self.raf_rx.recv();
    }

    /// 唤醒 Blitz winit 事件循环（不发送任何消息，仅触发 about_to_wait）
    pub fn wake_blitz(&self) {
        if let Ok(g) = self.wake_fn.try_lock() {
            if let Some(f) = g.as_ref() { f(); }
        }
    }
}

impl BlitzSide {
    /// winit 事件循环创建后调用，注入唤醒回调
    pub fn set_wake_fn(&self, f: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut g) = self.wake_fn.lock() {
            *g = Some(Box::new(f));
        }
    }

    /// 投递用户事件到 JS 线程（不可丢）
    pub fn send_event(&self, event: Event) {
        let _ = self.event_tx.send(event);
    }

    /// vsync 后发出 RafTick；channel 已满（JS 尚未消费上一帧）则静默跳帧
    pub fn try_send_raf_tick(&self) {
        let _ = self.raf_tx.try_send(());
    }

    /// JS 侧是否有 rAF watcher 在等待 tick（raf_tx channel 为空 = JS 已消费上帧 tick）
    ///
    /// Blitz 可据此决定是否持续 request_redraw 驱动帧循环。
    pub fn has_raf_consumer(&self) -> bool {
        self.raf_tx.is_empty()
    }

    /// vsync 时调用：排干队列，分离 DOM 操作与布局请求。
    ///
    /// 返回 None 表示队列为空，无需渲染。
    /// Full 出现时清除之前所有 Patch，QueryLayout / LayoutNotifyRequest 始终被收集。
    pub fn drain_dom_msgs(&self) -> Option<DrainResult> {
        let mut dom_update: Option<DomUpdate> = None;
        let mut layout_queries: Vec<usize> = Vec::new();
        let mut notify_count: usize = 0;
        let mut any = false;

        while let Ok(msg) = self.dom_rx.try_recv() {
            any = true;
            match msg {
                DomMsg::Full(snap) => {
                    dom_update = Some(DomUpdate::Full(snap));
                }
                DomMsg::Patch(ops) => match &mut dom_update {
                    Some(DomUpdate::Patch(acc)) => acc.extend(ops),
                    // Full 之后同帧的 Patch 丢弃：Full 已是完整快照，
                    // JS 侧发 Full 时不应再发增量；若有遗漏下一 vsync 会补发。
                    Some(DomUpdate::Full(_)) => {}
                    None => dom_update = Some(DomUpdate::Patch(ops)),
                },
                DomMsg::QueryLayout { node_id } => {
                    layout_queries.push(node_id);
                }
                DomMsg::LayoutNotifyRequest => {
                    notify_count += 1;
                }
            }
        }

        if !any {
            return None;
        }

        Some(DrainResult { dom_update, layout_queries, notify_count })
    }
}
