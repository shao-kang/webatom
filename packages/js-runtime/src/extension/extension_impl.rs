use rquickjs::{Ctx, Result};

use crate::event_loop::ActiveHandles;

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

    fn active_handles<'js>(&self, ctx: &Ctx<'js>) -> ActiveHandles {
        ctx.userdata::<ActiveHandles>()
            .expect("ActiveHandles userdata not registered")
            .clone()
    }
}
