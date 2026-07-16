use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use js_runtime::event_loop::event_loop_impl::{EventPort, EventPortRegistrar, QueueKind};
use js_runtime::extension::ExtensionEnv;
use rquickjs::{Class, Ctx, Function, Persistent, Result, Value};
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
    /// 持久 flush 端口，首次 schedule_flush 时注册，之后复用。
    #[qjs(skip_trace)]
    flush_port: Option<EventPort>,
    /// true 表示已向 flush_port send(())，handler 还未执行（防止同步代码路径重复 send）。
    #[qjs(skip_trace)]
    flush_pending: Arc<AtomicBool>,
    #[qjs(skip_trace)]
    patch_buffer: Arc<Mutex<PatchBuffer>>,
    #[qjs(skip_trace)]
    dom_state: Option<DomExtensionState>,
    /// 每次 on_event 替换；旧端口 drop → handler 自动注销。
    #[qjs(skip_trace)]
    event_port: Option<EventPort>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for DocumentHandle {
    type Changed<'to> = rquickjs::Class<'to, DocumentHandle>;
}

impl DocumentHandle {
    fn schedule_flush_with_ctx(&mut self, ctx: &Ctx<'_>) {
        // 若已有 pending send 尚未被 handler 消费，不重复 send
        if self.flush_pending.swap(true, Ordering::AcqRel) {
            return;
        }

        if self.flush_port.is_none() {
            let Some(mut registrar) = EventPortRegistrar::from_ctx(ctx).map(|g| (*g).clone()) else {
                self.flush_pending.store(false, Ordering::Release);
                return;
            };
            let patch_buffer = Arc::clone(&self.patch_buffer);
            let dom_state = self.dom_state.clone();
            let pending = Arc::clone(&self.flush_pending);
            let port = registrar.register_js_event_port(QueueKind::Macro, move |_ctx, _| {
                // 先重置 pending，再 drain，使后续同步 DOM 变更可再次触发 flush
                pending.store(false, Ordering::Release);
                let ops = patch_buffer.lock().unwrap().drain_ops();
                if !ops.is_empty() {
                    if let Some(state) = &dom_state {
                        state.send_patch(ops);
                    }
                }
                Ok(())
            });
            self.flush_port = Some(port);
        }

        if let Some(port) = &self.flush_port {
            port.send(());
        }
    }
}

