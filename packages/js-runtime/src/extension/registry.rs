use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rquickjs::{Ctx, Result};

use super::Extension;

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
    applied: HashSet<String>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            extension_modules: ExtensionModules::new(),
            applied: HashSet::new(),
        }
    }

    pub fn register(&mut self, ext: impl Extension + 'static) {
        if self.extensions.iter().any(|e| e.name() == ext.name()) {
            eprintln!("[webatom] extension '{}' already registered, skipping", ext.name());
            return;
        }
        self.extensions.push(Box::new(ext));
    }

    pub fn apply<'js>(&mut self, ctx: &Ctx<'js>) -> Result<()> {
        for ext in &self.extensions {
            if self.applied.contains(ext.name()) {
                continue;
            }
            self.extension_modules.insert(ext.native_module_name());
            ext.native_module_init(ctx)
                .map_err(|e| rquickjs::Error::new_loading_message(ext.name(), e.to_string()))?;
            self.extension_modules.insert(ext.module_name());
            ext.js_module_init(ctx)
                .map_err(|e| rquickjs::Error::new_loading_message(ext.name(), e.to_string()))?;
            self.applied.insert(ext.name().to_string());
        }
        Ok(())
    }
}
