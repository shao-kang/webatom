use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use rquickjs::{Ctx, Error, JsLifetime, Result};
use rquickjs::loader::{ImportAttributes, Resolver};

/// 共享的 import map 表，可被 EsmResolver 和 Extension 同时持有。
///
/// base_url 作为固有属性，所有通过 insert/extend 写入的 value
/// 若是相对路径，在写入时立即展开为完整形式，保证存储的值永远可直接使用。
#[derive(Clone)]
pub struct ImportMap {
    base_url: Arc<String>,
    entries: Arc<RwLock<ImportMapEntries>>,
}

#[derive(Default)]
struct ImportMapEntries {
    imports: HashMap<String, String>,
    /// scopes：scope prefix → (specifier → resolved)
    scopes: Vec<(String, HashMap<String, String>)>,
}

impl Default for ImportMap {
    fn default() -> Self {
        Self::new("")
    }
}

impl ImportMap {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: Arc::new(base_url.into()),
            entries: Arc::new(RwLock::new(ImportMapEntries::default())),
        }
    }

    pub fn with_entries(base_url: impl Into<String>, entries: HashMap<String, String>) -> Self {
        let base_url = base_url.into();
        let imports = entries
            .into_iter()
            .map(|(k, v)| (k, normalize_specifier(&v, &base_url).unwrap_or(v)))
            .collect();
        Self {
            base_url: Arc::new(base_url),
            entries: Arc::new(RwLock::new(ImportMapEntries { imports, scopes: Vec::new() })),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 插入单条 imports 映射，value 中的相对路径自动基于 base_url 展开。
    pub fn insert(&self, specifier: impl Into<String>, target: impl Into<String>) {
        let specifier = specifier.into();
        let raw = target.into();
        let target = normalize_specifier(&raw, &self.base_url).unwrap_or(raw);
        let mut e = self.entries.write().unwrap();
        if let Some(existing) = e.imports.get(&specifier) {
            tracing::error!(
                specifier,
                old = existing.as_str(),
                new = target.as_str(),
                "import map: duplicate specifier, overwriting"
            );
        }
        e.imports.insert(specifier, target);
    }

    /// 内部专用：静默插入，跳过规范化（用于 identity mapping）。
    pub(crate) fn insert_internal(&self, specifier: impl Into<String>, target: impl Into<String>) {
        self.entries.write().unwrap().imports.insert(specifier.into(), target.into());
    }

    pub(crate) fn has(&self, specifier: &str) -> bool {
        self.entries.read().unwrap().imports.contains_key(specifier)
    }

    /// 批量合并 imports，value 中的相对路径自动展开。
    pub fn extend(&self, entries: HashMap<String, String>) {
        let base = self.base_url.clone();
        let mut e = self.entries.write().unwrap();
        for (k, v) in entries {
            let v = normalize_specifier(&v, &base).unwrap_or(v);
            if let Some(existing) = e.imports.get(&k) {
                tracing::error!(
                    specifier = k.as_str(),
                    old = existing.as_str(),
                    new = v.as_str(),
                    "import map: duplicate specifier, overwriting"
                );
            }
            e.imports.insert(k, v);
        }
    }

    /// 设置 scopes（来自 `<script type="importmap">` 解析结果）。
    /// value 中的相对路径自动基于 base_url 展开。
    pub fn set_scopes(&self, scopes: Vec<(String, HashMap<String, String>)>) {
        let base = self.base_url.clone();
        let normalized = scopes
            .into_iter()
            .map(|(scope, map)| {
                let map = map
                    .into_iter()
                    .map(|(k, v)| (k, normalize_specifier(&v, &base).unwrap_or(v)))
                    .collect();
                (scope, map)
            })
            .collect();
        self.entries.write().unwrap().scopes = normalized;
    }

    pub(crate) fn resolve(&self, base_module: &str, name: &str) -> Option<String> {
        let e = self.entries.read().unwrap();

        // scopes：最长匹配前缀优先
        let scope_map = e.scopes.iter()
            .filter(|(scope, _)| base_module.starts_with(scope.as_str()))
            .max_by_key(|(scope, _)| scope.len())
            .map(|(_, map)| map);

        if let Some(map) = scope_map {
            if let Some(v) = resolve_in_map(map, name) {
                return Some(v);
            }
        }

        resolve_in_map(&e.imports, name)
    }
}

/// `(base, name) -> Option<resolved_name>`
/// 返回 `Some` 表示命中，`None` 表示交给默认逻辑。
type ResolverFn = Box<dyn FnMut(&str, &str) -> Option<String> + Send>;

pub struct EsmResolver {
    import_map: ImportMap,
    extras: Vec<ResolverFn>,
}

impl EsmResolver {
    pub fn new(import_map: ImportMap) -> Self {
        Self { import_map, extras: Vec::new() }
    }

    /// 注册扩展解析函数，在默认逻辑之前尝试。
    /// 返回 `Some(resolved)` 命中；`None` 继续走默认逻辑。
    pub fn add_resolver<F>(&mut self, f: F)
    where
        F: FnMut(&str, &str) -> Option<String> + Send + 'static,
    {
        self.extras.push(Box::new(f));
    }
}

impl Resolver for EsmResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attr: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        // 1. import map：命中则替换，未命中保留原值
        let name = self.import_map.resolve(base, name)
            .unwrap_or_else(|| name.to_string());

        // 2. 扩展解析器（处理 import map 未覆盖的特殊路径，如 /@id/virtual:...）
        for extra in &mut self.extras {
            if let Some(resolved) = extra(base, &name) {
                return Ok(resolved);
            }
        }

        // 3. 仍是 bare specifier：
        //    - import map 命中且值等于原始 name（identity mapping，native specifier）→ 直接返回
        //    - 否则无法解析 → 报错
        if is_bare_specifier(&name) {
            if self.import_map.has(&name) {
                return Ok(name); // identity mapping
            }
            return Err(Error::new_resolving_message(
                base,
                &name,
                format!("bare import '{}' not found in import map", name),
            ));
        }

        // 4. 规范化路径
        normalize_specifier(&name, base)
    }
}

