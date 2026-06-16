use super::{Attribute, ElementData, TextNodeData};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NodeKind {
    Element  = 1,
    Text     = 3,
    Comment  = 8,
    Document = 9,
    DocumentFragment = 11,
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Element(ElementData),
    Text(TextNodeData),
    Comment { contents: String },
    Fragment(ElementData)
}

impl NodeData {
    pub fn downcast_element(&self) -> Option<&ElementData> {
        match self {
            Self::Element(data) => Some(data),
            _ => None,
        }
    }

    pub fn downcast_element_mut(&mut self) -> Option<&mut ElementData> {
        match self {
            Self::Element(data) => Some(data),
            _ => None,
        }
    }

    pub fn is_element_with_tag_name(&self, tag: &str) -> bool {
        let Some(elem) = self.downcast_element() else { return false };
        &*elem.name.local == tag
    }

    pub fn attrs(&self) -> Option<&[Attribute]> {
        Some(&self.downcast_element()?.attrs)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.downcast_element()?.attr(name)
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.downcast_element().is_some_and(|e| e.has_attr(name))
    }

    pub fn kind(&self) -> NodeKind {
        match self {
            NodeData::Document    => NodeKind::Document,
            NodeData::Element(_)  => NodeKind::Element,
            NodeData::Text(_)     => NodeKind::Text,
            NodeData::Comment {..} => NodeKind::Comment,
            NodeData::Fragment(_) => NodeKind::DocumentFragment,
        }
    }
}

pub struct Node {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub data: NodeData,
    pub has_handle: bool,
}

