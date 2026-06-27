use rquickjs::{Ctx, Result};

use crate::core::{ScriptInfo, ScriptKind};

/// 根据 `<script>` 元素属性执行脚本，语义对齐 MDN 规范。
///
/// src 支持：
///   - `file://` URL → 读取本地文件
///   - 绝对 / 相对路径 → 读取本地文件（相对路径以 CWD 为基准）
///   - `http://` / `https://` → Phase 0 暂不支持，打 warn 跳过
///
/// Phase 0 已知偏差：
///   - module defer：规范要求文档解析完后执行，当前近似为立即执行
///   - async / defer on classic+src：当前统一同步加载，不区分异步 / 延迟
///   - importmap：TODO，尚未注册到模块 resolver
pub(crate) fn execute_script<'js>(ctx: &Ctx<'js>, info: ScriptInfo, node_id: usize) -> Result<()> {
    // nomodule：支持 ES 模块的环境应忽略此脚本
    if info.nomodule {
        return Ok(());
    }

    match info.kind {
        ScriptKind::Classic => {
            if let Some(ref src) = info.src {
                match load_source(src) {
                    Ok(code) => {
                        ctx.eval::<(), _>(code.as_bytes())?;
                    }
                    Err(e) => {
                        tracing::warn!(src = src.as_str(), error = %e, "classic script src load failed");
                    }
                }
            } else if !info.content.trim().is_empty() {
                // 内联经典脚本：立即同步执行，全局作用域，var 污染 globalThis
                ctx.eval::<(), _>(info.content.as_bytes())?;
            }
        }

        ScriptKind::Module => {
            if let Some(ref src) = info.src {
                // 外部模块：加载源码后以 src 作为 specifier 执行，
                // 使模块内的相对 import 能被 loader 正确解析
                match load_source(src) {
                    Ok(code) => {
                        rquickjs::Module::evaluate(ctx.clone(), src.as_str(), code)?;
                    }
                    Err(e) => {
                        tracing::warn!(src = src.as_str(), error = %e, "module script src load failed");
                    }
                }
            } else if !info.content.trim().is_empty() {
                // 内联模块：合成 specifier 供 loader 解析模块内相对 import
                // 规范：defer（文档解析完后执行）；当前近似为立即执行
                let specifier = format!("file:///inline-module-{node_id}.mjs");
                rquickjs::Module::evaluate(ctx.clone(), specifier, info.content)?;
            }
        }

        ScriptKind::ImportMap => {
            if !info.content.trim().is_empty() {
                // 规范：必须在所有 module script 之前出现，支持 "imports"/"scopes" 字段
                // TODO: 解析 JSON → 注册到模块 resolver
                tracing::debug!("importmap encountered (not yet applied)");
            }
        }
    }

    Ok(())
}

/// 从 `src` 加载脚本源码。
/// 支持 `file://` URL、绝对路径、相对路径（相对 CWD）。
/// HTTP URL 返回错误。
fn load_source(src: &str) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Err(format!("HTTP external scripts not yet supported: {src}").into());
    }
    let path = src.strip_prefix("file://").unwrap_or(src);
    Ok(std::fs::read_to_string(path)?)
}