fn resolve_file_path(path: &Path, base: &str, name: &str) -> Result<String> {
    for candidate in [
        path.to_path_buf(),
        path.with_extension("js"),
        path.with_extension("mjs"),
    ] {
        if let Ok(canonical) = candidate.canonicalize() {
            return Ok(canonical.to_string_lossy().into_owned());
        }
    }
    Err(Error::new_resolving_message(
        base,
        name,
        format!("module not found: {}", path.display()),
    ))
}

/// 将 specifier 规范化为可直接使用的完整形式：
///
/// - 已经是完整形式（URL scheme、bare specifier）→ 原样返回
/// - 相对路径（`./`、`../`）→ 基于 base 展开
/// - 裸绝对路径（`/path`）→ HTTP base 时补全 origin，否则原样
fn normalize_specifier(name: &str, base: &str) -> Result<String> {
    // 已有 scheme，直接透传
    if name.contains("://") || name.starts_with("data:") {
        return Ok(name.to_string());
    }

    // 相对路径
    if name.starts_with("./") || name.starts_with("../") {
        if base.starts_with("http://") || base.starts_with("https://") || base.starts_with("file://") {
            let base_dir = base.trim_end_matches('/');
            let base_dir = match base_dir.rfind('/') {
                Some(pos) => &base_dir[..pos],
                None => base_dir,
            };
            let rel = name.trim_start_matches("./");
            return Ok(format!("{base_dir}/{rel}"));
        }
        // 本地文件系统相对路径
        let base_path = Path::new(base);
        let base_dir = if base_path.extension().is_some() {
            base_path.parent().unwrap_or(Path::new("."))
        } else {
            base_path
        };
        return resolve_file_path(&base_dir.join(name), base, name);
    }

    // 裸绝对路径（/path）
    if name.starts_with('/') {
        if base.starts_with("http://") || base.starts_with("https://") {
            let origin = base
                .split_once("://")
                .map(|(scheme, rest)| {
                    let host_end = rest.find('/').unwrap_or(rest.len());
                    format!("{}://{}", scheme, &rest[..host_end])
                })
                .unwrap_or_else(|| base.to_string());
            return Ok(format!("{origin}{name}"));
        }
        return Ok(name.to_string());
    }

    // 路径类 specifier 都应通过上面的分支处理完毕
    // 此处不应到达（bare specifier 由调用方 is_bare_specifier 拦截）
    Ok(name.to_string())
}

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

/// bare specifier：不以 `/`、`./`、`../` 开头，且不含 `://`，也不是 `data:` URL。
fn is_bare_specifier(name: &str) -> bool {
    !name.starts_with('/')
        && !name.starts_with("./")
        && !name.starts_with("../")
        && !name.contains("://")
        && !name.starts_with("data:")
}
