use std::collections::HashMap;

use rquickjs::{Ctx, Module, Object, Result, Value};
use js_runtime::event_loop::HostBridge;

use crate::core::{ScriptInfo, ScriptKind};
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
        match load_source(src) {
            Ok(code) => ctx.eval::<(), _>(code.as_bytes())?,
            Err(e)   => tracing::warn!(src = src.as_str(), error = %e, "classic script src load failed"),
        }
    } else if !info.content.trim().is_empty() {
        ctx.eval::<(), _>(info.content.as_bytes())?;
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
    let (specifier, code) = if let Some(ref src) = info.src {
        // 先查 import map 解析 src
        let resolved = ctx.userdata::<ImportMapState>()
            .and_then(|state| state.resolve("", src))
            .unwrap_or_else(|| src.clone());

        match load_source(&resolved) {
            Ok(code) => (resolved, code),
            Err(e) => {
                tracing::warn!(src = src.as_str(), error = %e, "module script src load failed");
                return Ok(());
            }
        }
    } else if !info.content.trim().is_empty() {
        // 内联模块：合成 specifier 供 loader 解析模块内的相对 import
        (format!("file:///inline-module-{node_id}.mjs"), info.content)
    } else {
        return Ok(());
    };

    // 浏览器规范：module 默认 defer，文档解析完成后执行
    // 实现：推入 task_tx 宏任务队列，在当前同步代码结束后执行
    if let Some(host) = host {
        let _ = host.io.task_tx.try_send(Box::new(move |ctx: Ctx<'_>| {
            use rquickjs::CatchResultExt;
            Module::evaluate(ctx.clone(), specifier.as_str(), code)
                .catch(&ctx)
                .map_err(|e| e.throw(&ctx))?
                .finish::<()>()
                .catch(&ctx)
                .map_err(|e| e.throw(&ctx))
        }));
    } else {
        // 测试场景（无 HostBridge）：立即执行
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

/// 从 `src` 加载脚本源码。
/// 支持 `file://` URL、绝对路径、相对路径（相对 CWD）。HTTP URL 返回错误。
fn load_source(src: &str) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Err(format!("HTTP external scripts not yet supported: {src}").into());
    }
    let path = src.strip_prefix("file://").unwrap_or(src);
    Ok(std::fs::read_to_string(path)?)
}
