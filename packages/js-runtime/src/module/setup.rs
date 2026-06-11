use std::collections::HashMap;

use rquickjs::Runtime;

use super::{EsmResolver, FileLoader};

pub fn setup_module_system(runtime: &Runtime, import_map: HashMap<String, String>) {
    runtime.set_loader(EsmResolver::new(import_map), FileLoader);
}
