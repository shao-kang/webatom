use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::event::BlitzEvent;
use crate::msg::DomMsg;

pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,
    pub event_rx: Receiver<BlitzEvent>,
}

pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>,
    pub event_tx: Sender<BlitzEvent>,
}

pub fn create_channels() -> (JsSide, BlitzSide) {
    let (dom_tx, dom_rx) = unbounded::<DomMsg>();
    let (event_tx, event_rx) = unbounded::<BlitzEvent>();
    (
        JsSide   { dom_tx, event_rx },
        BlitzSide { dom_rx, event_tx },
    )
}

impl BlitzSide {
    /// vsync 时调用：排干队列，合并连续 Patch，Full 出现时丢弃其之前的所有消息。
    ///
    /// 返回有序消息列表，调用方按顺序 apply：
    ///   [Patch(a), Patch(b), Full(snap), Patch(c)] → [Full(snap), Patch(c)]
    pub fn drain_dom_msgs(&self) -> Vec<DomMsg> {
        let mut result: Vec<DomMsg> = Vec::new();

        while let Ok(msg) = self.dom_rx.try_recv() {
            match msg {
                DomMsg::Full(snap) => {
                    // Full 是完整树状态，丢弃之前累积的所有消息
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
