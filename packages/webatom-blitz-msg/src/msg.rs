use crate::patch::DomOp;
use crate::snapshot::DomSnapshot;

#[derive(Clone, Debug)]
pub enum DomMsg {
    /// 首帧或强制重建时发送整棵树
    Full(DomSnapshot),
    /// 正常帧：一批有序的增量操作
    Patch(Vec<DomOp>),
    /// 异步查询指定节点布局；Blitz 完成本批次布局后通过 Event::LayoutResult 回调
    QueryLayout { node_id: usize },
    /// nextTick 语义：Blitz 处理完当前队列所有 patch 并完成布局后发 Event::LayoutNotify 回调
    LayoutNotifyRequest,
}
