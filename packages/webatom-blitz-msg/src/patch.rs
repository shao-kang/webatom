#[derive(Clone, Debug)]
pub enum DomOp {
    // ── 节点创建（JS 分配 id，先创建再插入）
    CreateElement { id: usize, tag: String, attrs: Vec<(String, String)> },
    CreateText    { id: usize, content: String },
    CreateComment { id: usize, content: String },

    // ── 树结构变更
    AppendChild  { parent: usize, child: usize },
    InsertBefore { parent: usize, child: usize, before: usize },
    /// 只从树中摘除，节点仍存在（对应 Node.removeChild）
    RemoveChild  { parent: usize, child: usize },
    /// JS 侧彻底释放节点（无任何 JS 引用）
    DropNode     { id: usize },

    // ── 属性变更
    SetAttribute    { node: usize, name: String, value: String },
    SetAttributes   { node: usize, attrs: Vec<(String, String)> },
    RemoveAttribute { node: usize, name: String },

    // ── 文本内容变更（CharacterData.data）
    SetTextContent { node: usize, content: String },
}
