use std::collections::HashMap;

use blitz_dom::{Attribute, BaseDocument, DocumentMutator};
use markup5ever::{LocalName, Namespace, QualName};
use webatom_blitz_msg::{DomOp, DomSnapshot, SnapshotNode, SnapshotNodeData};

const BLITZ_ROOT: usize = 0;

fn elem_qname(tag: &str) -> QualName {
    QualName::new(
        None,
        Namespace::from("http://www.w3.org/1999/xhtml"),
        LocalName::from(tag),
    )
}

fn attr_qname(name: &str) -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(name))
}

fn to_blitz_attrs(attrs: &[(String, String)]) -> Vec<Attribute> {
    attrs
        .iter()
        .map(|(name, value)| Attribute {
            name: attr_qname(name),
            value: value.as_str().into(),
        })
        .collect()
}

/// 全量重建：清空 blitz root，按 DomSnapshot 重建整棵树
pub fn apply_full(doc: &mut BaseDocument, id_map: &mut HashMap<usize, usize>, snap: &DomSnapshot) {
    id_map.clear();
    id_map.insert(snap.root, BLITZ_ROOT);

    let node_lookup: HashMap<usize, &SnapshotNode> =
        snap.nodes.iter().map(|n| (n.id, n)).collect();

    let mut mutator = DocumentMutator::new(doc);
    mutator.remove_and_drop_all_children(BLITZ_ROOT);
    build_children(&mut mutator, id_map, snap.root, BLITZ_ROOT, &node_lookup);
    // Drop → auto flush
}

fn build_children(
    mutator: &mut DocumentMutator<'_>,
    id_map: &mut HashMap<usize, usize>,
    wa_parent: usize,
    blitz_parent: usize,
    node_lookup: &HashMap<usize, &SnapshotNode>,
) {
    let Some(parent) = node_lookup.get(&wa_parent) else {
        return;
    };

    let child_blitz_ids: Vec<usize> = parent
        .children
        .iter()
        .filter_map(|&wa_child| {
            let child = node_lookup.get(&wa_child)?;
            let blitz_id = create_blitz_node(mutator, child);
            id_map.insert(wa_child, blitz_id);
            Some(blitz_id)
        })
        .collect();

    if !child_blitz_ids.is_empty() {
        mutator.append_children(blitz_parent, &child_blitz_ids);
    }

    for &wa_child in &parent.children {
        if let Some(&blitz_child) = id_map.get(&wa_child) {
            build_children(mutator, id_map, wa_child, blitz_child, node_lookup);
        }
    }
}

fn create_blitz_node(mutator: &mut DocumentMutator<'_>, node: &SnapshotNode) -> usize {
    match &node.data {
        SnapshotNodeData::Document => BLITZ_ROOT,
        SnapshotNodeData::Element { tag, attrs } => {
            mutator.create_element(elem_qname(tag), to_blitz_attrs(attrs))
        }
        SnapshotNodeData::Text { content } => mutator.create_text_node(content),
        SnapshotNodeData::Comment { .. } => mutator.create_comment_node(),
    }
}

/// 增量更新：按 DomOp 顺序调用 DocumentMutator，维护 id_map
pub fn apply_patch(doc: &mut BaseDocument, id_map: &mut HashMap<usize, usize>, ops: &[DomOp]) {
    let mut mutator = DocumentMutator::new(doc);
    for op in ops {
        apply_op(&mut mutator, id_map, op);
    }
    // Drop → auto flush
}

fn apply_op(mutator: &mut DocumentMutator<'_>, id_map: &mut HashMap<usize, usize>, op: &DomOp) {
    match op {
        DomOp::CreateElement { id, tag, attrs } => {
            let blitz_id = mutator.create_element(elem_qname(tag), to_blitz_attrs(attrs));
            id_map.insert(*id, blitz_id);
        }
        DomOp::CreateText { id, content } => {
            let blitz_id = mutator.create_text_node(content);
            id_map.insert(*id, blitz_id);
        }
        DomOp::CreateComment { id, .. } => {
            let blitz_id = mutator.create_comment_node();
            id_map.insert(*id, blitz_id);
        }
        DomOp::AppendChild { parent, child } => {
            let (Some(&blitz_parent), Some(&blitz_child)) =
                (id_map.get(parent), id_map.get(child))
            else {
                return;
            };
            mutator.append_children(blitz_parent, &[blitz_child]);
        }
        DomOp::InsertBefore { child, before, .. } => {
            let (Some(&blitz_child), Some(&blitz_before)) =
                (id_map.get(child), id_map.get(before))
            else {
                return;
            };
            mutator.insert_nodes_before(blitz_before, &[blitz_child]);
        }
        DomOp::RemoveChild { child, .. } => {
            if let Some(&blitz_child) = id_map.get(child) {
                mutator.remove_node(blitz_child);
            }
        }
        DomOp::DropNode { id } => {
            if let Some(blitz_id) = id_map.remove(id) {
                mutator.remove_and_drop_node(blitz_id);
            }
        }
        DomOp::ReplaceAttributes { node, attrs } => {
            // 全量替换：覆写新属性列表中的每个属性。
            // TODO(Phase 2)：先清除节点上不在新列表中的旧属性。
            if let Some(&blitz_id) = id_map.get(node) {
                for (name, value) in attrs {
                    mutator.set_attribute(blitz_id, attr_qname(&name), &value);
                }
            }
        }
        DomOp::SetTextContent { node, content } => {
            if let Some(&blitz_id) = id_map.get(node) {
                mutator.set_node_text(blitz_id, content);
            }
        }
    }
}
