use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use js_runtime::event_loop::{HostBridge, KeepaliveGuard};
use rquickjs::{Class, Ctx, Persistent, Result, Value};
use rquickjs::class::Trace;
use webatom_blitz_msg::patch::DomOp;

use crate::dom_extension::DomExtensionState;
use super::document_inner::{DocumentInner, PatchBuffer};
use super::node_handle::NodeHandle;
use super::script::execute_script;

#[derive(Trace)]
#[rquickjs::class(rename = "DocumentHandle")]
pub struct DocumentHandle {
    #[qjs(skip_trace)]
    pub(crate) inner: Rc<RefCell<DocumentInner>>,
    #[qjs(skip_trace)]
    #[allow(dead_code)] // RAII guard: held to keep the event loop alive while this document exists
    keepalive: Option<KeepaliveGuard>,
    #[qjs(skip_trace)]
    flush_pending: Arc<AtomicBool>,
    #[qjs(skip_trace)]
    host: Option<HostBridge>,
    #[qjs(skip_trace)]
    patch_buffer: Arc<Mutex<PatchBuffer>>,
    #[qjs(skip_trace)]
    dom_state: Option<DomExtensionState>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for DocumentHandle {
    type Changed<'to> = rquickjs::Class<'to, DocumentHandle>;
}

impl DocumentHandle {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(DocumentInner::new())),
            keepalive: None,
            flush_pending: Arc::new(AtomicBool::new(false)),
            host: None,
            patch_buffer: Arc::new(Mutex::new(PatchBuffer::new())),
            dom_state: None,
        }
    }

    fn schedule_flush(&self) {
        let Some(host) = &self.host else { return };
        if self.flush_pending.swap(true, Ordering::AcqRel) { return; }
        let pending = Arc::clone(&self.flush_pending);
        let patch_buffer = Arc::clone(&self.patch_buffer);
        let dom_state = self.dom_state.clone();
        let _ = host.io.task_tx.try_send(Box::new(move |_ctx| {
            pending.store(false, Ordering::Release);
            let ops = patch_buffer.lock().unwrap().drain_ops();
            if !ops.is_empty() {
                if let Some(state) = &dom_state {
                    state.send_patch(ops);
                }
            }
            Ok(())
        }));
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
    pub fn js_new<'js>(_ctx: Ctx<'js>) -> Self {
        tracing::info!("new handle document");
        let host = _ctx.userdata::<HostBridge>()
            .expect("HostBridge userdata not registered")
            .clone();
        let guard = host.runtime.keepalive.acquire();
        let dom_state = _ctx.userdata::<DomExtensionState>().map(|g| (*g).clone());
        // 若有 HTML 入口，以其内容初始化 Document，与首帧快照保持一致
        let html = dom_state.as_ref().and_then(|s| s.html_content()).unwrap_or("");
        let inner = Rc::new(RefCell::new(DocumentInner::new_html(html)));
        Self {
            inner,
            keepalive: guard,
            flush_pending: Arc::new(AtomicBool::new(false)),
            host: Some(host),
            patch_buffer: Arc::new(Mutex::new(PatchBuffer::new())),
            dom_state,
        }
    }

    #[qjs(rename = "createElement")]
    pub fn create_element<'js>(&self, ctx: Ctx<'js>, tag: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_element(&tag);
        if !tag.eq_ignore_ascii_case("script") {
            self.patch_buffer.lock().unwrap().push_structural(
                DomOp::CreateElement { id, tag, attrs: vec![] }
            );
            self.schedule_flush();
        }
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "createTextNode")]
    pub fn create_text_node<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_text_node(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateText { id, content }
        );
        self.schedule_flush();
        self.get_or_create(ctx, id)
    }

    #[qjs(rename = "createComment")]
    pub fn create_comment<'js>(&self, ctx: Ctx<'js>, content: String) -> Result<Value<'js>> {
        let id = self.inner.borrow_mut().doc.create_comment(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateComment { id, content }
        );
        self.schedule_flush();
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

    #[qjs(rename = "body")]
    pub fn body<'js>(&self, ctx: Ctx<'js>) -> Result<Option<Value<'js>>> {
        let id = self.inner.borrow().doc.body;
        match id {
            Some(id) => Ok(Some(self.get_or_create(ctx, id)?)),
            None => Ok(None),
        }
    }

    #[qjs(rename = "head")]
    pub fn head<'js>(&self, ctx: Ctx<'js>) -> Result<Option<Value<'js>>> {
        let id = self.inner.borrow().doc.head;
        match id {
            Some(id) => Ok(Some(self.get_or_create(ctx, id)?)),
            None => Ok(None),
        }
    }

    #[qjs(rename = "appendChild")]
    pub fn append_child<'js>(
        &self,
        ctx: Ctx<'js>,
        parent: Class<'_, NodeHandle>,
        child: Class<'_, NodeHandle>,
    ) -> Result<()> {
        tracing::info!("append_child");
        let parent_id = parent.borrow().id;
        let child_id = child.borrow().id;
        self.inner.borrow_mut().doc.append_child(parent_id, child_id);
        let inner = self.inner.borrow();
        let parent_is_script = inner.doc.is_script_element(parent_id);
        let child_is_script = inner.doc.is_script_element(child_id);
        let script_info = inner.doc.script_info(child_id);
        drop(inner);
        if !parent_is_script && !child_is_script {
            self.patch_buffer.lock().unwrap().push_structural(
                DomOp::AppendChild { parent: parent_id, child: child_id }
            );
        }
        if let Some(info) = script_info {
            execute_script(&ctx, self.host.as_ref(), info, child_id)?;
        }
        self.schedule_flush();
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
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::RemoveChild { parent: parent_id, child: child_id }
        );
        self.schedule_flush();
        Ok(())
    }

    #[qjs(rename = "insertBefore")]
    pub fn insert_before<'js>(
        &self,
        ctx: Ctx<'js>,
        parent: Class<'_, NodeHandle>,
        new_node: Class<'_, NodeHandle>,
        ref_node: Class<'_, NodeHandle>,
    ) -> Result<()> {
        let parent_id = parent.borrow().id;
        let new_id = new_node.borrow().id;
        let before_id = ref_node.borrow().id;
        self.inner.borrow_mut().doc.insert_before(parent_id, new_id, before_id);
        let inner = self.inner.borrow();
        let parent_is_script = inner.doc.is_script_element(parent_id);
        let new_is_script = inner.doc.is_script_element(new_id);
        let script_info = inner.doc.script_info(new_id);
        drop(inner);
        if !parent_is_script && !new_is_script {
            self.patch_buffer.lock().unwrap().push_structural(
                DomOp::InsertBefore { parent: parent_id, child: new_id, before: before_id }
            );
        }
        if let Some(info) = script_info {
            execute_script(&ctx, self.host.as_ref(), info, new_id)?;
        }
        self.schedule_flush();
        Ok(())
    }

    #[qjs(rename = "parentNode")]
    pub fn parent_node<'js>(
        &self,
        ctx: Ctx<'js>,
        node: Class<'_, NodeHandle>,
    ) -> Result<Option<Value<'js>>> {
        let id = node.borrow().id;
        let pid = self.inner.borrow().doc.parent_node(id);
        match pid {
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
        let cid = self.inner.borrow().doc.first_child(id);
        match cid {
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
        let cid = self.inner.borrow().doc.last_child(id);
        match cid {
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
        let sid = self.inner.borrow().doc.next_sibling(id);
        match sid {
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
        let sid = self.inner.borrow().doc.previous_sibling(id);
        match sid {
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
        let id = node.borrow().id;
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_node_value(id, value.as_deref().unwrap_or(""));
        let content = inner.doc.node_value(id).unwrap_or_default();
        drop(inner);
        self.patch_buffer.lock().unwrap().mark_text(id, content);
        self.schedule_flush();
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
        {
            let mut inner = self.inner.borrow_mut();
            inner.doc.insert_before(parent_id, new_id, old_id);
            inner.doc.remove_child(parent_id, old_id);
        }
        let mut buf = self.patch_buffer.lock().unwrap();
        buf.push_structural(DomOp::InsertBefore { parent: parent_id, child: new_id, before: old_id });
        buf.push_structural(DomOp::RemoveChild { parent: parent_id, child: old_id });
        drop(buf);
        self.schedule_flush();
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
        let id = node.borrow().id;
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_attribute(id, &name, &value);
        let is_script = inner.doc.is_script_element(id);
        let attrs = if !is_script { inner.doc.attributes_list(id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(id, attrs);
            self.schedule_flush();
        }
        Ok(())
    }

    #[qjs(rename = "removeAttribute")]
    pub fn remove_attribute(&self, node: Class<'_, NodeHandle>, name: String) -> Result<()> {
        let id = node.borrow().id;
        let mut inner = self.inner.borrow_mut();
        inner.doc.remove_attribute(id, &name);
        let is_script = inner.doc.is_script_element(id);
        let attrs = if !is_script { inner.doc.attributes_list(id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(id, attrs);
            self.schedule_flush();
        }
        Ok(())
    }
}
