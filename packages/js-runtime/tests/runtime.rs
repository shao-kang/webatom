use js_runtime::JsRuntime;

fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().unwrap()
}

#[tokio::test]
async fn run_exits_immediately_when_no_keepalive() {
    let rt = build();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn eval_before_run_returns_value() {
    let rt = build();
    let v: i32 = rt.eval("1 + 2").unwrap();
    assert_eq!(v, 3);
    rt.run().await.unwrap();
}

#[tokio::test]
async fn eval_module_then_run() {
    let rt = build();
    rt.eval_module("test.js", "globalThis.__x = 42;").unwrap();
    let v: i32 = rt.eval("__x").unwrap();
    assert_eq!(v, 42);
    rt.run().await.unwrap();
}
