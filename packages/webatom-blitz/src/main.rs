use std::sync::Arc;

use js_runtime::JsRuntime;
use webatom_blitz_msg::create_channels;
use webatom_extension_dom::{DomExtension, DomExtensionState, html_entry::HtmlEntry};

fn main() {
    tracing_subscriber::fmt::init();
    let (js_side, blitz_side) = create_channels();

    // JS 线程：tokio current_thread 运行时 + QuickJS 事件循环。
    // 必须在独立 std 线程运行，主线程留给 Blitz/winit（macOS 强制要求）。
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            // 从命令行读取 HTML 入口路径；无参数时使用内置默认页面
            let entry: Arc<HtmlEntry> = match std::env::args().nth(1) {
                Some(path) => HtmlEntry::load(path).await.expect("HTML 入口加载失败"),
                None => Arc::new(HtmlEntry {
                    url:      "file:///index.html".to_string(),
                    base_url: "file:///".to_string(),
                    content:  include_str!("../assets/index.html").to_string(),
                }),
            };

            let state = DomExtensionState::new(js_side).with_entry(entry);

            let mut js_rt = JsRuntime::builder()
                .with_extension(DomExtension::with_state(state))
                .build()
                .await
                .expect("JS 运行时初始化失败");
            let _ =js_rt.schedule_eval("console.log('hello worldAtom')");

            if let Err(e) = js_rt.run().await {
                eprintln!("JS 事件循环错误: {e}");
            }
        });
    });

    // 主线程运行 Blitz（阻塞直到窗口关闭）
    webatom_blitz::run(blitz_side);
}
