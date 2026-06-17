use std::collections::HashMap;

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, DocumentMutator};
use markup5ever::{LocalName, Namespace, QualName};

use super::snapshot::{DomSnapshot, SnapshotNode, SnapshotNodeData};

/// blitz-dom 中 document 根节点固定在 Slab index 0
const BLITZ_ROOT: usize = 0;

/// 管理一个 blitz-dom `BaseDocument`，按帧接收 `DomSnapshot` 并全量重建 DOM 树。
///
/// 阶段 1：全量 FullSync（每帧清空重建，足够简单正确）
/// 阶段 2：增量 diff（后续优化）
pub struct BlitzRenderer {
    doc: BaseDocument,
    /// webAtom node id → blitz node id 的映射（每帧重建时重置）
    id_map: HashMap<usize, usize>,
}

impl BlitzRenderer {
    pub fn new() -> Self {
        Self {
            doc: BaseDocument::new(DocumentConfig::default()),
            id_map: HashMap::new(),
        }
    }

    pub fn new_with_config(config: DocumentConfig) -> Self {
        Self {
            doc: BaseDocument::new(config),
            id_map: HashMap::new(),
        }
    }

    /// vsync 时调用：将快照全量应用到 blitz-dom，然后触发 flush（样式 + 布局计算）。
    pub fn apply_snapshot(&mut self, snap: &DomSnapshot) {
        self.id_map.clear();
        // webAtom document 根节点 → blitz 根节点
        self.id_map.insert(snap.root, BLITZ_ROOT);

        // 构建 wa_id → SnapshotNode 的查找表
        let node_map: HashMap<usize, &SnapshotNode> =
            snap.nodes.iter().map(|n| (n.id, n)).collect();

        let mut mutator = DocumentMutator::new(&mut self.doc);

        // 清空 blitz 根节点下的所有子节点
        mutator.remove_and_drop_all_children(BLITZ_ROOT);

        // 递归建树
        Self::build_children(
            &mut mutator,
            &mut self.id_map,
            snap.root,
            BLITZ_ROOT,
            &node_map,
        );

        mutator.flush();
    }

    fn build_children(
        mutator: &mut DocumentMutator<'_>,
        id_map: &mut HashMap<usize, usize>,
        wa_parent: usize,
        blitz_parent: usize,
        node_map: &HashMap<usize, &SnapshotNode>,
    ) {
        let Some(parent) = node_map.get(&wa_parent) else { return };

        let child_blitz_ids: Vec<usize> = parent
            .children
            .iter()
            .filter_map(|&wa_child| {
                let child = node_map.get(&wa_child)?;
                let blitz_id = Self::create_node(mutator, child);
                id_map.insert(wa_child, blitz_id);
                Some(blitz_id)
            })
            .collect();

        if !child_blitz_ids.is_empty() {
            mutator.append_children(blitz_parent, &child_blitz_ids);
        }

        for &wa_child in &parent.children {
            if let Some(&blitz_child) = id_map.get(&wa_child) {
                Self::build_children(mutator, id_map, wa_child, blitz_child, node_map);
            }
        }
    }

    fn create_node(mutator: &mut DocumentMutator<'_>, node: &SnapshotNode) -> usize {
        match &node.data {
            SnapshotNodeData::Document => BLITZ_ROOT,
            SnapshotNodeData::Element { tag, attrs } => {
                let qname = QualName::new(
                    None,
                    Namespace::from("http://www.w3.org/1999/xhtml"),
                    LocalName::from(tag.as_str()),
                );
                let blitz_attrs: Vec<Attribute> = attrs
                    .iter()
                    .map(|(name, value)| Attribute {
                        name: QualName::new(
                            None,
                            Namespace::from(""),
                            LocalName::from(name.as_str()),
                        ),
                        value: value.as_str().into(),
                    })
                    .collect();
                mutator.create_element(qname, blitz_attrs)
            }
            SnapshotNodeData::Text { content } => mutator.create_text_node(content),
            SnapshotNodeData::Comment { .. } => mutator.create_comment_node(),
        }
    }

    /// 返回对 blitz-dom `BaseDocument` 的引用，供 blitz-shell 渲染使用。
    pub fn document(&self) -> &BaseDocument {
        &self.doc
    }

    pub fn document_mut(&mut self) -> &mut BaseDocument {
        &mut self.doc
    }
}

impl Default for BlitzRenderer {
    fn default() -> Self {
        Self::new()
    }
}
