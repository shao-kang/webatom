use std::collections::HashMap;

use rquickjs::{Persistent, Value};

use crate::core::Document;

pub(crate) struct DocumentInner {
    pub doc: Document,
    /// 每个节点对应的 JS `WeakRef` 对象（Persistent 持有 WeakRef，但 WeakRef 不阻止 NodeHandle 被 GC 回收）
    pub node_handles: HashMap<usize, Persistent<Value<'static>>>,
}

impl DocumentInner {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            node_handles: HashMap::new(),
        }
    }
    pub fn new_html(html: &str) -> Self {
         Self {
            doc: Document::parse_html(html),
            node_handles: HashMap::new(),
        }

    }
}
