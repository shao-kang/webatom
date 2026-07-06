use rquickjs::{Ctx, Function, Result, module::{Declarations, Exports, ModuleDef}};

use crate::{extension::ExtensionContext, runtime::runtime::JsContext};
use crate::extension::Extension;
use crate::log_targets as target;

pub struct ConsoleModule;

impl ModuleDef for ConsoleModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("log")?;
        decl.declare("info")?;
        decl.declare("warn")?;
        decl.declare("error")?;
        decl.declare("debug")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        exports.export(
            "log",
            Function::new(ctx.clone(), |_ctx: Ctx<'js>, s: String| {
                tracing::info!(target: target::JS_CONSOLE, "{s}");
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;
        exports.export(
            "info",
            Function::new(ctx.clone(), |_ctx: Ctx<'js>, s: String| {
                tracing::info!(target: target::JS_CONSOLE, "{s}");
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;
        exports.export(
            "warn",
            Function::new(ctx.clone(), |ctx: Ctx<'js>, s: String| {
                let stack = js_stack_trace(&ctx);
                tracing::warn!(target: target::JS_CONSOLE, "{s}\n{stack}");
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;
        exports.export(
            "error",
            Function::new(ctx.clone(), |ctx: Ctx<'js>, s: String| {
                let stack = js_stack_trace(&ctx);
                eprintln!("[js::console] {s}\n{stack}");
                tracing::error!(target: target::JS_CONSOLE, "{s}\n{stack}");
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;
        exports.export(
            "debug",
            Function::new(ctx.clone(), |_ctx: Ctx<'js>, s: String| {
                tracing::debug!(target: target::JS_CONSOLE, "{s}");
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;
        Ok(())
    }
}

fn js_stack_trace<'js>(ctx: &Ctx<'js>) -> String {
    (|| -> Option<String> {
        let ctor: rquickjs::Function = ctx.globals().get("Error").ok()?;
        let err: rquickjs::Object = ctor.call(()).ok()?;
        let stack: Option<String> = err.get("stack").ok()?;
        Some(
            stack?
                .lines()
                .take(10)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })()
    .unwrap_or_else(|| "no JS stack trace".to_string())
}

pub struct ConsoleExtension;

impl Extension for ConsoleExtension {
    fn name(&self) -> &'static str {
        "console"
    }

    fn module_specifiers(&self) -> &'static [&'static str] {
        &["@webatom/console"]
    }

    fn install(&self, extension_context: &ExtensionContext<'_, '_>) -> rquickjs::Result<()> {
        rquickjs::Module::declare_def::<ConsoleModule, _>(extension_context.ctx.clone(), "@webatom/console")?;
        Ok(())
    }

    fn get_js_source(&self, specifiers: &str) -> Option<String> {
        match specifiers {
            "@webatom/console" => 
                {
                    Some(concat!(
                    "import * as _c from '@webatom/console';\n",
                    "const _j = (...a) => a.map(String).join(' ');\n",
                    "globalThis.console = {\n",
                    "  log:   (...a) => _c.log(_j(...a)),\n",
                    "  info:  (...a) => _c.info(_j(...a)),\n",
                    "  warn:  (...a) => _c.warn(_j(...a)),\n",
                    "  error: (...a) => _c.error(_j(...a)),\n",
                    "  debug: (...a) => _c.debug(_j(...a)),\n",
                    "};\n").to_string())
                }
            
            _ => None
        }
    }
}
