use std::borrow::Cow;

use html5ever::{
    parse_document,
    parse_fragment as html5_parse_fragment,
    tendril::{StrTendril, TendrilSink},
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
    Attribute as H5Attr, QualName,
};
use markup5ever::{namespace_url, ns, ExpandedName, LocalName, Namespace};

use super::node::NodeData;
use super::Document;

pub struct HtmlSink {
    pub doc: Document,
}
impl HtmlSink {
    pub fn fragment(document: Document) -> Self {
        Self { doc: document }
    }

}

impl Default for HtmlSink {
    fn default() -> Self {
        Self { doc: Document::new() }
    }
    
}

impl TreeSink for HtmlSink {
    type Handle = usize;
    type Output = Document;

    fn finish(self) -> Document {
        self.doc
    }

    fn parse_error(&mut self, _msg: Cow<'static, str>) {}

    fn get_document(&mut self) -> usize {
        self.doc.root()
    }

    fn set_quirks_mode(&mut self, _mode: QuirksMode) {}

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> ExpandedName<'a> {
        match self.doc.get(*target).map(|n| &n.data) {
            Some(NodeData::Element(e)) => e.name.expanded(),
            _ => {
                use std::sync::OnceLock;
                static NS: OnceLock<markup5ever::Namespace> = OnceLock::new();
                static LOCAL: OnceLock<LocalName> = OnceLock::new();
                ExpandedName {
                    ns: NS.get_or_init(|| ns!()),
                    local: LOCAL.get_or_init(|| LocalName::from("unknown")),
                }
            }
        }
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<H5Attr>, _flags: ElementFlags) -> usize {
        let id = self.doc.create_element(&name.local);
        for attr in attrs {
            self.doc.set_attribute(id, &attr.name.local, &attr.value);
        }
        let tag = name.local.to_string();
        match tag.as_str() {
            "html" => {
                self.doc.document_element = Some(id);
            }
            "head" => {
                self.doc.head = Some(id);
            }
            "body" => {
                self.doc.body = Some(id);
            }
            _ => {}
        }
        id
    }

    fn create_comment(&mut self, text: StrTendril) -> usize {
        self.doc.create_comment(&text)
    }

    fn create_pi(&mut self, _target: StrTendril, _data: StrTendril) -> usize {
        self.doc.create_comment("")
    }

    fn append(&mut self, parent: &usize, child: NodeOrText<usize>) {
        let child_id = match child {
            NodeOrText::AppendNode(id) => id,
            NodeOrText::AppendText(text) => self.doc.create_text_node(&text),
        };
        self.doc.append_child(*parent, child_id);
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        if self.doc.get(*element).and_then(|n| n.parent).is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &mut self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    fn get_template_contents(&mut self, target: &usize) -> usize {
        *target
    }

    fn mark_script_already_started(&mut self, _node: &usize) {}

    fn pop(&mut self, _node: &usize) {}

    fn remove_from_parent(&mut self, target: &usize) {
        if let Some(parent_id) = self.doc.get(*target).and_then(|n| n.parent) {
            self.doc.remove_child(parent_id, *target);
        }
    }

    fn reparent_children(&mut self, node: &usize, new_parent: &usize) {
        let children: Vec<usize> = self.doc.get(*node)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.doc.remove_child(*node, child);
            self.doc.append_child(*new_parent, child);
        }
    }

    fn append_before_sibling(&mut self, sibling: &usize, new_node: NodeOrText<usize>) {
        let child_id = match new_node {
            NodeOrText::AppendNode(id) => id,
            NodeOrText::AppendText(text) => self.doc.create_text_node(&text),
        };
        if let Some(parent_id) = self.doc.get(*sibling).and_then(|n| n.parent) {
            self.doc.insert_before(parent_id, child_id, *sibling);
        }
    }

    fn add_attrs_if_missing(&mut self, target: &usize, attrs: Vec<H5Attr>) {
        for attr in attrs {
            if self.doc.get_attribute(*target, &attr.name.local).is_none() {
                self.doc.set_attribute(*target, &attr.name.local, &attr.value);
            }
        }
    }

    fn set_current_line(&mut self, _line_number: u64) {}
}

pub fn parse_html(html: &str) -> Document {
    let sink = HtmlSink::default();
    parse_document(sink, Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap()
}

/// 在已有 `doc` 上解析 `html` 片段（以 `context_tag` 为上下文），
/// 返回 (文档根节点id, 本次新增的节点id列表)
pub fn parse_fragment(doc: &mut Document, html: &str, context_tag: &str) -> (usize, Vec<usize>) {
    let existing: std::collections::HashSet<usize> = doc.node_ids().into_iter().collect();
    let root = doc.root();

    let sink = HtmlSink::fragment(std::mem::take(doc));
    let context_name = QualName::new(
        None,
        Namespace::from("http://www.w3.org/1999/xhtml"),
        LocalName::from(context_tag),
    );
    *doc = html5_parse_fragment(sink, Default::default(), context_name, vec![])
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();

    let new_ids: Vec<usize> = doc.node_ids()
        .into_iter()
        .filter(|id| !existing.contains(id))
        .collect();

    (root, new_ids)
}

pub fn parse_fragment_body(doc: &mut Document, html: &str) -> (usize, Vec<usize>) {
    parse_fragment(doc, html, "body")
}


