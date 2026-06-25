use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rquickjs::{Ctx, Module, Result};

use crate::event_loop::HostBridge;

use super::Extension;

/// Tracks which module specifiers were declared as native modules,
/// so the resolver can recognise them without going to the filesystem.
#[derive(Clone)]
pub struct ExtensionModules(Arc<Mutex<HashSet<String>>>);

unsafe impl<'js> rquickjs::JsLifetime<'js> for ExtensionModules {
    type Changed<'to> = ExtensionModules;
}

impl ExtensionModules {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashSet::new())))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.lock().unwrap().contains(name)
    }

    pub fn insert(&self, name: String) {
        self.0.lock().unwrap().insert(name);
    }
}

pub struct ExtensionRegistry {
    pub extension_modules: ExtensionModules,
    extensions: Vec<Box<dyn Extension>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            extension_modules: ExtensionModules::new(),
        }
    }

    pub fn register_boxed(&mut self, ext: Box<dyn Extension>) {
        if self.extensions.iter().any(|e| e.name() == ext.name()) {
            tracing::warn!("[webatom] extension '{}' already registered, skipping", ext.name());
            return;
        }
        self.extensions.push(ext);
    }

    pub fn register(&mut self, ext: impl Extension + 'static) {
        if self.extensions.iter().any(|e| e.name() == ext.name()) {
            tracing::warn!("[webatom] extension '{}' already registered, skipping", ext.name());
            return;
        }
        self.extensions.push(Box::new(ext));
    }

    /// Two-step apply:
    /// Step A — `install` all extensions (registers native modules + userdata).
    /// Step B — evaluate `js_glue` for each extension that has one.
    pub fn apply(&self, ctx: &Ctx<'_>, host: &HostBridge) -> Result<()> {
        // Step A
        for ext in &self.extensions {
            for &specifier in ext.native_module_specifiers() {
                self.extension_modules.insert(specifier.to_string());
            }
            ext.install(ctx, host)
                .map_err(|e| rquickjs::Error::new_loading_message(ext.name(), e.to_string()))?;
        }
        // Step B
        for ext in &self.extensions {
            if let Some(glue) = ext.js_glue() {
                let module_name = format!("<{}-glue>", ext.name());
                Module::evaluate(ctx.clone(), module_name, glue)
                    .map_err(|e| rquickjs::Error::new_loading_message(ext.name(), e.to_string()))?
                    .finish::<()>()
                    .map_err(|e| rquickjs::Error::new_loading_message(ext.name(), e.to_string()))?;
            }
        }
        Ok(())
    }
}
