use std::path::Path;

use js_runtime::JsRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "./tests/fixtures/entry.js".to_string());

    let abs_path = Path::new(&path)
        .canonicalize()
        .unwrap_or_else(|_| panic!("cannot find file: {}", path));
    let abs_path_str = abs_path.to_string_lossy().into_owned();

    let source = std::fs::read_to_string(&abs_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", abs_path_str, e));

    JsRuntime::builder()
        .build()
        .unwrap()
        .eval_module(&abs_path_str, source)
        .unwrap()
        .run()
        .await
        .unwrap();
}
