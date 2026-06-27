use webatom_blitz_msg::snapshot::DomSnapshot;

use super::document_handle::DocumentHandle;

impl DocumentHandle {
    /// 将当前 Document 序列化为 DomSnapshot，用于首帧全量发送。
    pub fn to_snapshot(&self) -> DomSnapshot {
        self.inner.borrow().doc.to_snapshot()
    }
}
