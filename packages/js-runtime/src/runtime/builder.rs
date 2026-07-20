use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub use crate::extension::{Extension, ExtensionSet};
use crate::module::ImportMap;
use super::runtime::JsRuntime;

pub struct JsRuntimeBuilder {
    extensions: ExtensionSet,
    cancel_token: Option<CancellationToken>,
    import_map: ImportMap,
    base_url: String,
    load_default_extensions: bool,
}

impl Default for JsRuntimeBuilder {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            cancel_token: None,
            import_map: ImportMap::new(""),
            base_url: String::new(),
            load_default_extensions: true,
        }
    }
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 base URL，用于 import map 中相对路径 value 的展开基准。
    /// 应在 `with_import_map` / `with_import_map_entry` 之前调用。
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self.import_map = ImportMap::new(self.base_url.clone());
        self
    }

    pub fn with_extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub fn with_extensions(mut self, mut exts: ExtensionSet) -> Self {
        self.extensions.append(&mut exts);
        self
    }

    /// 禁用默认扩展（console / timer 等），仅加载通过 `with_extension` 手动添加的扩展。
    pub fn without_default_extensions(mut self) -> Self {
        self.load_default_extensions = false;
        self
    }

    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// 设置初始 import map（bare specifier → URL/path），以 HashMap 批量传入。
    /// value 中的相对路径基于已设定的 base_url 展开。
    pub fn with_import_map(mut self, map: HashMap<String, String>) -> Self {
        self.import_map = ImportMap::with_entries(self.base_url.clone(), map);
        self
    }

    /// 追加单条 import map 映射，value 中的相对路径自动展开。
    pub fn with_import_map_entry(self, specifier: impl Into<String>, target: impl Into<String>) -> Self {
        self.import_map.insert(specifier, target);
        self
    }

    pub fn build(self) -> rquickjs::Result<JsRuntime> {
        let cancel_token = self.cancel_token.unwrap_or_else(CancellationToken::new);
        let mut extensions = self.extensions;
        if self.load_default_extensions {
            let mut defaults = crate::web::default_extensions();
            defaults.append(&mut extensions);
            extensions = defaults;
        }
        JsRuntime::assemble(extensions, cancel_token, self.import_map)
    }
}
