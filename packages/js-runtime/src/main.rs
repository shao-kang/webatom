use std::collections::HashMap;
use std::path::Path;
use rquickjs::{CatchResultExt, Context, Module, Runtime};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "index.js".to_string());

    let abs_path = Path::new(&path)
        .canonicalize()
        .unwrap_or_else(|_| panic!("cannot find file: {}", path));
    let abs_path_str = abs_path.to_string_lossy().into_owned();

    let source = std::fs::read_to_string(&abs_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", abs_path_str, e));

    let runtime = Runtime::new().unwrap();
    js_runtime::setup_module_system(&runtime, HashMap::new());

    let ctx = Context::full(&runtime).unwrap();
    ctx.with(|ctx| {
        js_runtime::setup_console(&ctx).unwrap();
        Module::evaluate(ctx.clone(), abs_path_str.as_str(), source)
            .catch(&ctx)
            .unwrap()
            .finish::<()>()
            .catch(&ctx)
            .unwrap();
    });
}

