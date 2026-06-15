use std::borrow::Cow;

use html5ever::{
    parse_document,
    tendril::{StrTendril, TendrilSink},
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
    Attribute as H5Attr, QualName,
};
use markup5ever::{namespace_url, ns, ExpandedName, LocalName};

use super::node::NodeData;
use super::Document;

pub struct HtmlSink {
    doc: Document,
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
            _ => ExpandedName { ns: &ns!(), local: &LocalName::from("unknown") },
        }
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<H5Attr>, _flags: ElementFlags) -> usize {
        let id = self.doc.create_element(&name.local);
        for attr in attrs {
            self.doc.set_attribute(id, &attr.name.local, &attr.value);
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