#[rquickjs::methods]
impl DocumentHandle {
    #[qjs(constructor)]
    pub fn js_new<'js>(ctx: Ctx<'js>) -> Self {
        tracing::info!("new handle document");
        let dom_state_guard = ExtensionEnv::get_state::<DomExtensionState>(&ctx);
        let html = dom_state_guard.as_deref().and_then(|s| s.html_content()).unwrap_or("").to_owned();
        let dom_state: Option<DomExtensionState> = dom_state_guard.map(|g| (*g).clone());
        let inner = Rc::new(RefCell::new(DocumentInner::new_html(&html)));
        Self {
            inner,
            flush_port: None,
            flush_pending: Arc::new(AtomicBool::new(false)),
            patch_buffer: Arc::new(Mutex::new(PatchBuffer::new())),
            dom_state,
            event_port: None,
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
    pub fn create_element<'js>(&mut self, ctx: Ctx<'js>, tag: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_element(&tag);
        if !tag.eq_ignore_ascii_case("script") {
            self.patch_buffer.lock().unwrap().push_structural(
                DomOp::CreateElement { id, tag, attrs: vec![] }
            );
            self.schedule_flush_with_ctx(&ctx);
        }
        Ok(id)
    }

    #[qjs(rename = "createTextNode")]
    pub fn create_text_node<'js>(&mut self, ctx: Ctx<'js>, content: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_text_node(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateText { id, content }
        );
        self.schedule_flush_with_ctx(&ctx);
        Ok(id)
    }

    #[qjs(rename = "createComment")]
    pub fn create_comment<'js>(&mut self, ctx: Ctx<'js>, content: String) -> Result<usize> {
        let id = self.inner.borrow_mut().doc.create_comment(&content);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::CreateComment { id, content }
        );
        self.schedule_flush_with_ctx(&ctx);
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
        &mut self,
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
        let registrar = EventPortRegistrar::from_ctx(&ctx).map(|g| (*g).clone());
        if let Some(info) = script_info {
            execute_script(&ctx, registrar.as_ref(), info, child_id)?;
        }
        self.schedule_flush_with_ctx(&ctx);
        Ok(())
    }

    #[qjs(rename = "removeChild")]
    pub fn remove_child<'js>(&mut self, ctx: Ctx<'js>, parent_id: usize, child_id: usize) -> Result<()> {
        self.inner.borrow_mut().doc.remove_child(parent_id, child_id);
        self.patch_buffer.lock().unwrap().push_structural(
            DomOp::RemoveChild { parent: parent_id, child: child_id }
        );
        self.schedule_flush_with_ctx(&ctx);
        Ok(())
    }

    #[qjs(rename = "insertBefore")]
    pub fn insert_before<'js>(
        &mut self,
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
        let registrar = EventPortRegistrar::from_ctx(&ctx).map(|g| (*g).clone());
        if let Some(info) = script_info {
            execute_script(&ctx, registrar.as_ref(), info, new_id)?;
        }
        self.schedule_flush_with_ctx(&ctx);
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
    pub fn set_node_value<'js>(&mut self, ctx: Ctx<'js>, node_id: usize, value: Option<String>) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_node_value(node_id, value.as_deref().unwrap_or(""));
        let content = inner.doc.node_value(node_id).unwrap_or_default();
        drop(inner);
        self.patch_buffer.lock().unwrap().mark_text(node_id, content);
        self.schedule_flush_with_ctx(&ctx);
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
    pub fn replace_child<'js>(&mut self, ctx: Ctx<'js>, parent_id: usize, new_id: usize, old_id: usize) -> Result<()> {
        {
            let mut inner = self.inner.borrow_mut();
            inner.doc.insert_before(parent_id, new_id, old_id);
            inner.doc.remove_child(parent_id, old_id);
        }
        let mut buf = self.patch_buffer.lock().unwrap();
        buf.push_structural(DomOp::InsertBefore { parent: parent_id, child: new_id, before: old_id });
        buf.push_structural(DomOp::RemoveChild { parent: parent_id, child: old_id });
        drop(buf);
        self.schedule_flush_with_ctx(&ctx);
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
    pub fn set_attribute<'js>(&mut self, ctx: Ctx<'js>, node_id: usize, name: String, value: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.set_attribute(node_id, &name, &value);
        let is_script = inner.doc.is_script_element(node_id);
        let attrs = if !is_script { inner.doc.attributes_list(node_id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(node_id, attrs);
            self.schedule_flush_with_ctx(&ctx);
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
    pub fn remove_attribute<'js>(&mut self, ctx: Ctx<'js>, node_id: usize, name: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.doc.remove_attribute(node_id, &name);
        let is_script = inner.doc.is_script_element(node_id);
        let attrs = if !is_script { inner.doc.attributes_list(node_id) } else { vec![] };
        drop(inner);
        if !is_script {
            self.patch_buffer.lock().unwrap().mark_attrs(node_id, attrs);
            self.schedule_flush_with_ctx(&ctx);
        }
        Ok(())
    }

    /// 注册 JS 回调接收来自 Blitz 的事件。
    ///
    /// 每次调用替换旧端口（旧 handler 自动注销）。
    /// spawn_blocking 在后台持续接收事件，每条事件通过 port.send() 推入宏任务队列，
    /// handler 在 JS 线程反序列化并调用 callback。
    /// port drop（DocumentHandle GC）时 recv_event() 返回 Err，loop 自然退出。
    #[qjs(rename = "onEvent")]
    pub fn on_event<'js>(&mut self, ctx: Ctx<'js>, callback: Function<'js>) -> Result<()> {
        let Some(state) = &self.dom_state else { return Ok(()); };
        let Some(mut registrar) = EventPortRegistrar::from_ctx(&ctx).map(|g| (*g).clone()) else {
            return Ok(());
        };

        let cb = Persistent::save(&ctx, callback);
        let channel = Arc::clone(&state.channel);

        let port = registrar.register_js_event_port(QueueKind::Macro, move |ctx, payload| {
            use webatom_blitz_msg::event::Event;
            let Some(evt) = payload.downcast_ref::<Event>() else { return Ok(()); };
            let json = serde_json::to_string(evt).unwrap_or_else(|_| "{}".to_string());
            let json_str = rquickjs::String::from_str(ctx.clone(), &json)?;
            let obj: rquickjs::Value = ctx.globals()
                .get::<_, rquickjs::Object>("JSON")?
                .get::<_, rquickjs::Function>("parse")?
                .call((json_str,))?;
            if let Ok(func) = cb.clone().restore(&ctx) {
                func.call::<_, ()>((obj,))?;
            }
            Ok(())
        });

        // spawn_blocking 只持有纯 Rust 值（port clone + channel），无 Persistent<T>
        let port_clone = port.clone();
        tokio::task::spawn_blocking(move || {
            while let Ok(evt) = channel.recv_event() {
                port_clone.send(evt);
            }
        });

        // 替换旧端口：旧 EventPortGuard drop → cleanup 信号 → 旧 handler 注销
        self.event_port = Some(port);
        Ok(())
    }
}
