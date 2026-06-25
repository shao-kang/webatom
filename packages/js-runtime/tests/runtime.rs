use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().await.unwrap()
}

#[tokio::test]
async fn run_exits_immediately_when_no_keepalive() {
    let mut rt = build().await;
    rt.run().await.unwrap();
}

#[tokio::test]
async fn eval_before_run_returns_value() {
    let mut rt = build().await;
    let v: i32 = rt.eval("1 + 2").await.unwrap();
    assert_eq!(v, 3);
}

#[tokio::test]
async fn eval_after_run_returns_shutdown_error() {
    let mut rt = build().await;
    rt.run().await.unwrap();
    let err = rt.eval::<i32>("1").await.unwrap_err();
    assert!(matches!(err, js_runtime::JsRuntimeError::Shutdown));
}

#[tokio::test]
async fn eval_module_then_run() {
    let mut rt = build().await;
    rt.eval_module("test.js", "globalThis.__x = 42;").await.unwrap();
    let v: i32 = rt.eval("__x").await.unwrap();
    assert_eq!(v, 42);
    rt.run().await.unwrap();
}

#[tokio::test]
async fn second_run_returns_shutdown_error() {
    let mut rt = build().await;
    rt.run().await.unwrap();
    let err = rt.run().await.unwrap_err();
    assert!(matches!(err, js_runtime::JsRuntimeError::Shutdown));
}
