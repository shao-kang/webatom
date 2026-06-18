use slab::Slab;
use markup5ever::{LocalName, Namespace, QualName};

use super::parse_html;

use super::{Attributes, ElementData, Node, TextNodeData};
use super::node::NodeData;

pub struct Document {
    nodes: Slab<Node>,
    root: usize,
    pub document_element: Option<usize>,
    pub body: Option<usize>,
    pub head: Option<usize>,
    

}

impl Document {
    pub fn new() -> Self {
        let mut nodes = Slab::new();
        let entry = nodes.vacant_entry();
        let root = entry.key();
        entry.insert(Node {
            id: root,
            parent: None,
            children: Vec::new(),
            data: NodeData::Document,
            has_handle: false,
        });
        Self { nodes, root, body: None, head: None, document_element: None }
    }
    pub fn parse_html(html: &str) -> Self {
        parse_html(html)
    }
    pub fn root(&self) -> usize {
        self.root
    }

    pub fn get(&self, id: usize) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    // ── 节点创建 ────────────────────────────────────────────────────────────

    pub fn create_element(&mut self, tag: &str) -> usize {
        let name = QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from(tag));
        let entry = self.nodes.vacant_entry();
        let id = entry.key();
        entry.insert(Node {
            id,
            parent: None,
            children: Vec::new(),
            data: NodeData::Element(ElementData {
                name,
                attrs: Attributes::new(Vec::new()),
            }),
            has_handle: false,
        });
        id
    }

    pub fn create_text_node(&mut self, content: &str) -> usize {
        let entry = self.nodes.vacant_entry();
        let id = entry.key();
        entry.insert(Node {
            id,
            parent: None,
            children: Vec::new(),
            data: NodeData::Text(TextNodeData::new(content.to_owned())),
            has_handle: false,
        });
        id
    }

    pub fn create_comment(&mut self, contents: &str) -> usize {
        let entry = self.nodes.vacant_entry();
        let id = entry.key();
        entry.insert(Node {
            id,
            parent: None,
            children: Vec::new(),
            data: NodeData::Comment { contents: contents.to_owned() },
            has_handle: false,
        });
        id
    }

    // ── 树操作 ──────────────────────────────────────────────────────────────

    pub fn append_child(&mut self, parent_id: usize, child_id: usize) {
        if let Some(old) = self.nodes[child_id].parent {
            self.nodes[old].children.retain(|&c| c != child_id);
        }
        self.nodes[child_id].parent = Some(parent_id);
        self.nodes[parent_id].children.push(child_id);
    }

    pub fn remove_child(&mut self, parent_id: usize, child_id: usize) {
        if self.nodes.get(child_id).and_then(|n| n.parent) != Some(parent_id) {
            return;
        }
        self.nodes[parent_id].children.retain(|&c| c != child_id);
        self.nodes[child_id].parent = None;
    }

    pub fn insert_before(&mut self, parent_id: usize, new_id: usize, ref_id: usize) {
        if let Some(old) = self.nodes[new_id].parent {
            self.nodes[old].children.retain(|&c| c != new_id);
        }
        let pos = self.nodes[parent_id].children.iter().position(|&c| c == ref_id)
            .unwrap_or_else(|| self.nodes[parent_id].children.len());
        self.nodes[parent_id].children.insert(pos, new_id);
        self.nodes[new_id].parent = Some(parent_id);
    }

    // ── 节点查询 ────────────────────────────────────────────────────────────

    pub fn parent_node(&self, id: usize) -> Option<usize> {
        self.nodes.get(id)?.parent
    }

    pub fn first_child(&self, id: usize) -> Option<usize> {
        self.nodes.get(id)?.children.first().copied()
    }

    pub fn last_child(&self, id: usize) -> Option<usize> {
        self.nodes.get(id)?.children.last().copied()
    }

    pub fn next_sibling(&self, id: usize) -> Option<usize> {
        let parent_id = self.nodes.get(id)?.parent?;
        let children = &self.nodes[parent_id].children;
        let pos = children.iter().position(|&c| c == id)?;
        children.get(pos + 1).copied()
    }

    pub fn previous_sibling(&self, id: usize) -> Option<usize> {
        let parent_id = self.nodes.get(id)?.parent?;
        let children = &self.nodes[parent_id].children;
        let pos = children.iter().position(|&c| c == id)?;
        pos.checked_sub(1).map(|i| children[i])
    }

    pub fn node_type(&self, id: usize) -> Option<u16> {
        Some(self.nodes.get(id)?.data.kind() as u16)
    }

    pub fn tag_name(&self, id: usize) -> Option<String> {
        match &self.nodes.get(id)?.data {
            NodeData::Element(e) => Some(e.name.local.to_uppercase()),
            _ => None,
        }
    }

    pub fn node_value(&self, id: usize) -> Option<String> {
        match &self.nodes.get(id)?.data {
            NodeData::Text(t)              => Some(t.content.clone()),
            NodeData::Comment { contents } => Some(contents.clone()),
            _ => None,
        }
    }

    pub fn set_node_value(&mut self, id: usize, value: &str) {
        if let Some(node) = self.nodes.get_mut(id) {
            match &mut node.data {
                NodeData::Text(t)              => t.content = value.to_owned(),
                NodeData::Comment { contents } => *contents = value.to_owned(),
                _ => {}
            }
        }
    }

    // ── 属性读写 ────────────────────────────────────────────────────────────

    pub fn get_attribute(&self, id: usize, name: &str) -> Option<String> {
        self.nodes.get(id)?.data.attr(name).map(|s| s.to_owned())
    }

    pub fn set_attribute(&mut self, id: usize, name: &str, value: &str) {
        if let Some(elem) = self.nodes.get_mut(id).and_then(|n| n.data.downcast_element_mut()) {
            let qname = QualName::new(None, Namespace::from(""), LocalName::from(name));
            elem.attrs.set(qname, value);
        }
    }

    pub fn remove_attribute(&mut self, id: usize, name: &str) {
        if let Some(elem) = self.nodes.get_mut(id).and_then(|n| n.data.downcast_element_mut()) {
            let qname = QualName::new(None, Namespace::from(""), LocalName::from(name));
            elem.attrs.remove(&qname);
        }
    }

    pub fn has_attribute(&self, id: usize, name: &str) -> bool {
        self.nodes.get(id).map(|n| n.data.has_attr(name)).unwrap_or(false)
    }

    pub fn attributes_list(&self, id: usize) -> Vec<(String, String)> {
        self.nodes.get(id)
            .and_then(|n| n.data.attrs())
            .map(|attrs| attrs.iter().map(|a| (a.name.local.to_string(), a.value.clone())).collect())
            .unwrap_or_default()
    }

    pub fn remove_node(&mut self, id: usize) {
        self.nodes.try_remove(id);
    }

    pub fn node_ids(&self) -> Vec<usize> {
        self.nodes.iter().map(|(id, _)| id).collect()
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    // pub fn from_html(html: &str) -> Self {
    //     super::parse_html(html)
    // }

    pub fn script_contents(&self) -> Vec<String> {
        let mut scripts = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            if let Some(node) = self.nodes.get(id) {
                if let NodeData::Element(e) = &node.data {
                    if e.name.local.as_ref().eq_ignore_ascii_case("script") {
                        let text: String = node.children.iter()
                            .filter_map(|&child_id| {
                                self.nodes.get(child_id).and_then(|n| {
                                    if let NodeData::Text(t) = &n.data { Some(t.content.as_str()) }
                                    else { None }
                                })
                            })
                            .collect();
                        if !text.trim().is_empty() {
                            scripts.push(text);
                        }
                    }
                }
                stack.extend(node.children.iter().rev().copied());
            }
        }
        scripts
    }
}
