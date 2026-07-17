use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use rquickjs::{Ctx, Error, Result};
use rquickjs::loader::{ImportAttributes, Resolver};

/// 共享的 import map 表，可被 EsmResolver 和 Extension 同时持有。
///
/// - `EsmResolver` 每次 resolve 时 read()
/// - Extension（如 DOM importmap）在运行时 write().insert()
#[derive(Clone, Default)]
pub struct ImportMap( Arc<RwLock<HashMap<String, String>>>);

impl ImportMap {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn with_entries(entries: HashMap<String, String>) -> Self {
        Self(Arc::new(RwLock::new(entries)))
    }

    /// 插入单条映射（bare specifier → URL/path）。
    /// 若 key 已存在则覆盖，并通过 tracing 记录重复警告。
    pub fn insert(&self, specifier: impl Into<String>, target: impl Into<String>) {
        let specifier = specifier.into();
        let target = target.into();
        let mut map = self.0.write().unwrap();
        if let Some(existing) = map.get(&specifier) {
            tracing::error!(
                specifier,
                old = existing.as_str(),
                new = target.as_str(),
                "import map: duplicate specifier, overwriting"
            );
        }
        map.insert(specifier, target);
    }

    /// 内部专用：静默插入（相同 key 直接覆盖，不发 tracing 警告）。
    /// 用于 assemble() 注入扩展内部 specifier 的 identity mapping。
    pub(crate) fn insert_internal(&self, specifier: impl Into<String>, target: impl Into<String>) {
        let mut map = self.0.write().unwrap();
        map.insert(specifier.into(), target.into());
    }

    pub(crate) fn has(&self, specifier: &str) -> bool {
        self.0.read().unwrap().contains_key(specifier)
    }

    /// 批量合并，重复 key 以新值覆盖，并通过 tracing 记录每条重复警告。
    pub fn extend(&self, entries: HashMap<String, String>) {
        let mut map = self.0.write().unwrap();
        for (k, v) in entries {
            if let Some(existing) = map.get(&k) {
                tracing::error!(
                    specifier = k.as_str(),
                    old = existing.as_str(),
                    new = v.as_str(),
                    "import map: duplicate specifier, overwriting"
                );
            }
            map.insert(k, v);
        }
    }

    pub(crate) fn resolve(&self, name: &str) -> Option<String> {
        let map = self.0.read().unwrap();
        // 精确匹配
        if let Some(v) = map.get(name) {
            return Some(v.clone());
        }
        // trailing-slash 前缀匹配（路径包）
        for (k, v) in map.iter() {
            if k.ends_with('/') && name.starts_with(k.as_str()) {
                return Some(format!("{}{}", v, &name[k.len()..]));
            }
        }
        None
    }
}

pub struct EsmResolver {
    import_map: ImportMap,
}

impl EsmResolver {
    pub fn new(import_map: ImportMap) -> Self {
        Self { import_map }
    }
}

impl Resolver for EsmResolver {
    fn resolve<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attr: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        // import map 优先查找（覆盖所有形式的 key：bare specifier、URL、相对路径）
        if let Some(mapped) = self.import_map.resolve(name) {
            // identity mapping（内部 native specifier）：直接返回，不再递归
            if mapped == name {
                return Ok(mapped);
            }
            return self.resolve(ctx, base, &mapped, None);
        }

        // 自描述 URL scheme — 直接透传
        if name.starts_with("data:")
            || name.starts_with("http://")
            || name.starts_with("https://")
            || name.starts_with("file://")
        {
            return Ok(name.to_string());
        }

        // 相对路径
        if name.starts_with("./") || name.starts_with("../") {
            if base.starts_with("http://")
                || base.starts_with("https://")
                || base.starts_with("file://")
            {
                let base_url = base.trim_end_matches('/');
                let base_dir = match base_url.rfind('/') {
                    Some(pos) => &base_url[..pos],
                    None => base_url,
                };
                let rel = name.trim_start_matches("./");
                return Ok(format!("{base_dir}/{rel}"));
            }
            let base_path = Path::new(base);
            let base_dir = if base_path.extension().is_some() {
                base_path.parent().unwrap_or(Path::new("."))
            } else {
                base_path
            };
            return resolve_file_path(&base_dir.join(name), base, name);
        }

        // 绝对路径
        if Path::new(name).is_absolute() {
            return Ok(name.to_string());
        }

        // bare specifier 且不在 import map 中 → 报错
        Err(Error::new_resolving_message(
            base,
            name,
            format!("bare import '{}' not found in import map", name),
        ))
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
