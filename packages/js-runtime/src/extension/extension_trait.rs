use rquickjs::{Ctx, Result};

pub trait Extension {
    fn name(&self) -> &'static str;

    fn globals<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }

    fn module_name(&self) -> Option<&'static str> {
        None
    }

    fn module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }
}
