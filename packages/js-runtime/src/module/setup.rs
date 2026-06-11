use std::collections::HashMap;

use rquickjs::AsyncRuntime;

use super::{EsmResolver, FileLoader};

pub async fn setup_module_system(runtime: &AsyncRuntime, import_map: HashMap<String, String>) {
    runtime.set_loader(EsmResolver::new(import_map), FileLoader).await;
}
