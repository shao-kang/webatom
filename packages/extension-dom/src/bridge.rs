mod document_inner;
mod import_map;
mod script;
mod snapshot;
pub(crate) mod node_handle;
pub(crate) mod document_handle;

pub use node_handle::NodeHandle;
pub use document_handle::DocumentHandle;
pub(crate) use import_map::ImportMapState;

use js_runtime::extension::ExtensionEnv;
use js_runtime::event_loop::event_loop_impl::{EventPortRegistrar, QueueKind};
use crate::dom_extension::DomExtensionState;

/// HTML 入口初始化：解析 HTML、发送首帧全量快照、按文档顺序调度脚本。
///
/// - ImportMap 脚本立即执行（不依赖 DOM API，js_glue 运行前安全）
/// - Classic / Module 脚本通过 EventPort 推入宏任务队列，在 js_glue 完成后运行
pub(crate) fn init_html_entry(
    env: &mut ExtensionEnv<'_>,
    state: &DomExtensionState,
) {
    let html = match state.html_content() {
        Some(h) => h,
        None => return,
    };
    let doc = crate::core::Document::parse_html(html);
    state.send_full(doc.to_snapshot());

    let context = env.context();

    for info in doc.all_scripts_in_order() {
        if matches!(info.kind, crate::core::ScriptKind::ImportMap) {
            // ImportMap 立即执行，后续模块脚本可使用映射
            let _ = context.with(|ctx| script::execute_script(&ctx, None, info, 0));
        } else {
            // 每条脚本注册一个一次性宏任务端口，立即 send 触发，handler 执行后 port drop → handler 自动注销
            let port = env.register_js_event_port(QueueKind::Macro, {
                move |ctx, _payload: &dyn std::any::Any| {
              
                    // 在 JS 线程执行时，从 ctx 取 registrar 供 module 脚本内部异步加载使用
                    let registrar = EventPortRegistrar::from_ctx(&ctx).map(|g| (*g).clone());
                    script::execute_script_with_registrar(&ctx, registrar.as_ref(), info.clone(), 0)
            
                }
            });
            port.send(());
            // port drop → EventSenderGuard drop → cleanup 信号发出 → handler 自动注销
        }
    }
}
