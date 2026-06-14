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
    pub(super) inner: Weak<RefCell<DocumentInner>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for NodeHandle {
    type Changed<'to> = rquickjs::Class<'to, NodeHandle>;
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
        let Some(inner_rc) = self.inner.upgrade() else { return };
        let mut inner = inner_rc.borrow_mut();
        if let Some(node) = inner.doc.get_mut(self.id) {
            node.has_handle = false;
        }
        let is_detached = inner.doc.parent_node(self.id).is_none()
            && self.id != inner.doc.root();
        if is_detached {
            inner.doc.remove_node(self.id);
        }
    }
}
