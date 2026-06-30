use std::collections::HashMap;

use webatom_blitz_msg::patch::DomOp;

use crate::core::Document;

/// 帧内待发送的 DomOp 缓冲（Send + Sync，可安全移入 MacroTask 闭包）
///
/// 属性 / 文本采用"写时存快照"：每次变更后立即读取最终态写入，多次写同一节点
/// 以最后一次为准，drain 时无需再访问 DocumentInner。
pub(crate) struct PatchBuffer {
    structural_ops: Vec<DomOp>,
    dirty_attrs: HashMap<usize, Vec<(String, String)>>,
    dirty_text: HashMap<usize, String>,
}

impl PatchBuffer {
    pub fn new() -> Self {
        Self {
            structural_ops: Vec::new(),
            dirty_attrs: HashMap::new(),
            dirty_text: HashMap::new(),
        }
    }

    pub fn push_structural(&mut self, op: DomOp) {
        self.structural_ops.push(op);
    }

    pub fn mark_attrs(&mut self, node_id: usize, attrs: Vec<(String, String)>) {
        self.dirty_attrs.insert(node_id, attrs);
    }

    pub fn mark_text(&mut self, node_id: usize, content: String) {
        self.dirty_text.insert(node_id, content);
    }

    /// 取出本帧所有待发送的 DomOp（结构 op 在前，属性/文本 op 在后），清空缓冲
    pub fn drain_ops(&mut self) -> Vec<DomOp> {
        let mut ops = std::mem::take(&mut self.structural_ops);
        for (node_id, attrs) in self.dirty_attrs.drain() {
            ops.push(DomOp::ReplaceAttributes { node: node_id, attrs });
        }
        for (node_id, content) in self.dirty_text.drain() {
            ops.push(DomOp::SetTextContent { node: node_id, content });
        }
        ops
    }
}

pub(crate) struct DocumentInner {
    pub doc: Document,
}

impl DocumentInner {
    pub fn new_html(html: &str) -> Self {
        Self {
            doc: Document::parse_html(html),
        }
    }
}
