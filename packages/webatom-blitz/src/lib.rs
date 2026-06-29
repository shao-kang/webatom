pub mod app;
pub mod renderer;

use anyrender_vello::VelloWindowRenderer;
use blitz_dom::{BaseDocument, DocumentConfig, DocumentMutator};
use blitz_shell::{BlitzShellProxy, WindowConfig, create_default_event_loop};
use markup5ever::{LocalName, Namespace, QualName};
use webatom_blitz_msg::BlitzSide;

pub use app::WebAtomApp;

fn init_document(doc: &mut BaseDocument) {
    let mut m = DocumentMutator::new(doc);
    let html = m.create_element(
        QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from("html")),
        vec![],
    );
    let head = m.create_element(
        QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from("head")),
        vec![],
    );
    let body = m.create_element(
        QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from("body")),
        vec![],
    );
    m.append_children(0, &[html]);
    m.append_children(html, &[head, body]);
}

/// 在主线程上启动 Blitz 渲染引擎。
///
/// 接管当前线程直到窗口关闭。调用前须已通过 `create_channels()` 创建 `BlitzSide`。
pub fn run(blitz_side: BlitzSide) {
    let event_loop = create_default_event_loop();
    let (proxy, event_queue) = BlitzShellProxy::new(event_loop.create_proxy());

    // JS 发送 DomMsg 后自动唤醒 winit 事件循环
    let wake_proxy = proxy.clone();
    blitz_side.set_wake_fn(move || wake_proxy.wake_up());

    let mut doc = BaseDocument::new(DocumentConfig::default());
    init_document(&mut doc);

    let renderer = VelloWindowRenderer::new();
    let window_config = WindowConfig::new(Box::new(doc), renderer);

    let mut app = Box::new(WebAtomApp::new(proxy, event_queue, blitz_side));
    app.add_window(window_config);
    // winit 0.31.0-beta.2 requires &'static mut A on macOS
    let app: &'static mut WebAtomApp = Box::leak(app);
    event_loop.run_app(app).unwrap();
}
