use std::collections::HashMap;

use rquickjs::{Ctx, Module, Object, Result, Value};
use js_runtime::event_loop::HostBridge;

use crate::core::{ScriptInfo, ScriptKind};
use crate::dom_extension::DomExtensionState;
use super::import_map::{ImportMapData, ImportMapState};

/// 根据 `<script>` 元素属性执行脚本，语义对齐 MDN 规范。
///
/// - Classic 脚本：立即同步执行
/// - Module 脚本：通过 `HostBridge.task_tx` 延迟为宏任务（近似 defer 语义）
/// - ImportMap：解析 JSON → 更新 ctx userdata 中的 `ImportMapState`
///
/// `host` 为 None 时（测试场景）module 脚本退化为立即执行。
pub(crate) fn execute_script<'js>(
    ctx: &Ctx<'js>,
    host: Option<&HostBridge>,
    info: ScriptInfo,
    node_id: usize,
) -> Result<()> {
    if info.nomodule {
        return Ok(());
    }

    match info.kind {
        ScriptKind::Classic => exec_classic(ctx, info),
        ScriptKind::Module  => exec_module(ctx, host, info, node_id),
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
    host: Option<&HostBridge>,
    info: ScriptInfo,
    node_id: usize,
) -> Result<()> {
    if let Some(ref src) = info.src {
        let src = src.trim();
        if src.is_empty() { return Ok(()); }

        // import map 解析
        let resolved = ctx.userdata::<ImportMapState>()
            .and_then(|state| state.resolve("", src))
            .unwrap_or_else(|| src.to_owned());

        // 拼接 base_url（HTTP 页面的相对路径 → 完整 HTTP URL）
        let base = ctx.userdata::<DomExtensionState>().and_then(|s| s.base_url().map(str::to_owned));
        let full_url = resolve_url(&resolved, base.as_deref());

        if full_url.starts_with("http://") || full_url.starts_with("https://") {
            // HTTP 脚本：异步 fetch，完成后推入 task_tx
            if let Some(host) = host {
                let task_tx = host.io.task_tx.clone();
                tokio::task::spawn(async move {
                    match reqwest::get(&full_url).await {
                        Ok(resp) => match resp.text().await {
                            Ok(code) => {
                                let specifier = full_url.clone();
                                let _ = task_tx.send(Box::new(move |ctx: Ctx<'_>| {
                                    use rquickjs::CatchResultExt;
                                    Module::evaluate(ctx.clone(), specifier.as_str(), code)
                                        .catch(&ctx)
                                        .map_err(|e| { tracing::error!(module = specifier.as_str(), "JS module eval error: {e}"); e.throw(&ctx) })?
                                        .finish::<()>()
                                        .catch(&ctx)
                                        .map_err(|e| { tracing::error!(module = specifier.as_str(), "JS module finish error: {e}"); e.throw(&ctx) })
                                })).await;
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
            Ok(code) => eval_module(ctx, host, full_url, code),
            Err(e) => {
                tracing::warn!(src, error = %e, "module script src load failed");
                Ok(())
            }
        }
    } else if !info.content.trim().is_empty() {
        let specifier = format!("file:///inline-module-{node_id}.mjs");
        eval_module(ctx, host, specifier, info.content)
    } else {
        Ok(())
    }
}

fn eval_module<'js>(ctx: &Ctx<'js>, host: Option<&HostBridge>, specifier: String, code: String) -> Result<()> {
    if let Some(host) = host {
        let _ = host.io.task_tx.try_send(Box::new(move |ctx: Ctx<'_>| {
            use rquickjs::CatchResultExt;
            Module::evaluate(ctx.clone(), specifier.as_str(), code)
                .catch(&ctx)
                .map_err(|e| { tracing::error!(module = specifier.as_str(), "JS module eval error: {e}"); e.throw(&ctx) })?
                .finish::<()>()
                .catch(&ctx)
                .map_err(|e| { tracing::error!(module = specifier.as_str(), "JS module finish error: {e}"); e.throw(&ctx) })
        }));
    } else {
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

    // 用 QuickJS 解析 JSON（安全：JSON.parse 不执行任意代码）
    let parse_fn: rquickjs::Function = ctx.eval("JSON.parse")?;
    let value: Value = parse_fn.call((content,))?;
    let Some(obj) = value.as_object() else {
        tracing::warn!("importmap root must be a JSON object");
        return Ok(());
    };

    let imports = extract_string_map(obj.get::<_, Option<Object>>("imports")?)?;
    let scopes  = extract_scopes(obj.get::<_, Option<Object>>("scopes")?)?;

    if let Some(state) = ctx.userdata::<ImportMapState>() {
        state.set(ImportMapData { imports, scopes });
        tracing::debug!(
            imports = state.0.read().unwrap().imports.len(),
            scopes  = state.0.read().unwrap().scopes.len(),
            "importmap applied"
        );
    } else {
        tracing::warn!("ImportMapState not found in ctx userdata; importmap ignored");
    }

    Ok(())
}

/// `Object` → `HashMap<String, String>`（跳过非字符串值）
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

/// scopes Object → `Vec<(scope_prefix, HashMap<String, String>)>`
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

/// 从本地路径加载脚本源码（HTTP URL 由调用方异步处理，不传入此函数）。
fn load_source(src: &str) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = src.strip_prefix("file://").unwrap_or(src);
    Ok(std::fs::read_to_string(path)?)
}

/// 将 `src` 解析为完整 URL。
/// - 已含协议头（http/https/file）→ 原样返回
/// - 绝对路径 `/foo` → `file:///foo`
/// - 相对路径 `./foo` / `foo` → 追加到 `base_url`
fn resolve_url(src: &str, base_url: Option<&str>) -> String {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("file://") {
        return src.to_owned();
    }
    if src.starts_with('/') {
        return format!("file://{src}");
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
