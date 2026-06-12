use markup5ever::QualName;
use super::Attributes;

#[derive(Debug, Clone)]
pub struct ElementData {
    pub name: QualName,
    pub attrs: Attributes,
}

impl ElementData {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter()
            .find(|a| &*a.name.local == name)
            .map(|a| a.value.as_str())
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.attr(name).is_some()
    }
}

#[derive(Debug, Clone)]
pub struct TextNodeData {
    pub content: String,
}

impl TextNodeData {
    pub fn new(content: String) -> Self {
        Self { content }
    }
}
