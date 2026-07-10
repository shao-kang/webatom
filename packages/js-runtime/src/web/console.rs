use rquickjs::{Ctx, Function, Result, module::{Declarations, Exports, ModuleDef}};

use crate::extension::{Extension, ExtensionEnv};
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
        exports.export("log", make_log_fn(ctx, "log")?)?;
        exports.export("info", make_log_fn(ctx, "info")?)?;
        exports.export("warn", make_warn_fn(ctx)?)?;
        exports.export("error", make_error_fn(ctx)?)?;
        exports.export("debug", make_debug_fn(ctx)?)?;
        Ok(())
    }
}

fn make_log_fn<'js>(ctx: &Ctx<'js>, _level: &'static str) -> Result<Function<'js>> {
    Function::new(ctx.clone(), |_ctx: Ctx<'_>, s: String| {
        tracing::info!(target: target::JS_CONSOLE, "{s}");
        Ok::<(), rquickjs::Error>(())
    })
}

fn make_warn_fn<'js>(ctx: &Ctx<'js>) -> Result<Function<'js>> {
    Function::new(ctx.clone(), |ctx: Ctx<'_>, s: String| {
        let stack = js_stack_trace(&ctx);
        tracing::warn!(target: target::JS_CONSOLE, "{s}\n{stack}");
        Ok::<(), rquickjs::Error>(())
    })
}

fn make_error_fn<'js>(ctx: &Ctx<'js>) -> Result<Function<'js>> {
    Function::new(ctx.clone(), |ctx: Ctx<'_>, s: String| {
        let stack = js_stack_trace(&ctx);
        eprintln!("[js::console] {s}\n{stack}");
        tracing::error!(target: target::JS_CONSOLE, "{s}\n{stack}");
        Ok::<(), rquickjs::Error>(())
    })
}

fn make_debug_fn<'js>(ctx: &Ctx<'js>) -> Result<Function<'js>> {
    Function::new(ctx.clone(), |_ctx: Ctx<'_>, s: String| {
        tracing::debug!(target: target::JS_CONSOLE, "{s}");
        Ok::<(), rquickjs::Error>(())
    })
}

fn js_stack_trace<'js>(ctx: &Ctx<'js>) -> String {
    (|| -> Option<String> {
        let ctor: rquickjs::Function = ctx.globals().get("Error").ok()?;
        let err: rquickjs::Object = ctor.call(()).ok()?;
        let stack: Option<String> = err.get("stack").ok()?;
        Some(stack?.lines().take(10).collect::<Vec<_>>().join("\n"))
    })()
    .unwrap_or_else(|| "no JS stack trace".to_string())
}

pub struct ConsoleExtension;

impl Extension for ConsoleExtension {
    fn name(&self) -> &'static str {
        "console"
    }

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &["@webatom/console"]
    }

    fn native_setup(&self, env: &mut ExtensionEnv<'_>) {
        env.declare_native_module::<ConsoleModule>("@webatom/console");
    }

    fn global_js(&self) -> Option<&'static str> {
        Some(concat!(
            "import { log, info, warn, error, debug } from '@webatom/console';\n",
            "const _j = (...a) => a.map(String).join(' ');\n",
            "globalThis.console = {\n",
            "  log:   (...a) => log(_j(...a)),\n",
            "  info:  (...a) => info(_j(...a)),\n",
            "  warn:  (...a) => warn(_j(...a)),\n",
            "  error: (...a) => error(_j(...a)),\n",
            "  debug: (...a) => debug(_j(...a)),\n",
            "};\n",
        ))
    }
}
