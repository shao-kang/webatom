use std::sync::{Arc, Mutex};

use super::snapshot::DomSnapshot;

/// 共享槽：JS 线程覆盖写入，Blitz 线程 vsync 时 take 消费。
/// 槽内始终只保留最新快照，旧值被直接覆盖丢弃。
#[derive(Clone)]
pub struct DomSlot(Arc<Mutex<Option<DomSnapshot>>>);

impl DomSlot {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// JS 线程调用：写入最新快照，覆盖上一帧未消费的旧值。
    pub fn write(&self, snap: DomSnapshot) {
        *self.0.lock().unwrap() = Some(snap);
    }

    /// Blitz 线程调用（vsync）：取走快照，若无新快照则返回 None（跳过本帧重绘）。
    pub fn take(&self) -> Option<DomSnapshot> {
        self.0.lock().unwrap().take()
    }

    /// 标记 DOM 已变更但不立即序列化——用于脏标记检查。
    pub fn has_pending(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}

impl Default for DomSlot {
    fn default() -> Self {
        Self::new()
    }
}
