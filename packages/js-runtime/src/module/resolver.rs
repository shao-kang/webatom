use std::collections::HashMap;
use std::path::Path;

use rquickjs::{Ctx, Error, Result};
use rquickjs::loader::{ImportAttributes, Resolver};

use crate::extension::ExtensionModules;

pub struct EsmResolver {
    pub import_map: HashMap<String, String>,
}

impl EsmResolver {
    pub fn new(import_map: HashMap<String, String>) -> Self {
        Self { import_map }
    }
}

impl Resolver for EsmResolver {
    fn resolve<'js>(&mut self, ctx: &Ctx<'js>, base: &str, name: &str, _attr: Option<ImportAttributes<'js>>) -> Result<String> {
        if let Some(modules) = ctx.userdata::<ExtensionModules>() {
            if modules.contains(name) {
                return Ok(name.to_string());
            }
        }
        // Self-contained / absolute URL schemes — pass through as-is.
        if name.starts_with("data:")
            || name.starts_with("http://")
            || name.starts_with("https://")
            || name.starts_with("file://")
        {
            return Ok(name.to_string());
        }
        if name.starts_with("./") || name.starts_with("../") {
            // Resolve relative to the base, which may itself be a URL.
            if base.starts_with("http://") || base.starts_with("https://") || base.starts_with("file://") {
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
            resolve_file_path(&base_dir.join(name), base, name)
        } else if Path::new(name).is_absolute() {
            Ok(name.to_string())
        } else {
            match self.import_map.get(name).cloned() {
                Some(mapped) => self.resolve(ctx, base, &mapped, None),
                None => Err(Error::new_resolving_message(
                    base,
                    name,
                    format!("bare import '{}' not found in import map", name),
                )),
            }
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
