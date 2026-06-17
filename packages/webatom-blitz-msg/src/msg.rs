use crate::patch::DomOp;
use crate::snapshot::DomSnapshot;

#[derive(Clone, Debug)]
pub enum DomMsg {
    /// 首帧或强制重建：发送整棵树
    Full(DomSnapshot),
    /// 正常帧：一批有序的增量操作
    Patch(Vec<DomOp>),
}
