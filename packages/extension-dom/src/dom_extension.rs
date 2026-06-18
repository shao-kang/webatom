use std::sync::Arc;

use rquickjs::{module::{Declarations, Exports, ModuleDef}, Class, Ctx, Result};
use js_runtime::Extension;
use webatom_blitz_msg::JsSide;
use webatom_js_runtime_macro::extension_native_module;
use webatom_js_runtime_macro::extension_js_module;

use crate::bridge::{DocumentHandle, NodeHandle};

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
        exports.export("DocumentHandle", Class::<DocumentHandle>::create_constructor(ctx)?)?;
        exports.export("NodeHandle", Class::<NodeHandle>::create_constructor(ctx)?)?;
        Ok(())
    }
}

/// `JsSide` 含有 `Receiver`，不支持 Clone/Copy，用 Arc 共享。
#[derive(rquickjs::JsLifetime, Clone)]
pub struct DomExtensionState {
    pub channel: Arc<JsSide>,
}

impl DomExtensionState {
    pub fn new(channel: JsSide) -> Self {
        Self { channel: Arc::new(channel) }
    }
}

pub struct DomExtension {
    state: Option<DomExtensionState>,
}

impl DomExtension {
    pub fn new() -> Self {
        Self { state: None }
    }
    pub fn set_state(&mut self, state: DomExtensionState) {
        self.state = Some(state);
    }
}

impl Extension for DomExtension {
    fn name(&self) -> &'static str {
        "dom"
    }
    extension_native_module!(DomModule);
    extension_js_module!(include_str!("../js/dist/index.js"));
    fn init<'js>(&self, _ctx: &Ctx<'js>) {
        //  if let Some(state) = self.state.clone() {
        //     let _ = _ctx.store_userdata(state);
        // }
    }
}