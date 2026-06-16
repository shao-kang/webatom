use rquickjs::{Ctx, Function, Result, module::{ ModuleDef, Declarations, Exports}};

use crate::extension::{Extension};
use webatom_js_runtime_macro::extension_js_module;
use webatom_js_runtime_macro::extension_native_module;

pub struct ConsoleModule;

impl ModuleDef for ConsoleModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("info")?;
        decl.declare("log")?;
        decl.declare("warn")?;
        decl.declare("error")?;

        
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        exports.export("info", Function::new(ctx.clone(), move |_ctx: Ctx<'js>, s: String| {
                // 获取 JS 调用栈
                // let js_stack = get_js_stack_trace(&ctx);
                
                let _span = tracing::info_span!("js",).entered();
                tracing::info!(target: "web_console_info",  message = %s);
                Ok::<(), rquickjs::Error>(())
            })?)?;
        exports.export("log", Function::new(ctx.clone(), move |_ctx: Ctx<'js>, s: String| {
            // 获取 JS 调用栈
            // let js_stack = get_js_stack_trace(&ctx);
            
            let _span = tracing::info_span!("js", ).entered();
            tracing::info!(target: "web_console_log",  message = %s);
            Ok::<(), rquickjs::Error>(())
        })?)?;
        exports.export("warn", Function::new(ctx.clone(), move |ctx: Ctx<'js>, s: String| {
            // 获取 JS 调用栈
            let js_stack = get_js_stack_trace(&ctx);
            
            let _span = tracing::warn_span!("js",  js_stack = %js_stack).entered();
            tracing::warn!(target: "web_console_warn",  message = %s, js_stack = %js_stack);
            Ok::<(), rquickjs::Error>(())
        })?)?;
        exports.export("error", Function::new(ctx.clone(), move |ctx: Ctx<'js>, s: String| {
            // 获取 JS 调用栈
            let js_stack = get_js_stack_trace(&ctx);
            
            let _span = tracing::error_span!("js",  js_stack = %js_stack).entered();
            tracing::error!(target: "web_console_error",  message = %s, js_stack = %js_stack);
            Ok::<(), rquickjs::Error>(())
        })?)?;
       
        Ok(())
    }
}

// 辅助函数：获取 JS 调用栈
fn get_js_stack_trace<'js>(ctx: &Ctx<'js>) -> String {
    // 尝试获取 JS Error 对象的堆栈
    if let Ok(error_constructor) = ctx.globals().get::<_, rquickjs::Function>("Error") {
        if let Ok(error_obj) = error_constructor.call::<_, rquickjs::Object>(()) {
            if let Ok(stack) = error_obj.get::<_, Option<String>>("stack") {
                if let Some(stack_str) = stack {
                    // 只返回 JS 部分的堆栈，过滤掉 Rust 部分
                    return stack_str
                        .lines()
                        .take(10) // 只取前10行，避免过长
                        .filter(|line| !line.contains("webAtom") || line.contains(".js"))
                        .collect::<Vec<_>>()
                        .join("\n");
                }
            }
        }
    }
    
    // 如果无法获取详细的堆栈，则返回一个简单的占位符
    "no JS stack trace".to_string()
}

const JS_SOURCE: &str = r#"
            import {log, info, warn, error} from "webatom_ext_native:console"
            globalThis.console = {
                log:   (...args) => log(args.map(String).join(' ')),
                info:  (...args) => info(args.map(String).join(' ')),
                warn:  (...args) => warn('[WARN] ' + args.map(String).join(' ')),
                error: (...args) => error('[ERROR] ' + args.map(String).join(' ')),
            };
            "#;

pub struct ConsoleExtension;


impl Extension for ConsoleExtension {
    fn name(&self) -> &'static str {
        "console"
    }
    extension_native_module!(ConsoleModule);
    extension_js_module!(JS_SOURCE);
}