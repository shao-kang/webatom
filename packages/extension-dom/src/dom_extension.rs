use std::sync::Arc;

use js_runtime::{ Extension};
use rquickjs::{
    module::{Declarations, Exports, ModuleDef},
    Class, Ctx, Result,
};
use webatom_blitz_msg::{msg::DomMsg, patch::DomOp, JsSide};

use crate::bridge::{DocumentHandle, ImportMapState, NodeHandle};
use crate::html_entry::HtmlEntry;

// ── Native module ─────────────────────────────────────────────────────────────

pub struct DomModule;

impl ModuleDef for DomModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("DocumentHandle")?;
        decl.declare("NodeHandle")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        Class::<DocumentHandle>::define(&ctx.globals())?;
        Class::<NodeHandle>::define(&ctx.globals())?;
        exports.export(
            "DocumentHandle",
            Class::<DocumentHandle>::create_constructor(ctx)?,
        )?;
        exports.export(
            "NodeHandle",
            Class::<NodeHandle>::create_constructor(ctx)?,
        )?;
        Ok(())
    }
}

// ── DomExtensionState（Context userdata）──────────────────────────────────────────────

/// DOM 扩展运行时状态，存入 context userdata，DocumentHandle 创建时读取。
/// Clone 后可安全移入 MacroTask 闭包（Arc<JsSide> 为 Send）。
#[derive(rquickjs::JsLifetime, Clone)]
pub struct DomExtensionState {
    pub channel: Arc<JsSide>,
    entry: Option<Arc<HtmlEntry>>,
}

impl DomExtensionState {
    pub fn new(channel: JsSide) -> Self {
        Self { channel: Arc::new(channel), entry: None }
    }

    pub fn with_entry(mut self, entry: Arc<HtmlEntry>) -> Self {
        self.entry = Some(entry);
        self
    }

    /// 入口页面的基础 URL（用于相对路径解析）
    pub fn base_url(&self) -> Option<&str> {
        self.entry.as_ref().map(|e| e.base_url.as_str())
    }

    /// 入口页面的原始 HTML 内容
    pub fn html_content(&self) -> Option<&str> {
        self.entry.as_ref().map(|e| e.content.as_str())
    }

    /// 将增量 patch 发送到 Blitz 渲染线程（fire & forget，自动唤醒 Blitz）
    pub(crate) fn send_patch(&self, ops: Vec<DomOp>) {
        self.channel.send_dom(DomMsg::Patch(ops));
    }

    /// 发送首帧全量快照（自动唤醒 Blitz）
    pub(crate) fn send_full(&self, snapshot: webatom_blitz_msg::snapshot::DomSnapshot) {
        self.channel.send_dom(DomMsg::Full(snapshot));
    }
}

// ── Extension ─────────────────────────────────────────────────────────────────

pub struct DomExtension {
    state: Option<DomExtensionState>,
}

impl DomExtension {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn with_state(state: DomExtensionState) -> Self {
        Self { state: Some(state) }
    }

    pub fn set_state(&mut self, state: DomExtensionState) {
        self.state = Some(state);
    }
}

impl Extension for DomExtension {
    fn name(&self) -> &'static str {
        "dom"
    }

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &["webatom_ext_native:dom"]
    }

    fn native_setup(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
        rquickjs::Module::declare_def::<DomModule, _>(ctx.clone(), "webatom_ext_native:dom")?;
        if let Some(state) = &self.state {
            ctx.store_userdata(state.clone())?;
        }
        ctx.store_userdata(host.clone())?;
        // ImportMapState：初始为空，<script type="importmap"> 解析后更新
        ctx.store_userdata(ImportMapState::new())?;

        // HTML 入口：发送首帧快照 + 调度脚本（在 js_glue 运行前调用，脚本作为宏任务延迟）
        if let Some(state) = &self.state {
            crate::bridge::init_html_entry(ctx, host, state)?;
        }
        Ok(())
    }

    fn js_modules(&self) -> &[(&'static str, &'static str)] {
        &[("dom", include_str!("../js/dist/index.js"))]
    }
}
