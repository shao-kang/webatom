use crate::core::Document;
use crate::core::node::NodeData;

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

/// Serialize the entire DOM tree into a `DomSnapshot`.
/// Only nodes reachable from `doc.root()` are included.
pub fn take_snapshot(doc: &Document) -> DomSnapshot {
    let root = doc.root();
    let mut nodes = Vec::new();

    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = doc.get(id) else { continue };

        let data = match &node.data {
            NodeData::Document => SnapshotNodeData::Document,
            NodeData::Element(e) => SnapshotNodeData::Element {
                tag: e.name.local.to_string(),
                attrs: doc.attributes_list(id),
            },
            NodeData::Text(t) => SnapshotNodeData::Text {
                content: t.content.clone(),
            },
            NodeData::Comment { contents } => SnapshotNodeData::Comment {
                content: contents.clone(),
            },
        };

        nodes.push(SnapshotNode {
            id,
            parent: node.parent,
            children: node.children.clone(),
            data,
        });

        for &child_id in node.children.iter().rev() {
            stack.push(child_id);
        }
    }

    DomSnapshot { nodes, root }
}
