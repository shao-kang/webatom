use rquickjs::{Ctx, Function, Result, module::{ ModuleDef, Declarations, Exports}};

use crate::extension::{Extension, js_module_init, native_module_init};


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
        exports.export("info", Function::new(ctx.clone(), |s: String| {
                tracing::info!(kind = "js", "{}", s);
                Ok::<(), rquickjs::Error>(())
            })?)?;
        exports.export("log", Function::new(ctx.clone(), |s: String| {
            tracing::info!(kind = "js", "{}", s);
            Ok::<(), rquickjs::Error>(())
        })?)?;
        exports.export("warn", Function::new(ctx.clone(), |s: String| {
            tracing::warn!(kind = "js", "{}", s);
            Ok::<(), rquickjs::Error>(())
        })?)?;
        exports.export("error", Function::new(ctx.clone(), |s: String| {
            tracing::error!(kind = "js", "{}", s);
            Ok::<(), rquickjs::Error>(())
        })?)?;
       
        Ok(())
    }
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
    native_module_init!(ConsoleModule);
    js_module_init!(JS_SOURCE);
}
