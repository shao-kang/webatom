use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::event::BlitzEvent;
use crate::msg::DomMsg;

/// JS 线程持有：发 DomMsg、收 BlitzEvent
pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,
    pub event_rx: Receiver<BlitzEvent>,
}

/// Blitz 主线程持有：收 DomMsg、发 BlitzEvent
pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>,
    pub event_tx: Sender<BlitzEvent>,
}

/// 创建一对 channel 端点
pub fn create_channels() -> (JsSide, BlitzSide) {
    let (dom_tx, dom_rx)       = unbounded::<DomMsg>();
    let (event_tx, event_rx)   = unbounded::<BlitzEvent>();
    (
        JsSide   { dom_tx,   event_rx },
        BlitzSide { dom_rx,  event_tx },
    )
}

impl BlitzSide {
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
    /// 事件循环 idle 阶段调用：非阻塞地取出所有待处理 BlitzEvent。
    pub fn drain_events(&self) -> impl Iterator<Item = BlitzEvent> + '_ {
        std::iter::from_fn(|| self.event_rx.try_recv().ok())
    }
}
