#[derive(Clone, Debug)]
pub enum DomOp {
    // ── 节点创建（JS 分配 id，先创建再插入）
    CreateElement { id: usize, tag: String, attrs: Vec<(String, String)> },
    CreateText    { id: usize, content: String },
    CreateComment { id: usize, content: String },

    // ── 树结构变更
    AppendChild  { parent: usize, child: usize },
    InsertBefore { parent: usize, child: usize, before: usize },
    /// 只从树中摘除，节点仍存在
    RemoveChild  { parent: usize, child: usize },
    /// 节点从 JS 侧彻底释放
    DropNode     { id: usize },

    // ── 属性变更（全量替换，Blitz 先清空再写入）
    ReplaceAttributes { node: usize, attrs: Vec<(String, String)> },

    // ── 文本内容变更
    SetTextContent { node: usize, content: String },
}
