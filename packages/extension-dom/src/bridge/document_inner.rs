use std::collections::HashMap;

use rquickjs::{Persistent, Value};

use crate::core::Document;

pub(crate) struct DocumentInner {
    pub doc: Document,
    pub node_handles: HashMap<usize, Persistent<Value<'static>>>,
}

impl DocumentInner {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            node_handles: HashMap::new(),
        }
    }
}
