/// Console output capture tests.
///
/// tracing does not provide a simple in-process string capture that survives
/// tokio test isolation, so these tests verify the console API shape and
/// that calling it doesn't panic or error.  Actual log output is directed to
/// the test writer via `with_test_writer()` and visible with `-- --nocapture`.
use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().await.unwrap()
}

#[tokio::test]
async fn console_log_single_arg() {
    let mut rt = build().await;
    rt.eval::<()>("console.log('hello')").await.unwrap();
}

#[tokio::test]
async fn console_log_multiple_args_joined() {
    let mut rt = build().await;
    // Should not panic — args are space-joined in the JS glue.
    rt.eval::<()>("console.log('a', 'b', 'c')").await.unwrap();
}

#[tokio::test]
async fn console_log_number_and_bool() {
    let mut rt = build().await;
    rt.eval::<()>("console.log(42, true, null, undefined)").await.unwrap();
}

#[tokio::test]
async fn console_info_works() {
    let mut rt = build().await;
    rt.eval::<()>("console.info('info message')").await.unwrap();
}

#[tokio::test]
async fn console_warn_works() {
    let mut rt = build().await;
    rt.eval::<()>("console.warn('warn message')").await.unwrap();
}

#[tokio::test]
async fn console_error_works() {
    let mut rt = build().await;
    rt.eval::<()>("console.error('error message')").await.unwrap();
}

#[tokio::test]
async fn console_debug_works() {
    let mut rt = build().await;
    rt.eval::<()>("console.debug('debug message')").await.unwrap();
}

#[tokio::test]
async fn console_methods_chainable_in_sequence() {
    let mut rt = build().await;
    rt.eval::<()>(
        r#"
        console.log('log');
        console.info('info');
        console.warn('warn');
        console.error('error');
        console.debug('debug');
        "#,
    )
    .await
    .unwrap();
}
