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
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            extension_modules: ExtensionModules::new(),
        }
    }

    pub fn register(&mut self, ext: impl Extension + 'static) {
        self.extensions.push(Box::new(ext));
    }

    pub fn apply<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        for ext in &self.extensions {
            self.extension_modules.insert(ext.native_module_name());
            ext.native_module_init(ctx)?;
            self.extension_modules.insert(ext.module_name());
            ext.js_module_init(ctx)?;
        }
        Ok(())
    }
}
