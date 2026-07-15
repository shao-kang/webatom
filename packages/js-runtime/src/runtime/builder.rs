use tokio_util::sync::CancellationToken;

pub use crate::extension::{Extension, ExtensionSet};
use crate::event_loop::{HeadlessRenderScheduler, RenderScheduler};
use super::runtime::JsRuntime;

pub struct JsRuntimeBuilder {
    extensions: ExtensionSet,
    cancel_token: Option<CancellationToken>,
    render_scheduler: Box<dyn RenderScheduler>,
}

impl Default for JsRuntimeBuilder {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            cancel_token: None,
            render_scheduler: Box::new(HeadlessRenderScheduler),
        }
    }
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub fn with_extensions(mut self, mut exts: ExtensionSet) -> Self {
        self.extensions.append(&mut exts);
        self
    }

    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// 替换渲染调度器（接入真实渲染器如 blitz 时使用）。
    /// 默认为 [`HeadlessRenderScheduler`]（rAF 回调永不触发）。
    pub fn with_render_scheduler(mut self, scheduler: impl RenderScheduler + 'static) -> Self {
        self.render_scheduler = Box::new(scheduler);
        self
    }

    pub fn build(self) -> rquickjs::Result<JsRuntime> {
        let cancel_token = self.cancel_token.unwrap_or_else(CancellationToken::new);
        JsRuntime::assemble(self.extensions, cancel_token, self.render_scheduler)
    }
}
