use std::sync::Arc;

use js_runtime::{Extension, extension::ExtensionEnv};
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

// ── DomExtensionState（Context userdata）──────────────────────────────────────

/// DOM 扩展运行时状态，存入 context userdata，DocumentHandle 创建时读取。
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

    pub fn base_url(&self) -> Option<&str> {
        self.entry.as_ref().map(|e| e.base_url.as_str())
    }

    pub fn html_content(&self) -> Option<&str> {
        self.entry.as_ref().map(|e| e.content.as_str())
    }

    pub(crate) fn send_patch(&self, ops: Vec<DomOp>) {
        self.channel.send_dom(DomMsg::Patch(ops));
    }

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

    fn native_setup(&self, env: &mut ExtensionEnv<'_>) {
        env.declare_native_module::<DomModule>("webatom_ext_native:dom");
        if let Some(state) = &self.state {
            env.set_state(state.clone());
            env.set_state(ImportMapState::new());
            crate::bridge::init_html_entry(env, state);
        } else {
            env.set_state(ImportMapState::new());
        }
    }

    fn js_modules(&self) -> &[(&'static str, &'static str)] {
        &[("dom", include_str!("../js/dist/index.js"))]
    }
}
