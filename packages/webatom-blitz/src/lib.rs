pub mod app;
pub mod renderer;

use anyrender_vello::VelloWindowRenderer;
use blitz_dom::{BaseDocument, DocumentConfig};
use blitz_shell::{BlitzShellProxy, WindowConfig, create_default_event_loop};
use webatom_blitz_msg::BlitzSide;

pub use app::WebAtomApp;

/// 在主线程上启动 Blitz 渲染引擎。
///
/// 接管当前线程直到窗口关闭。调用前须已通过 `create_channels()` 创建 `BlitzSide`。
pub fn run(blitz_side: BlitzSide) {
    let event_loop = create_default_event_loop();
    let (proxy, event_queue) = BlitzShellProxy::new(event_loop.create_proxy());

    let doc = BaseDocument::new(DocumentConfig::default());
    let renderer = VelloWindowRenderer::new();
    let window_config = WindowConfig::new(Box::new(doc), renderer);

    let mut app = Box::new(WebAtomApp::new(proxy, event_queue, blitz_side));
    app.add_window(window_config);
    // winit 0.31.0-beta.2 requires &'static mut A on macOS
    let app: &'static mut WebAtomApp = Box::leak(app);
    event_loop.run_app(app).unwrap();
}
