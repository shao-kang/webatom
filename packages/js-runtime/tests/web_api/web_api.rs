use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().await.unwrap()
}

#[tokio::test]
async fn setup_web_api_succeeds() {
    build().await;
}

#[tokio::test]
async fn console_object_is_available() {
    let mut rt = build().await;
    let result: bool = rt.eval("typeof globalThis.console === 'object'").await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn console_has_expected_methods() {
    let mut rt = build().await;
    let result: bool = rt
        .eval("['log','info','warn','error'].every(m => typeof console[m] === 'function')")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn console_log_does_not_panic() {
    let mut rt = build().await;
    rt.eval::<()>("console.log('hello', 42, true)").await.unwrap();
}

#[tokio::test]
async fn console_warn_error_do_not_panic() {
    let mut rt = build().await;
    rt.eval::<()>("console.warn('warn msg'); console.error('error msg')")
        .await
        .unwrap();
}
