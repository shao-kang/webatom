use rquickjs::{Ctx, Function, Result, module::{ ModuleDef, Declarations, Exports}};

use crate::extension::{Extension, js_module_init, native_module_init};


pub struct ConsoleModule;

impl ModuleDef for ConsoleModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("print")?;
        decl.declare("eprint")?;

        
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        exports.export("print", Function::new(ctx.clone(), |s: String| {
                println!("{}", s);
                Ok::<(), rquickjs::Error>(())
            })?)?;
        exports.export("eprint", Function::new(ctx.clone(), |s: String| {
                eprintln!("{}", s);
                Ok::<(), rquickjs::Error>(())
            })?)?;
        Ok(())
    }
}
const JS_SOURCE: &str = r#"
            import {eprint, print} from "webatom_ext_native:console"
            globalThis.console = {
                log:   (...args) => print(args.map(String).join(' ')),
                info:  (...args) => print(args.map(String).join(' ')),
                warn:  (...args) => eprint('[WARN] ' + args.map(String).join(' ')),
                error: (...args) => eprint('[ERROR] ' + args.map(String).join(' ')),
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
