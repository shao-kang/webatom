use std::cell::RefCell;
use std::rc::Weak;

use rquickjs::class::Trace;

use super::DocumentHandle;

#[derive(Trace)]
#[rquickjs::class(rename = "NodeHandle")]
pub struct NodeHandle {
    #[qjs(skip_trace)]
    pub id: usize,
    #[qjs(skip_trace)]
    doc: Weak<RefCell<DocumentHandle>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for NodeHandle {
    type Changed<'to> = NodeHandle;
}

#[rquickjs::methods]
impl NodeHandle {
    #[qjs(get, rename = "nodeId")]
    pub fn node_id(&self) -> usize {
        self.id
    }
}

impl NodeHandle {
    pub fn new(id: usize, doc: Weak<RefCell<Document>>) -> Self {
        if let Some(d) = doc.upgrade() {
            if let Some(node) = d.borrow_mut().get_mut(id) {
                node.has_handle = true;
            }
        }
        Self { id, doc }
    }
}

impl Drop for NodeHandle {
    fn drop(&mut self) {
        if let Some(doc) = self.doc.upgrade() {
            let mut d = doc.borrow_mut();
            if let Some(node) = d.get_mut(self.id) {
                node.has_handle = false;
            }
            // 仅在脱离树时释放，在树内由 removeChild 时 DomRuntime 负责清理
            let is_detached = d.parent_node(self.id).is_none() && self.id != d.root();
            if is_detached {
                d.remove_node(self.id);
            }
        }
    }
}
