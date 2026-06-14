
#[macro_export]
macro_rules! extension_native_module {
    ($module:ty) => {
        fn native_module_init<'js>(&self, ctx: &rquickjs::Ctx<'js>) -> rquickjs::Result<()> {
            rquickjs::Module::declare_def::<$module, _>(
                ctx.clone(),
                self.native_module_name(),
            )?.eval()?;
            Ok(())
        }
    };
}
#[macro_export]
macro_rules! extension_js_module{
    ($source:expr) => {
        fn js_module_init<'js>(&self, ctx: &rquickjs::Ctx<'js>) -> rquickjs::Result<()> {
            rquickjs::Module::evaluate(ctx.clone(), self.module_name(), $source)?
                .finish::<()>()?;
            Ok(())
        }
    };
}