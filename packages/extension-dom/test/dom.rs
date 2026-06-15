use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().register_extension::<DomExtension>().build().await.unwrap()
}


#[tokio::test]
async fn setup_dom_succeeds() {
    build().await;
}