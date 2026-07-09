use std::path::Path;

use js_runtime::{JsRuntime, web::ConsoleExtension};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    let path = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/entry.js").to_string()
    });

    let abs_path = Path::new(&path)
        .canonicalize()
        .unwrap_or_else(|_| panic!("cannot find file: {}", path));
    let abs_path_str = abs_path.to_string_lossy().into_owned();

    let _source = std::fs::read_to_string(&abs_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", abs_path_str, e));

    let runtime = JsRuntime::builder()
        .with_extension(ConsoleExtension{})
        .build().unwrap();
    let () = runtime.eval("console.log('hello world');").unwrap();

    let num: i32 = runtime.eval("1+1").unwrap();
    println!("{}", num);

    // runtime.eval_module(&abs_path_str, source).unwrap();
    runtime.run().await.unwrap();
}
