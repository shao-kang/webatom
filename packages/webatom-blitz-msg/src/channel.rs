use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::event::BlitzEvent;
use crate::msg::DomMsg;

type WakeFn = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

/// JS 线程持有：发 DomMsg、收 BlitzEvent
pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,
    pub event_rx: Receiver<BlitzEvent>,
    wake_fn: WakeFn,
}

/// Blitz 主线程持有：收 DomMsg、发 BlitzEvent
pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>,
    pub event_tx: Sender<BlitzEvent>,
    wake_fn: WakeFn,
}

/// 创建一对 channel 端点
pub fn create_channels() -> (JsSide, BlitzSide) {
    let (dom_tx, dom_rx)     = unbounded::<DomMsg>();
    let (event_tx, event_rx) = unbounded::<BlitzEvent>();
    let wake_fn: WakeFn = Arc::new(Mutex::new(None));
    (
        JsSide   { dom_tx, event_rx, wake_fn: Arc::clone(&wake_fn) },
        BlitzSide { dom_rx, event_tx, wake_fn },
    )
}

impl BlitzSide {
    /// winit 事件循环创建后调用，注入唤醒回调；JS 发送 DomMsg 后会调用此函数唤醒 Blitz 主线程。
    pub fn set_wake_fn(&self, f: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut g) = self.wake_fn.lock() {
            *g = Some(Box::new(f));
        }
    }

    /// vsync 时调用：排干队列，合并 Patch；Full 出现时丢弃其之前的所有消息。
    ///
    /// 返回值：`[Full(snap), Patch(merged_ops), ...]`，调用方按序 apply。
    pub fn drain_dom_msgs(&self) -> Vec<DomMsg> {
        let mut result: Vec<DomMsg> = Vec::new();

        while let Ok(msg) = self.dom_rx.try_recv() {
            match msg {
                // Full 出现时，丢弃之前所有已积累的消息
                DomMsg::Full(snap) => {
                    result.clear();
                    result.push(DomMsg::Full(snap));
                }
                DomMsg::Patch(ops) => match result.last_mut() {
                    // 与上一条 Patch 合并
                    Some(DomMsg::Patch(acc)) => acc.extend(ops),
                    // Full 之后的 Patch 或首条 Patch
                    _ => result.push(DomMsg::Patch(ops)),
                },
            }
        }

        result
    }
}

impl JsSide {
    /// 发送 DomMsg 并自动唤醒 Blitz winit 事件循环。
    pub fn send(&self, msg: DomMsg) {
        let _ = self.dom_tx.send(msg);
        if let Ok(g) = self.wake_fn.try_lock() {
            if let Some(f) = g.as_ref() { f(); }
        }
    }

    /// 事件循环 idle 阶段调用：非阻塞地取出所有待处理 BlitzEvent。
    pub fn drain_events(&self) -> impl Iterator<Item = BlitzEvent> + '_ {
        std::iter::from_fn(|| self.event_rx.try_recv().ok())
    }
}
