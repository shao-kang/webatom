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
        if name.starts_with("./") || name.starts_with("../") {
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
