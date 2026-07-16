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
pub struct ImportMap(pub Arc<RwLock<HashMap<String, String>>>);

impl ImportMap {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn with_entries(entries: HashMap<String, String>) -> Self {
        Self(Arc::new(RwLock::new(entries)))
    }

    /// 插入单条映射（bare specifier → URL/path）。
    pub fn insert(&self, specifier: impl Into<String>, target: impl Into<String>) {
        self.0.write().unwrap().insert(specifier.into(), target.into());
    }

    /// 批量合并，重复 key 以新值覆盖。
    pub fn extend(&self, entries: HashMap<String, String>) {
        self.0.write().unwrap().extend(entries);
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

        // bare specifier → 查 import map（运行时动态）
        match self.import_map.resolve(name) {
            Some(mapped) => self.resolve(ctx, base, &mapped, None),
            None => Err(Error::new_resolving_message(
                base,
                name,
                format!("bare import '{}' not found in import map", name),
            )),
        }
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
