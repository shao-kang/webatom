mod document_inner;
mod import_map;
mod script;
mod snapshot;
pub(crate) mod node_handle;
pub(crate) mod document_handle;

pub use node_handle::NodeHandle;
pub use document_handle::DocumentHandle;
pub(crate) use import_map::ImportMapState;

/// HTML 入口初始化：解析 HTML、发送首帧全量快照、按文档顺序调度脚本。
///
/// - ImportMap 脚本立即执行（不依赖 DOM API，js_glue 运行前安全）
/// - Classic / Module 脚本推入宏任务队列，在 js_glue 完成后运行
pub(crate) fn init_html_entry(
    ctx: &rquickjs::Ctx<'_>,
    host: &js_runtime::event_loop::HostBridge,
    state: &crate::dom_extension::DomExtensionState,
) -> rquickjs::Result<()> {
    let html = match state.html_content() {
        Some(h) => h,
        None => return Ok(()),
    };
    let doc = crate::core::Document::parse_html(html);
    state.send_full(doc.to_snapshot());

    for info in doc.all_scripts_in_order() {
        if matches!(info.kind, crate::core::ScriptKind::ImportMap) {
            // 立即执行，更新 ImportMapState，后续模块脚本可使用
            script::execute_script(ctx, Some(host), info, 0)?;
        } else {
            // 延迟到宏任务队列，保证 js_glue DOM API 已就绪
            let _ = host.io.task_tx.try_send(Box::new(move |ctx: rquickjs::Ctx<'_>| {
                script::execute_script(&ctx, None, info, 0)
            }));
        }
    }
    Ok(())
}
