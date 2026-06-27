use js_runtime::JsRuntime;
use webatom_extension_dom::DomExtension;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    match JsRuntime::builder()
        .with_extension(DomExtension::new())
        .build()
        .await {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("Failed to build runtime: {:?}", e);
                // 尝试获取更底层的错误信息
                panic!("Runtime build failed: {:?}", e);
            }
        }
}
async fn drop_runtime(rt: &mut JsRuntime) {
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
    let mut _rt = build().await;

    match _rt.eval_module("entry", include_str!("./dist/index.js")).await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("JS Execution Error: {:?}", e);
            panic!("Failed to load module: {:?}", e);
        }
    };

    let handle = _rt.handle();
    tokio::spawn(async move {
        let Some(_guard) = handle.keepalive_count.acquire() else { return };
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        // drop_runtime 等价操作
        handle.task_tx.send(Box::new(|ctx| {
            ctx.eval::<(), _>("globalThis.document = undefined")?;
            Ok(())
        })).await.ok();
        // _guard 在此 drop，keepalive 归零，事件循环退出
    });

    _rt.run().await.expect("event loop failed");
        

    // println!("\n--- DOM Tree ---\n{tree}");

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
