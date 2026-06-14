use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{class::Trace, Class, Ctx, Persistent, Result, Value};

use super::document_inner::DocumentInner;
use super::node_handle::NodeHandle;

#[derive(Trace)]
#[rquickjs::class(rename = "DocumentHandle")]
pub struct DocumentHandle {
    #[qjs(skip_trace)]
    pub(crate) inner: Rc<RefCell<DocumentInner>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for DocumentHandle {
    type Changed<'to> = rquickjs::Class<'to, DocumentHandle>;
}

impl DocumentHandle {
    pub fn new() -> Self {
        Self { inner: Rc::new(RefCell::new(DocumentInner::new())) }
    }

    fn get_or_create<'js>(&self, ctx: Ctx<'js>, id: usize) -> Result<Value<'js>> {
        if let Some(p) = self.inner.borrow().node_handles.get(&id).cloned() {
            return p.restore(&ctx);
        }
        if let Some(node) = self.inner.borrow_mut().doc.get_mut(id) {
            node.has_handle = true;
        }
        let handle = NodeHandle {
            id,
            inner: Rc::downgrade(&self.inner),
        };
        let val: Value<'js> = Class::instance(ctx.clone(), handle)?.into_value();
        self.inner.borrow_mut().node_handles.insert(id, Persistent::save(&ctx, val.clone()));
        Ok(val)
    }

    fn remove_subtree(&self, id: usize) {
        let children = self.inner.borrow().doc.get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        self.inner.borrow_mut().node_handles.remove(&id);
    }
}

#[rquickjs::methods]
impl DocumentHandle {
    #[qjs(constructor)]
    pub fn js_new() -> Self {
        Self::new()
    }

    pub fn create_element<'js>(&self, ctx: Ctx<'js>, tag: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_element(&tag);
        self.get_or_create(ctx, id)
    }

    pub fn create_text_node<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_text_node(&content);
        self.get_or_create(ctx, id)
    }

    pub fn create_comment<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_comment(&content);
        self.get_or_create(ctx, id)
    }

    pub fn document_element<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let id = self.inner.borrow().doc.root();
        self.get_or_create(ctx, id)
    }

    pub fn append_child(
        &self,
        parent: Class<'_, NodeHandle>,
        child: Class<'_, NodeHandle>,
    ) -> Result<()> {
        let parent_id = parent.borrow().id;
        let child_id = child.borrow().id;
        self.inner.borrow_mut().doc.append_child(parent_id, child_id);
        Ok(())
    }

    pub fn remove_child(
        &self,
        parent: Class<'_, NodeHandle>,
        child: Class<'_, NodeHandle>,
    ) -> Result<()> {
        let parent_id = parent.borrow().id;
        let child_id = child.borrow().id;
        self.inner.borrow_mut().doc.remove_child(parent_id, child_id);
        self.remove_subtree(child_id);
        Ok(())
    }

    pub fn insert_before(
        &self,
        parent: Class<'_, NodeHandle>,
        new_node: Class<'_, NodeHandle>,
        ref_node: Class<'_, NodeHandle>,
    ) -> Result<()> {
        self.inner.borrow_mut().doc.insert_before(
            parent.borrow().id,
            new_node.borrow().id,
            ref_node.borrow().id,
        );
        Ok(())
    }

    pub fn parent_node<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        match self.inner.borrow().doc.parent_node(id) {
            Some(pid) => Ok(Some(self.get_or_create(ctx, pid)?)),
            None => Ok(None),
        }
    }

    pub fn first_child<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        match self.inner.borrow().doc.first_child(id) {
            Some(cid) => Ok(Some(self.get_or_create(ctx, cid)?)),
            None => Ok(None),
        }
    }

    pub fn last_child<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        match self.inner.borrow().doc.last_child(id) {
            Some(cid) => Ok(Some(self.get_or_create(ctx, cid)?)),
            None => Ok(None),
        }
    }

    pub fn next_sibling<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        match self.inner.borrow().doc.next_sibling(id) {
            Some(sid) => Ok(Some(self.get_or_create(ctx, sid)?)),
            None => Ok(None),
        }
    }

    pub fn previous_sibling<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        match self.inner.borrow().doc.previous_sibling(id) {
            Some(sid) => Ok(Some(self.get_or_create(ctx, sid)?)),
            None => Ok(None),
        }
    }

    pub fn node_type(&self, node: Class<'_, NodeHandle>) -> Result<u16> {
        Ok(self.inner.borrow().doc.node_type(node.borrow().id).unwrap_or(0))
    }

    pub fn tag_name(&self, node: Class<'_, NodeHandle>) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.tag_name(node.borrow().id))
    }

    pub fn node_value(&self, node: Class<'_, NodeHandle>) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.node_value(node.borrow().id))
    }

    pub fn set_node_value(&self, node: Class<'_, NodeHandle>, value: Option<String>) -> Result<()> {
        self.inner.borrow_mut().doc.set_node_value(node.borrow().id, value.as_deref().unwrap_or(""));
        Ok(())
    }

    pub fn has_attribute(&self, node: Class<'_, NodeHandle>, name: String) -> Result<bool> {
        Ok(self.inner.borrow().doc.has_attribute(node.borrow().id, &name))
    }

    pub fn attributes(&self, node: Class<'_, NodeHandle>) -> Result<Vec<Vec<String>>> {
        let list = self.inner.borrow().doc.attributes_list(node.borrow().id);
        Ok(list.into_iter().map(|(k, v)| vec![k, v]).collect())
    }

    pub fn replace_child(
        &self,
        parent: Class<'_, NodeHandle>,
        new_node: Class<'_, NodeHandle>,
        old_node: Class<'_, NodeHandle>,
    ) -> Result<()> {
        let parent_id = parent.borrow().id;
        let new_id = new_node.borrow().id;
        let old_id = old_node.borrow().id;
        let mut inner = self.inner.borrow_mut();
        inner.doc.insert_before(parent_id, new_id, old_id);
        inner.doc.remove_child(parent_id, old_id);
        Ok(())
    }

    pub fn create_document_fragment<'js>(&self, _ctx: Ctx<'js>) -> Result<Value<'js>> {
        Err(rquickjs::Error::new_loading_message(
            "dom",
            "createDocumentFragment not yet implemented",
        ))
    }

    pub fn get_attribute(
        &self,
        node: Class<'_, NodeHandle>,
        name: String,
    ) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.get_attribute(node.borrow().id, &name))
    }

    pub fn set_attribute(
        &self,
        node: Class<'_, NodeHandle>,
        name: String,
        value: String,
    ) -> Result<()> {
        self.inner.borrow_mut().doc.set_attribute(node.borrow().id, &name, &value);
        Ok(())
    }

    pub fn remove_attribute(&self, node: Class<'_, NodeHandle>, name: String) -> Result<()> {
        self.inner.borrow_mut().doc.remove_attribute(node.borrow().id, &name);
        Ok(())
    }
}
