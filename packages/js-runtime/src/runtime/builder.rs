use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub use crate::extension::{Extension, ExtensionSet};
use crate::event_loop::{HeadlessRenderScheduler, RenderScheduler};
use crate::module::ImportMap;
use super::runtime::JsRuntime;

pub struct JsRuntimeBuilder {
    extensions: ExtensionSet,
    cancel_token: Option<CancellationToken>,
    render_scheduler: Box<dyn RenderScheduler>,
    import_map: ImportMap,
}

impl Default for JsRuntimeBuilder {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            cancel_token: None,
            render_scheduler: Box::new(HeadlessRenderScheduler),
            import_map: ImportMap::new(),
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

    pub fn with_render_scheduler(mut self, scheduler: impl RenderScheduler + 'static) -> Self {
        self.render_scheduler = Box::new(scheduler);
        self
    }

    /// 设置初始 import map（bare specifier → URL/path），以 HashMap 批量传入。
    pub fn with_import_map(mut self, map: HashMap<String, String>) -> Self {
        self.import_map = ImportMap::with_entries(map);
        self
    }

    /// 追加单条 import map 映射。
    pub fn with_import_map_entry(self, specifier: impl Into<String>, target: impl Into<String>) -> Self {
        self.import_map.insert(specifier, target);
        self
    }

    pub fn build(self) -> rquickjs::Result<JsRuntime> {
        let cancel_token = self.cancel_token.unwrap_or_else(CancellationToken::new);
        JsRuntime::assemble(self.extensions, cancel_token, self.render_scheduler, self.import_map)
    }
}
