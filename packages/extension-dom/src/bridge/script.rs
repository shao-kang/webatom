use std::collections::HashMap;
use std::sync::Arc;

use rquickjs::{Ctx, Module, Object, Result, Value};
use js_runtime::event_loop::event_loop_impl::{EventPortRegistrar, QueueKind};

use crate::core::{ScriptInfo, ScriptKind};
use crate::dom_extension::DomExtensionState;
use super::import_map::ImportMapState;

/// 根据 `<script>` 元素属性执行脚本（无 registrar 版，用于 ImportMap 和立即执行场景）。
pub(crate) fn execute_script<'js>(
    ctx: &Ctx<'js>,
    registrar: Option<&EventPortRegistrar>,
    info: ScriptInfo,
    node_id: usize,
) -> Result<()> {
    execute_script_with_registrar(ctx, registrar, info, node_id)
}

/// 根据 `<script>` 元素属性执行脚本，语义对齐 MDN 规范。
///
/// - Classic 脚本：立即同步执行
/// - Module 脚本：通过 EventPort 推入宏任务队列（defer 语义）
/// - ImportMap：解析 JSON → 更新 ctx userdata 中的 `ImportMapState`
///
/// `registrar` 为 None 时 module 脚本退化为立即执行（测试场景）。
pub(crate) fn execute_script_with_registrar<'js>(
    ctx: &Ctx<'js>,
    registrar: Option<&EventPortRegistrar>,
    info: ScriptInfo,
    node_id: usize,
) -> Result<()> {
    if info.nomodule {
        return Ok(());
    }
    match info.kind {
        ScriptKind::Classic   => exec_classic(ctx, info),
        ScriptKind::Module    => exec_module(ctx, registrar, info, node_id),
        ScriptKind::ImportMap => exec_importmap(ctx, info),
    }
}

// ── Classic ───────────────────────────────────────────────────────────────────

fn exec_classic<'js>(ctx: &Ctx<'js>, info: ScriptInfo) -> Result<()> {
    if let Some(ref src) = info.src {
        let src = src.trim();
        if src.is_empty() { return Ok(()); }
        let base = ctx.userdata::<DomExtensionState>().and_then(|s| s.base_url().map(str::to_owned));
        let url = resolve_url(src, base.as_deref());
        match load_source(&url) {
            Ok(code) => {
                use rquickjs::CatchResultExt;
                ctx.eval::<(), _>(code.as_bytes())
                    .catch(ctx)
                    .map_err(|e| { tracing::error!(src, "classic script eval error: {e}"); e.throw(ctx) })?;
            }
            Err(e) => tracing::warn!(src, error = %e, "classic script src load failed"),
        }
    } else if !info.content.trim().is_empty() {
        use rquickjs::CatchResultExt;
        ctx.eval::<(), _>(info.content.as_bytes())
            .catch(ctx)
            .map_err(|e| { tracing::error!("inline classic script eval error: {e}"); e.throw(ctx) })?;
    }
    Ok(())
}

// ── Module ────────────────────────────────────────────────────────────────────

fn exec_module<'js>(
    ctx: &Ctx<'js>,
    registrar: Option<&EventPortRegistrar>,
    info: ScriptInfo,
    node_id: usize,
) -> Result<()> {
    if let Some(ref src) = info.src {
        let src = src.trim();
        if src.is_empty() { return Ok(()); }

        let resolved = ctx.userdata::<ImportMapState>()
            .and_then(|state| state.resolve("", src))
            .unwrap_or_else(|| src.to_owned());

        let base = ctx.userdata::<DomExtensionState>().and_then(|s| s.base_url().map(str::to_owned));
        let full_url = resolve_url(&resolved, base.as_deref());

        if full_url.starts_with("http://") || full_url.starts_with("https://") {
            // HTTP 脚本：fetch 在 tokio 任务里跑，拿到 code 后通过 EventPort 推入宏任务
            if let Some(registrar) = registrar {
                let mut registrar = registrar.clone();
                // code 通过 Arc<Mutex<Option>> 共享给 handler（JS 线程）和 tokio task（Send）
                let code_slot: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
                let code_slot_tx = Arc::clone(&code_slot);
                let specifier = full_url.clone();
                let port = registrar.register_js_event_port(QueueKind::Macro, move |ctx, _| {
                    use rquickjs::CatchResultExt;
                    let code = match code_slot.lock().unwrap().take() {
                        Some(c) => c,
                        None => return Ok(()),
                    };
                    Module::evaluate(ctx.clone(), specifier.as_str(), code)
                        .catch(&ctx)
                        .map_err(|e| { tracing::error!(module = specifier.as_str(), "module eval error: {e}"); e.throw(&ctx) })?
                        .finish::<()>()
                        .catch(&ctx)
                        .map_err(|e| { tracing::error!(module = specifier.as_str(), "module finish error: {e}"); e.throw(&ctx) })
                });
                // 只有 port（Send）和 code_slot_tx（Send）进入 async block
                tokio::task::spawn(async move {
                    match reqwest::get(&full_url).await {
                        Ok(resp) => match resp.text().await {
                            Ok(code) => {
                                *code_slot_tx.lock().unwrap() = Some(code);
                                port.send(());
                            }
                            Err(e) => tracing::warn!(url = full_url.as_str(), error = %e, "HTTP module fetch body failed"),
                        },
                        Err(e) => tracing::warn!(url = full_url.as_str(), error = %e, "HTTP module fetch failed"),
                    }
                });
            }
            return Ok(());
        }

        match load_source(&full_url) {
            Ok(code) => eval_module(ctx, registrar, full_url, code),
            Err(e) => {
                tracing::warn!(src, error = %e, "module script src load failed");
                Ok(())
            }
        }
    } else if !info.content.trim().is_empty() {
        let specifier = format!("file:///inline-module-{node_id}.mjs");
        eval_module(ctx, registrar, specifier, info.content)
    } else {
        Ok(())
    }
}

