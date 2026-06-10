use rquickjs::{Ctx, Error, Module, Result};
use rquickjs::loader::Loader;

pub struct FileLoader;

impl Loader for FileLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js>> {
        let source = std::fs::read_to_string(name)
            .map_err(|e| Error::new_loading_message(name, e.to_string()))?;
        Module::declare(ctx.clone(), name, source)
    }
}
