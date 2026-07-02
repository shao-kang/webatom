use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use js_runtime::event_loop::{HostBridge, KeepaliveGuard};
use rquickjs::{Class, Ctx, Function, Persistent, Result, Value};
use rquickjs::class::Trace;
use webatom_blitz_msg::patch::DomOp;

use crate::dom_extension::DomExtensionState;
use super::document_inner::{DocumentInner, PatchBuffer};
use super::node_handle::NodeHandle;
use super::send_persistent::SendPersistent;
use super::script::execute_script;

/// Stores the JS event callback as context userdata so it is dropped with the context
/// (before JS_FreeRuntime), rather than in an Arc shared with spawn_blocking.
/// This prevents Persistent<Function> from outliving the QuickJS runtime.
struct EventCallbackStore(Mutex<Option<SendPersistent<Function<'static>>>>);

unsafe impl<'js> rquickjs::JsLifetime<'js> for EventCallbackStore {
    type Changed<'to> = Self;
}

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
    /// Set to true once the spawn_blocking loop has been started; prevents duplicate loops.
    #[qjs(skip_trace)]
    event_loop_running: Arc<AtomicBool>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for DocumentHandle {
    type Changed<'to> = rquickjs::Class<'to, DocumentHandle>;
}

impl DocumentHandle {
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
        let html = dom_state.as_ref().and_then(|s| s.html_content()).unwrap_or("");
        let inner = Rc::new(RefCell::new(DocumentInner::new_html(html)));
        Self {
            inner,
            keepalive: guard,
            flush_pending: Arc::new(AtomicBool::new(false)),
            host: Some(host),
            patch_buffer: Arc::new(Mutex::new(PatchBuffer::new())),
            dom_state,
            event_loop_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 为指定 id 创建 NodeHandle。若该节点已有 handle（has_handle == true），返回 null。
    #[qjs(rename = "acquireHandle")]
    pub fn acquire_handle<'js>(&self, ctx: Ctx<'js>, id: usize) -> Result<Option<Value<'js>>> {
        let mut inner = self.inner.borrow_mut();
        match inner.doc.get_mut(id) {
            Some(node) if node.has_handle => return Ok(None),
            Some(node) => node.has_handle = true,
            None => return Ok(None),
        }
        drop(inner);
        let handle = NodeHandle::new(id, Rc::downgrade(&self.inner));
        Ok(Some(Class::instance(ctx, handle)?.into_value()))
    }

    #[qjs(rename = "createElement")]
    pub fn create_element(&self, tag: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_element(&tag);
        if !tag.eq_ignore_ascii_case("script") {
            self.patch_buffer.lock().unwrap().push_structural(
                DomOp::CreateElement { id, tag, attrs: vec![] }
            );
            self.schedule_flush();
        }
        Ok(id)
    }

    #[qjs(rename = "createTextNode")]
    pub fn create_text_node(&self, content: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_text_node(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateText { id, content }
        );
        self.schedule_flush();
        Ok(id)
    }

    #[qjs(rename = "createComment")]
    pub fn create_comment(&self, content: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_comment(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateComment { id, content }
        );
        self.schedule_flush();
        Ok(id)
    }

    #[qjs(rename = "documentNode")]
    pub fn document_node(&self) -> Result<usize> {
        Ok(self.inner.borrow().doc.root())
    }

    #[qjs(rename = "documentElement")]
    pub fn document_element(&self) -> Result<Option<usize>> {
        let root = self.inner.borrow().doc.root();
        let inner = self.inner.borrow();
        let mut cur = inner.doc.first_child(root);
        loop {
            match cur {
                Some(id) if inner.doc.node_type(id) == Some(1) => return Ok(Some(id)),
                Some(id) => cur = inner.doc.next_sibling(id),
                None => return Ok(None),
            }
        }
    }

    #[qjs(rename = "body")]
    pub fn body(&self) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.body)
    }

    #[qjs(rename = "head")]
    pub fn head(&self) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.head)
    }

    #[qjs(rename = "appendChild")]
    pub fn append_child<'js>(
        &self,
        ctx: Ctx<'js>,
        parent_id: usize,
        child_id: usize,
    ) -> Result<()> {
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
    pub fn remove_child(&self, parent_id: usize, child_id: usize) -> Result<()> {
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
        parent_id: usize,
        new_id: usize,
        before_id: usize,
    ) -> Result<()> {
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
    pub fn parent_node(&self, node_id: usize) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.parent_node(node_id))
    }

    #[qjs(rename = "childNodes")]
    pub fn child_nodes(&self, node_id: usize) -> Result<Vec<usize>> {
        let inner = self.inner.borrow();
        let mut children = Vec::new();
        let mut cur = inner.doc.first_child(node_id);
        while let Some(id) = cur {
            children.push(id);
            cur = inner.doc.next_sibling(id);
        }
        Ok(children)
    }

    #[qjs(rename = "firstChild")]
    pub fn first_child(&self, node_id: usize) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.first_child(node_id))
    }

    #[qjs(rename = "lastChild")]
    pub fn last_child(&self, node_id: usize) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.last_child(node_id))
    }

    #[qjs(rename = "nextSibling")]
    pub fn next_sibling(&self, node_id: usize) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.next_sibling(node_id))
    }

    #[qjs(rename = "previousSibling")]
    pub fn previous_sibling(&self, node_id: usize) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.previous_sibling(node_id))
    }

    #[qjs(rename = "nodeType")]
    pub fn node_type(&self, node_id: usize) -> Result<u16> {
        Ok(self.inner.borrow().doc.node_type(node_id).unwrap_or(0))
    }

    #[qjs(rename = "tagName")]
    pub fn tag_name(&self, node_id: usize) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.tag_name(node_id))
    }

    #[qjs(rename = "nodeValue")]
    pub fn node_value(&self, node_id: usize) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.node_value(node_id))
    }

    #[qjs(rename = "setNodeValue")]
    pub fn set_node_value(&self, node_id: usize, value: Option<String>) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_node_value(node_id, value.as_deref().unwrap_or(""));
        let content = inner.doc.node_value(node_id).unwrap_or_default();
        drop(inner);
        self.patch_buffer.lock().unwrap().mark_text(node_id, content);
        self.schedule_flush();
        Ok(())
    }

    #[qjs(rename = "hasAttribute")]
    pub fn has_attribute(&self, node_id: usize, name: String) -> Result<bool> {
        Ok(self.inner.borrow().doc.has_attribute(node_id, &name))
    }

    pub fn attributes(&self, node_id: usize) -> Result<Vec<Vec<String>>> {
        let list = self.inner.borrow().doc.attributes_list(node_id);
        Ok(list.into_iter().map(|(k, v)| vec![k, v]).collect())
    }

    #[qjs(rename = "replaceChild")]
    pub fn replace_child(&self, parent_id: usize, new_id: usize, old_id: usize) -> Result<()> {
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
    pub fn create_document_fragment(&self) -> Result<usize> {
        Err(rquickjs::Error::new_loading_message(
            "dom",
            "createDocumentFragment not yet implemented",
        ))
    }

    #[qjs(rename = "getAttribute")]
    pub fn get_attribute(&self, node_id: usize, name: String) -> Result<Option<String>> {
        Ok(self.inner.borrow().doc.get_attribute(node_id, &name))
    }

    #[qjs(rename = "setAttribute")]
    pub fn set_attribute(&self, node_id: usize, name: String, value: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_attribute(node_id, &name, &value);
        let is_script = inner.doc.is_script_element(node_id);
        let attrs = if !is_script { inner.doc.attributes_list(node_id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(node_id, attrs);
            self.schedule_flush();
        }
        Ok(())
    }

    #[qjs(rename = "querySelector")]
    pub fn query_selector(&self, scope_id: usize, selector: String) -> Result<Option<usize>> {
        Ok(self.inner.borrow().doc.query_selector(scope_id, &selector))
    }

    #[qjs(rename = "querySelectorAll")]
    pub fn query_selector_all(&self, scope_id: usize, selector: String) -> Result<Vec<usize>> {
        Ok(self.inner.borrow().doc.query_selector_all(scope_id, &selector))
    }

    #[qjs(rename = "removeAttribute")]
    pub fn remove_attribute(&self, node_id: usize, name: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.remove_attribute(node_id, &name);
        let is_script = inner.doc.is_script_element(node_id);
        let attrs = if !is_script { inner.doc.attributes_list(node_id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(node_id, attrs);
            self.schedule_flush();
        }
        Ok(())
    }
    

    /// Register a JS callback to receive Events.
    ///
    /// The callback is stored in context userdata (EventCallbackStore) so it is dropped with
    /// the context before JS_FreeRuntime — preventing Persistent<Function> from outliving the
    /// runtime. spawn_blocking only captures pure-Rust values (no Persistent<T>).
    #[qjs(rename = "onEvent")]
    pub fn on_event<'js>(&self, ctx: Ctx<'js>, callback: Function<'js>) -> Result<()> {
        // Update or create the context-scoped callback store.
        let new_cb = SendPersistent(Persistent::save(&ctx, callback));
        if let Some(store) = ctx.userdata::<EventCallbackStore>() {
            *store.0.lock().unwrap() = Some(new_cb);
        } else {
            ctx.store_userdata(EventCallbackStore(Mutex::new(Some(new_cb))))?;
        }

        if self.event_loop_running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let Some(state) = &self.dom_state else { return Ok(()); };
        let Some(host) = &self.host else { return Ok(()); };

        let channel = Arc::clone(&state.channel);
        let task_tx = host.io.task_tx.clone();
        // spawn_blocking captures no Persistent<T> — only pure Rust values.
        tokio::task::spawn_blocking(move || {
            while let Ok(evt) = channel.recv_event() {
                let task: js_runtime::event_loop::MacroTask = Box::new(move |ctx: Ctx<'_>| {
                    let json = serde_json::to_string(&evt).unwrap_or_else(|_| "{}".to_string());
                    let json_str = rquickjs::String::from_str(ctx.clone(), &json)?;
                    let obj: rquickjs::Value = ctx.globals()
                        .get::<_, rquickjs::Object>("JSON")?
                        .get::<_, rquickjs::Function>("parse")?
                        .call((json_str,))?;
                    // Retrieve callback from context userdata (JS thread only — safe to restore here).
                    let func = ctx.userdata::<EventCallbackStore>()
                        .and_then(|store| {
                            store.0.lock().ok()
                                .and_then(|g| g.as_ref().and_then(|p| p.restore(&ctx).ok()))
                        });
                    if let Some(func) = func {
                        func.call::<_, ()>((obj,))?;
                    }
                    Ok(())
                });
                if task_tx.blocking_send(task).is_err() {
                    break;
                }
            }
        });

        Ok(())
    }
}
