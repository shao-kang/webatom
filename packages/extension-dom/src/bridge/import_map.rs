use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// import map 数据：对应 JSON 中 "imports" 和 "scopes" 字段
#[derive(Default, Clone)]
pub struct ImportMapData {
    /// 全局映射：bare specifier → URL/path
    pub imports: HashMap<String, String>,
    /// 作用域映射：scope prefix → (bare specifier → URL/path)
    pub scopes: Vec<(String, HashMap<String, String>)>,
}

/// Ctx userdata：存储当前文档的 import map，可被动态更新。
/// 在 `DomExtension::install()` 时创建并注册，`<script type="importmap">` 解析后更新。
#[derive(Clone, rquickjs::JsLifetime)]
pub struct ImportMapState(pub Arc<RwLock<ImportMapData>>);

impl ImportMapState {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ImportMapData::default())))
    }

    pub fn set(&self, data: ImportMapData) {
        *self.0.write().unwrap() = data;
    }

    /// 解析 `name` specifier（从 `base` 模块出发）：
    /// 先查 scopes（最长匹配前缀优先），再查 imports，最后尝试前缀匹配（trailing `/`）。
    /// 未命中返回 None，调用方再做路径拼接。
    pub fn resolve(&self, base: &str, name: &str) -> Option<String> {
        let data = self.0.read().unwrap();

        // scopes：找最长匹配的 scope prefix
        let scope_map = data.scopes.iter()
            .filter(|(scope, _)| base.starts_with(scope.as_str()))
            .max_by_key(|(scope, _)| scope.len())
            .map(|(_, map)| map);

        if let Some(map) = scope_map {
            if let Some(v) = resolve_in_map(map, name) {
                return Some(v);
            }
        }

        // global imports
        resolve_in_map(&data.imports, name)
    }
}

/// 在单个映射表内查找 specifier（精确匹配优先，再尝试 trailing-slash 前缀匹配）
fn resolve_in_map(map: &HashMap<String, String>, name: &str) -> Option<String> {
    if let Some(v) = map.get(name) {
        return Some(v.clone());
    }
    for (k, v) in map {
        if k.ends_with('/') && name.starts_with(k.as_str()) {
            return Some(format!("{}{}", v, &name[k.len()..]));
        }
    }
    None
}