fn eval_module<'js>(
    ctx: &Ctx<'js>,
    registrar: Option<&EventPortRegistrar>,
    specifier: String,
    code: String,
) -> Result<()> {
    if let Some(registrar) = registrar {
        let mut registrar = registrar.clone();
        let port = registrar.register_js_event_port(QueueKind::Macro, move |ctx, _| {
            use rquickjs::CatchResultExt;
            Module::evaluate(ctx.clone(), specifier.as_str(), code.clone())
                .catch(&ctx)
                .map_err(|e| { tracing::error!(module = specifier.as_str(), "module eval error: {e}"); e.throw(&ctx) })?
                .finish::<()>()
                .catch(&ctx)
                .map_err(|e| { tracing::error!(module = specifier.as_str(), "module finish error: {e}"); e.throw(&ctx) })
        });
        port.send(());
    } else {
        // 测试场景：无 registrar，立即执行
        Module::evaluate(ctx.clone(), specifier.as_str(), code)?;
    }
    Ok(())
}

// ── ImportMap ─────────────────────────────────────────────────────────────────

fn exec_importmap<'js>(ctx: &Ctx<'js>, info: ScriptInfo) -> Result<()> {
    let content = info.content.trim();
    if content.is_empty() {
        return Ok(());
    }
    let parse_fn: rquickjs::Function = ctx.eval("JSON.parse")?;
    let value: Value = parse_fn.call((content,))?;
    let Some(obj) = value.as_object() else {
        tracing::warn!("importmap root must be a JSON object");
        return Ok(());
    };
    let imports = extract_string_map(obj.get::<_, Option<Object>>("imports")?)?;
    let scopes  = extract_scopes(obj.get::<_, Option<Object>>("scopes")?)?;
    if let Some(state) = ctx.userdata::<ImportMapState>() {
        state.set(super::import_map::ImportMapData { imports, scopes });
    } else {
        tracing::warn!("ImportMapState not found in ctx userdata; importmap ignored");
    }
    Ok(())
}

fn extract_string_map(obj: Option<Object<'_>>) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    if let Some(obj) = obj {
        for key in obj.keys::<String>() {
            let key = key?;
            if let Some(value) = obj.get::<_, Option<String>>(&key)? {
                map.insert(key, value);
            }
        }
    }
    Ok(map)
}

fn extract_scopes(obj: Option<Object<'_>>) -> Result<Vec<(String, HashMap<String, String>)>> {
    let mut scopes = Vec::new();
    if let Some(obj) = obj {
        for key in obj.keys::<String>() {
            let key = key?;
            let inner: Option<Object> = obj.get(&key)?;
            let map = extract_string_map(inner)?;
            scopes.push((key, map));
        }
    }
    Ok(scopes)
}

// ── 文件加载 ──────────────────────────────────────────────────────────────────

fn load_source(src: &str) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = src.strip_prefix("file://").unwrap_or(src);
    Ok(std::fs::read_to_string(path)?)
}

fn resolve_url(src: &str, base_url: Option<&str>) -> String {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("file://") {
        return src.to_owned();
    }
    if src.starts_with('/') {
        // 绝对路径：相对于 origin（协议+host+port），而非文件系统根目录。
        // base_url 是 http(s) 时取 origin 拼接；file:// 时回退文件系统绝对路径。
        return match base_url {
            Some(base) if base.starts_with("http://") || base.starts_with("https://") => {
                // 取 origin：协议到第三个 `/` 之前的部分
                let origin = base
                    .split_once("://")
                    .and_then(|(scheme, rest)| {
                        let host_end = rest.find('/').map(|i| i + scheme.len() + 3).unwrap_or(base.len());
                        Some(&base[..host_end])
                    })
                    .unwrap_or(base);
                format!("{origin}{src}")
            }
            _ => format!("file://{src}"),
        };
    }
    match base_url {
        Some(base) => {
            let base = base.trim_end_matches('/');
            let rel = src.trim_start_matches("./");
            format!("{base}/{rel}")
        }
        None => src.to_owned(),
    }
}
