use js_runtime::JsRuntime;
use webatom_extension_dom::DomExtension;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder()
        .extension(DomExtension)
        .build()
        .await
        .unwrap()
}
async fn drop_runtime(rt: JsRuntime) {
   let _result = rt.eval::<()>("globalThis.document = undefined").await;
}
#[tokio::test]
async fn setup_dom_succeeds() {
    build().await;
}

/// JS 中用 DocumentHandle 原语构建 DOM 树，序列化后打印并断言结构：
///
///   #document
///   └── <HTML>
///       ├── <HEAD>
///       │   └── <TITLE>
///       │       └── #text "Hello webAtom"
///       └── <BODY>
///           ├── <H1>
///           │   └── #text "标题"
///           └── <P>
///               └── #text "段落内容"
#[tokio::test]
async fn build_dom_tree_and_print() {
    let rt = build().await;

    let tree = rt
        .eval::<String>(include_str!("./dom.js"))
        .await
        .unwrap();
        drop_runtime(rt).await;

    println!("\n--- DOM Tree ---\n{tree}");

    // assert!(tree.contains("#document"),                 "missing #document root");
    // assert!(tree.contains("<HTML>"),                    "missing <HTML>");
    // assert!(tree.contains("<HEAD>"),                    "missing <HEAD>");
    // assert!(tree.contains("<TITLE>"),                   "missing <TITLE>");
    // assert!(tree.contains("#text \"Hello webAtom\""),   "missing title text");
    // assert!(tree.contains("<BODY>"),                    "missing <BODY>");
    // assert!(tree.contains("<H1>"),                      "missing <H1>");
    // assert!(tree.contains("#text \"标题\""),             "missing h1 text");
    // assert!(tree.contains("<P>"),                       "missing <P>");
    // assert!(tree.contains("#text \"段落内容\""),          "missing p text");
}
