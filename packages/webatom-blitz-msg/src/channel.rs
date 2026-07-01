use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::event::Event;
use crate::msg::DomMsg;

type WakeFn = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

/// JS 线程持有：发 DomMsg、收 Event
pub struct JsSide {
    dom_tx:   Sender<DomMsg>,
    event_rx: Receiver<Event>,
    wake_fn:  WakeFn,
}

/// Blitz 主线程持有：收 DomMsg、发 Event
pub struct BlitzSide {
    dom_rx:   Receiver<DomMsg>,
    event_tx: Sender<Event>,
    wake_fn:  WakeFn,
}

/// 创建一对 channel 端点
pub fn create_channels() -> (JsSide, BlitzSide) {
    let (dom_tx, dom_rx)     = unbounded::<DomMsg>();
    let (event_tx, event_rx) = unbounded::<Event>();
    let wake_fn: WakeFn = Arc::new(Mutex::new(None));
    (
        JsSide   { dom_tx, event_rx, wake_fn: Arc::clone(&wake_fn) },
        BlitzSide { dom_rx, event_tx, wake_fn },
    )
}

impl JsSide {
    /// 阻塞等待下一个 Event（供 spawn_blocking 使用）。
    pub fn recv_event(&self) -> Result<Event, crossbeam_channel::RecvError> {
        self.event_rx.recv()
    }

    /// 发送 DomMsg 并自动唤醒 Blitz winit 事件循环。
    pub fn send_dom(&self, msg: DomMsg) {
        let _ = self.dom_tx.send(msg);
        if let Ok(g) = self.wake_fn.try_lock() {
            if let Some(f) = g.as_ref() { f(); }
        }
    }

    /// 非阻塞取出所有待处理 Event。
    pub fn drain_events(&self) -> impl Iterator<Item = Event> + '_ {
        std::iter::from_fn(|| self.event_rx.try_recv().ok())
    }
}

impl BlitzSide {
    /// winit 事件循环创建后调用，注入唤醒回调。
    pub fn set_wake_fn(&self, f: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut g) = self.wake_fn.lock() {
            *g = Some(Box::new(f));
        }
    }

    /// 投递用户事件到 JS 线程。
    pub fn send_event(&self, event: Event) {
        let _ = self.event_tx.send(event);
    }

    /// vsync 时调用：排干队列，合并 Patch；Full 出现时丢弃其之前所有消息。
    pub fn drain_dom_msgs(&self) -> Vec<DomMsg> {
        let mut result: Vec<DomMsg> = Vec::new();
        while let Ok(msg) = self.dom_rx.try_recv() {
            match msg {
                DomMsg::Full(snap) => {
                    result.clear();
                    result.push(DomMsg::Full(snap));
                }
                DomMsg::Patch(ops) => match result.last_mut() {
                    Some(DomMsg::Patch(acc)) => acc.extend(ops),
                    _ => result.push(DomMsg::Patch(ops)),
                },
            }
        }
        result
    }
}
