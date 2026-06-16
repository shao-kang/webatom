use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Class, Ctx, Persistent, Result, Value};
use rquickjs::class::Trace;

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
            let weak_ref_val = p.restore(&ctx)?;
            let deref_fn: rquickjs::Function = ctx.eval("(wr) => wr.deref()")?;
            let handle: Value<'js> = deref_fn.call((weak_ref_val,))?;
            if !handle.is_undefined() {
                return Ok(handle);
            }
            self.inner.borrow_mut().node_handles.remove(&id);
            
        }
        if let Some(node) = self.inner.borrow_mut().doc.get_mut(id) {
            node.has_handle = true;
        }
        let handle = NodeHandle { id, inner: Rc::downgrade(&self.inner) };
        let val: Value<'js> = Class::instance(ctx.clone(), handle)?.into_value();
        let make_weak: rquickjs::Function = ctx.eval("(t) => new WeakRef(t)")?;
        let weak_ref_val: Value<'js> = make_weak.call((val.clone(),))?;
        self.inner.borrow_mut().node_handles.insert(id, Persistent::save(&ctx, weak_ref_val));
        Ok(val)
    }
}

#[rquickjs::methods]
impl DocumentHandle {
    #[qjs(constructor)]
    pub fn js_new() -> Self {
        Self::new()
    }

    #[qjs(rename = "createElement")]
    pub fn create_element<'js>(&self, ctx: Ctx<'js>, tag: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_element(&tag);
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "createTextNode")]
    pub fn create_text_node<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_text_node(&content);
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "createComment")]
    pub fn create_comment<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_comment(&content);
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "documentNode")]
    pub fn document_node<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let id = self.inner.borrow().doc.root();
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "documentElement")]
    pub fn document_element<'js>(&self, ctx: Ctx<'js>) -> Result<Option<Value<'js>>> {
        let root = self.inner.borrow().doc.root();
        let child_id = self.inner.borrow().doc.first_child(root).and_then(|first| {
            let inner = self.inner.borrow();
            let mut cur = Some(first);
            loop {
                match cur {
                    Some(id) if inner.doc.node_type(id) == Some(1) => return Some(id),
                    Some(id) => cur = inner.doc.next_sibling(id),
                    None => return None,
                }
            }
        });
        match child_id {
            Some(id) => Ok(Some(self.get_or_create(ctx, id)?)),
            None => Ok(None),
        }
    }

    #[qjs(rename = "appendChild")]
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

    #[qjs(rename = "removeChild")]
    pub fn remove_child(
        &self,
        parent: Class<'_, NodeHandle>,
        child: Class<'_, NodeHandle>,
    ) -> Result<()> {
        let parent_id = parent.borrow().id;
        let child_id = child.borrow().id;
        self.inner.borrow_mut().doc.remove_child(parent_id, child_id);
        Ok(())
    }

    #[qjs(rename = "insertBefore")]
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

    #[qjs(rename = "parentNode")]
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

    #[qjs(rename = "firstChild")]
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

    #[qjs(rename = "lastChild")]
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

    #[qjs(rename = "nextSibling")]
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

    #[qjs(rename = "previousSibling")]
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

    #[qjs(rename = "nodeType")]
    pub fn node_type(&self, node: Class<'_, NodeHandle>) -> Result<u16> {
        Ok(self.inner.borrow().doc.node_type(node.borrow().id).unwrap_or(0))
    }

    #[qjs(rename = "tagName")]
    pub fn tag_name(&self, node: Class<'_, NodeHandle>) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.tag_name(node.borrow().id))
    }

    #[qjs(rename = "nodeValue")]
    pub fn node_value(&self, node: Class<'_, NodeHandle>) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.node_value(node.borrow().id))
    }

    #[qjs(rename = "setNodeValue")]
    pub fn set_node_value(&self, node: Class<'_, NodeHandle>, value: Option<String>) -> Result<()> {
        self.inner.borrow_mut().doc.set_node_value(node.borrow().id, value.as_deref().unwrap_or(""));
        Ok(())
    }

    #[qjs(rename = "hasAttribute")]
    pub fn has_attribute(&self, node: Class<'_, NodeHandle>, name: String) -> Result<bool> {
        Ok(self.inner.borrow().doc.has_attribute(node.borrow().id, &name))
    }

    pub fn attributes(&self, node: Class<'_, NodeHandle>) -> Result<Vec<Vec<String>>> {
        let list = self.inner.borrow().doc.attributes_list(node.borrow().id);
        Ok(list.into_iter().map(|(k, v)| vec![k, v]).collect())
    }

    #[qjs(rename = "replaceChild")]
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

    #[qjs(rename = "createDocumentFragment")]
    pub fn create_document_fragment<'js>(&self, _ctx: Ctx<'js>) -> Result<Value<'js>> {
        Err(rquickjs::Error::new_loading_message(
            "dom",
            "createDocumentFragment not yet implemented",
        ))
    }

    #[qjs(rename = "getAttribute")]
    pub fn get_attribute(
        &self,
        node: Class<'_, NodeHandle>,
        name: String,
    ) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.get_attribute(node.borrow().id, &name))
    }

    #[qjs(rename = "setAttribute")]
    pub fn set_attribute(
        &self,
        node: Class<'_, NodeHandle>,
        name: String,
        value: String,
    ) -> Result<()> {
        self.inner.borrow_mut().doc.set_attribute(node.borrow().id, &name, &value);
        Ok(())
    }

    #[qjs(rename = "removeAttribute")]
    pub fn remove_attribute(&self, node: Class<'_, NodeHandle>, name: String) -> Result<()> {
        self.inner.borrow_mut().doc.remove_attribute(node.borrow().id, &name);
        Ok(())
    }
}
