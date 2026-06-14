use rquickjs::{module::{Declarations, Exports, ModuleDef}, Class, Ctx, Result};
use js_runtime::Extension;
use js_runtime::native_module_init;
// use js_runtime::Extension::js_module_init;

use crate::bridge::{DocumentHandle, NodeHandle};

pub struct DomModule;

impl ModuleDef for DomModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("DocumentHandle")?;
        decl.declare("NodeHandle")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        Class::<DocumentHandle>::define(&ctx.globals())?;
        Class::<NodeHandle>::define(&ctx.globals())?;
        exports.export("DocumentHandle", Class::<DocumentHandle>::create_constructor(ctx)?)?;
        exports.export("NodeHandle", Class::<NodeHandle>::create_constructor(ctx)?)?;
        Ok(())
    }
}

pub struct DomExtension;

impl Extension for DomExtension {
    fn name(&self) -> &'static str {
        "dom"
    }
    native_module_init!(DomModule);
    // js_module_init!("");

  

    // fn module_init<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
    //     rquickjs::Module::declare_def::<DomModule, _>(ctx.clone(), "webatom-native:dom")?.eval()?;
    //     Ok(())
    // }
}