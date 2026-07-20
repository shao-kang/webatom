use rquickjs::Runtime;

use super::{EsmResolver, FileLoader, ImportMap};

/// 为 Runtime 安装模块解析器和加载器。
/// resolver 与 ImportMap 共享同一个 Arc，Extension 可在运行时动态写入。
pub fn setup_module_system(runtime: &Runtime, import_map: ImportMap) {
    runtime.set_loader(EsmResolver::new(import_map), FileLoader::new());
}
