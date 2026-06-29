use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use js_runtime::event_loop::{HostBridge, KeepaliveGuard};
use rquickjs::{Class, Ctx, Function, Persistent, Result, Value};
use rquickjs::class::Trace;
use webatom_blitz_msg::patch::DomOp;

use crate::dom_extension::DomExtensionState;
use super::document_inner::{DocumentInner, PatchBuffer};
use super::node_handle::NodeHandle;
use super::script::execute_script;

/// `Persistent<Function>` wrapped in `SendPersistent` so it can cross thread boundaries
/// when stored in `Arc<Mutex<...>>`. Safe because rquickjs guarantees the underlying
/// JSRuntime is pinned to a single thread and we only restore the function on that thread.
struct SendPersistent<T>(Persistent<T>);
unsafe impl<T> Send for SendPersistent<T> {}
unsafe impl<T> Sync for SendPersistent<T> {}

impl<T: rquickjs::JsLifetime<'static> + Clone> SendPersistent<T> {
    fn restore<'js>(&self, ctx: &Ctx<'js>) -> Result<T::Changed<'js>> {
        self.0.clone().restore(ctx)
    }
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
    /// Shared callback slot: spawn_blocking loop reads from here; on_event just overwrites it.
    #[qjs(skip_trace)]
    event_listener: Arc<Mutex<Option<SendPersistent<Function<'static>>>>>,
    /// Set to true once the spawn_blocking loop has been started; prevents duplicate loops.
    #[qjs(skip_trace)]
    event_loop_running: Arc<AtomicBool>,
    /// Mirror of DocumentInner.node_handles in a Send-capable container so MacroTask closures
    /// can resolve a blitz node_id to its JS NodeHandle (via WeakRef restore + deref).
    #[qjs(skip_trace)]
    shared_handles: Arc<Mutex<HashMap<usize, SendPersistent<Value<'static>>>>>,
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
            event_listener: Arc::new(Mutex::new(None)),
            event_loop_running: Arc::new(AtomicBool::new(false)),
            shared_handles: Arc::new(Mutex::new(HashMap::new())),
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
        // Save one persistent in node_handles (for get_or_create cache) and a second in
        // shared_handles (Send-capable, consumed by MacroTask handle resolution).
        self.inner.borrow_mut().node_handles.insert(id, Persistent::save(&ctx, weak_ref_val.clone()));
        self.shared_handles.lock().unwrap().insert(id, SendPersistent(Persistent::save(&ctx, weak_ref_val)));
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
            event_listener: Arc::new(Mutex::new(None)),
            event_loop_running: Arc::new(AtomicBool::new(false)),
            shared_handles: Arc::new(Mutex::new(HashMap::new())),
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

    /// Register a JS callback to receive Events.
    ///
    /// The callback is stored in a shared slot. A single `spawn_blocking` loop started on
    /// the first call reads events from `event_rx` and dispatches MacroTasks that invoke
    /// whatever callback is currently in the slot. Subsequent calls to `on_event` simply
    /// replace the callback in the slot — no new loop is spawned.
    #[qjs(rename = "onEvent")]
    pub fn on_event<'js>(&self, ctx: Ctx<'js>, callback: Function<'js>) -> Result<()> {
        use webatom_blitz_msg::Event;

        // Always update the shared callback slot.
        *self.event_listener.lock().unwrap() =
            Some(SendPersistent(Persistent::save(&ctx, callback)));

        // Start the blocking loop only once.
        if self.event_loop_running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let Some(state) = &self.dom_state else { return Ok(()); };
        let Some(host) = &self.host else { return Ok(()); };

        let channel = Arc::clone(&state.channel);
        let task_tx = host.io.task_tx.clone();
        let listener = Arc::clone(&self.event_listener);
        let shared_handles = Arc::clone(&self.shared_handles);

        tokio::task::spawn_blocking(move || {
            while let Ok(evt) = channel.event_rx.recv() {
                let cb = Arc::clone(&listener);
                let shared2 = Arc::clone(&shared_handles);
                let task: js_runtime::event_loop::MacroTask = Box::new(move |ctx: Ctx<'_>| {
                    let obj = rquickjs::Object::new(ctx.clone())?;
                    let mut target_node_id: Option<usize> = None;
                    match evt {
                        Event::KeyDown { key, modifiers } => {
                            obj.set("type", "keydown")?;
                            obj.set("key", key)?;
                            obj.set("modifiers", modifiers)?;
                        }
                        Event::KeyUp { key, modifiers } => {
                            obj.set("type", "keyup")?;
                            obj.set("key", key)?;
                            obj.set("modifiers", modifiers)?;
                        }
                        Event::Click { node_id, x, y } => {
                            obj.set("type", "click")?;
                            target_node_id = Some(node_id);
                            obj.set("x", x)?;
                            obj.set("y", y)?;
                        }
                        Event::Focus { node_id } => {
                            obj.set("type", "focus")?;
                            target_node_id = Some(node_id);
                        }
                        Event::Blur { node_id } => {
                            obj.set("type", "blur")?;
                            target_node_id = Some(node_id);
                        }
                        Event::Resize { width, height } => {
                            obj.set("type", "resize")?;
                            obj.set("width", width)?;
                            obj.set("height", height)?;
                        }
                        Event::MouseMove { x, y } => {
                            obj.set("type", "mousemove")?;
                            obj.set("x", x)?;
                            obj.set("y", y)?;
                        }
                        Event::MouseDown { node_id, x, y, button } => {
                            obj.set("type", "mousedown")?;
                            target_node_id = Some(node_id);
                            obj.set("x", x)?;
                            obj.set("y", y)?;
                            obj.set("button", button)?;
                        }
                        Event::MouseUp { node_id, x, y, button } => {
                            obj.set("type", "mouseup")?;
                            target_node_id = Some(node_id);
                            obj.set("x", x)?;
                            obj.set("y", y)?;
                            obj.set("button", button)?;
                        }
                        Event::DblClick { node_id, x, y } => {
                            obj.set("type", "dblclick")?;
                            target_node_id = Some(node_id);
                            obj.set("x", x)?;
                            obj.set("y", y)?;
                        }
                        Event::Scroll { delta_x, delta_y } => {
                            obj.set("type", "scroll")?;
                            obj.set("deltaX", delta_x)?;
                            obj.set("deltaY", delta_y)?;
                        }
                    }
                    if let Some(nid) = target_node_id {
                        let handle_val = {
                            let guard = shared2.lock().unwrap();
                            guard.get(&nid).and_then(|p| p.restore(&ctx).ok())
                        }
                        .and_then(|wr| {
                            let wr_obj = wr.as_object()?.clone();
                            let deref_fn: rquickjs::Function<'_> = wr_obj.get("deref").ok()?;
                            deref_fn
                                .call::<_, rquickjs::Value>((rquickjs::function::This(wr_obj),))
                                .ok()
                        })
                        .filter(|v| !v.is_undefined() && !v.is_null())
                        .unwrap_or_else(|| rquickjs::Value::new_null(ctx.clone()));
                        obj.set("targetHandle", handle_val)?;
                    }
                    // Restore the current callback from the shared slot.
                    let func = cb.lock().unwrap()
                        .as_ref()
                        .map(|p| p.restore(&ctx))
                        .transpose()?;
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
