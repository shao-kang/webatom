use std::path::Path;

use js_runtime::JsRuntime;

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

    let source = std::fs::read_to_string(&abs_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", abs_path_str, e));

    let mut runtime = JsRuntime::builder()
        .with_extension(ConsoleExtension:: new())
        .build().unwrap();
    let context = runtime.create_context().await.expect("");
    let num: i32 =context.eval("1+1").await.unwrap();
    println!("{}", num);

    // context.eval_module(&abs_path_str, source).await.unwrap();
    runtime.run().await.unwrap();
}
