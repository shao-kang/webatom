use js_runtime::JsRuntime;
use webatom_blitz_msg::create_channels;
use webatom_extension_dom::{DomExtension, DomExtensionState, html_entry::HtmlEntry};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test/html_entry_script/fixtures/with_inline_script.html"
);

async fn build_with_entry(path: &str) -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let (js_side, _) = create_channels();
    let entry = HtmlEntry::load(path).await.expect("fixture should load");
    let state = DomExtensionState::new(js_side).with_entry(entry);
    JsRuntime::builder()
        .with_extension(DomExtension::with_state(state))
        .build()
        .await
        .expect("runtime should build")
}

/// HTML 中的内联 classic script 在事件循环中执行并设置全局变量
#[tokio::test]
async fn inline_classic_script_sets_global() {
    let mut rt = build_with_entry(FIXTURE).await;

    // 断言脚本在 HTML 脚本之后排入队列（FIFO），run() 自动排干后退出
    rt.schedule_eval(
        "if (globalThis.__html_entry_ok !== true) \
         throw new Error('inline script did not run: __html_entry_ok = ' + globalThis.__html_entry_ok)",
    ).expect("schedule should succeed");

    // 释放 document（持有 DocumentHandle keepalive），使事件循环自然退出
    rt.schedule_eval("globalThis.document = undefined")
        .expect("schedule cleanup");

    rt.run().await.expect("event loop should complete without error");
}

/// 多个 script 标签按文档顺序执行
#[tokio::test]
async fn multiple_scripts_run_in_order() {
    let mut rt = build_with_entry(FIXTURE).await;

    rt.schedule_eval(
        "if (globalThis.__counter !== 1) \
         throw new Error('expected __counter=1, got: ' + globalThis.__counter)",
    ).expect("schedule should succeed");

    // 释放 document（持有 DocumentHandle keepalive），使事件循环自然退出
    rt.schedule_eval("globalThis.document = undefined")
        .expect("schedule cleanup");

    rt.run().await.expect("event loop should complete without error");
}
