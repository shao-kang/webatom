use rquickjs::{Ctx, Result};

use super::Extension;

pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn Extension>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    pub fn register(&mut self, ext: impl Extension + 'static) {
        self.extensions.push(Box::new(ext));
    }

    pub fn apply<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        for ext in &self.extensions {
            ext.globals(ctx)?;
            // module_name() / module_init() integration with the loader pipeline
            // is reserved for a future loader extension point.
        }
        Ok(())
    }
}
