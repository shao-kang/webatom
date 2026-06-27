use std::sync::Arc;

use js_runtime::{event_loop::HostBridge, Extension};
use rquickjs::{
    module::{Declarations, Exports, ModuleDef},
    Class, Ctx, Result,
};
use webatom_blitz_msg::{msg::DomMsg, patch::DomOp, JsSide};

use crate::bridge::{DocumentHandle, ImportMapState, NodeHandle};

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
    channel: Arc<JsSide>,
}

impl DomExtensionState {
    pub fn new(channel: JsSide) -> Self {
        Self { channel: Arc::new(channel) }
    }

    /// 将增量 patch 发送到 Blitz 渲染线程（fire & forget）
    pub(crate) fn send_patch(&self, ops: Vec<DomOp>) {
        let _ = self.channel.dom_tx.send(DomMsg::Patch(ops));
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

    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
        rquickjs::Module::declare_def::<DomModule, _>(ctx.clone(), "webatom_ext_native:dom")?;
        if let Some(state) = &self.state {
            ctx.store_userdata(state.clone())?;
        }
        ctx.store_userdata(host.clone())?;
        // ImportMapState：初始为空，<script type="importmap"> 解析后更新
        ctx.store_userdata(ImportMapState::new())?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(include_str!("../js/dist/index.js"))
    }
}
