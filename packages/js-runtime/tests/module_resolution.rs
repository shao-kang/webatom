use std::collections::HashMap;
use std::path::Path;

use js_runtime::JsRuntime;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[tokio::test]
async fn relative_import_resolves() {
    let entry = fixtures_dir().join("entry.js");
    let source = std::fs::read_to_string(&entry).unwrap();
    let abs = entry.canonicalize().unwrap().to_string_lossy().into_owned();

    let rt = JsRuntime::builder().build().await.unwrap();
    let rt = rt.eval_module(&abs, source).await.unwrap();

    let sum: i32 = rt.eval("__sum").await.unwrap();
    let pi: f64 = rt.eval("__pi").await.unwrap();
    assert_eq!(sum, 3);
    assert!((pi - 3.14159).abs() < 1e-5);
}

#[tokio::test]
async fn bare_import_not_in_map_fails() {
    let rt = JsRuntime::builder().build().await.unwrap();
    let result = rt.eval_module("test.js", "import 'react';").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn import_map_redirects_bare_import() {
    let math_abs = fixtures_dir()
        .join("math.js")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let mut import_map = HashMap::new();
    import_map.insert("my-math".to_string(), math_abs);

    let rt = JsRuntime::builder().import_map(import_map).build().await.unwrap();
    let rt = rt.eval_module(
        "test.js",
        "import { add } from 'my-math'; globalThis.__result = add(10, 5);",
    )
    .await
    .unwrap();

    let result: i32 = rt.eval("__result").await.unwrap();
    assert_eq!(result, 15);
}
