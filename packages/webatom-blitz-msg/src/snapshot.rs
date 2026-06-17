#[derive(Clone, Debug)]
pub struct DomSnapshot {
    pub nodes: Vec<SnapshotNode>,
    pub root: usize,
}

#[derive(Clone, Debug)]
pub struct SnapshotNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub data: SnapshotNodeData,
}

#[derive(Clone, Debug)]
pub enum SnapshotNodeData {
    Document,
    Element { tag: String, attrs: Vec<(String, String)> },
    Text { content: String },
    Comment { content: String },
}
