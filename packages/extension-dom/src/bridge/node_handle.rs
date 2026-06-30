use std::cell::RefCell;
use std::rc::Weak;

use rquickjs::class::Trace;

use super::document_inner::DocumentInner;

#[derive(Trace)]
#[rquickjs::class(rename = "NodeHandle")]
pub struct NodeHandle {
    #[qjs(skip_trace)]
    pub id: usize,
    #[qjs(skip_trace)]
    pub(crate) inner: Weak<RefCell<DocumentInner>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for NodeHandle {
    type Changed<'to> = NodeHandle;
}

impl NodeHandle {
    pub(crate) fn new(id: usize, inner: Weak<RefCell<DocumentInner>>) -> Self {
        Self { id, inner }
    }
}

#[rquickjs::methods]
impl NodeHandle {

    #[qjs(get, rename = "nodeId")]
    pub fn node_id(&self) -> usize {
        self.id
    }
}

impl Drop for NodeHandle {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            let mut d = inner.borrow_mut();
            if let Some(node) = d.doc.get_mut(self.id) {
                node.has_handle = false;
            }
            let is_detached = d.doc.parent_node(self.id).is_none() && self.id != d.doc.root();
            if is_detached {
                d.doc.remove_node(self.id);
            }
        }
    }
}
