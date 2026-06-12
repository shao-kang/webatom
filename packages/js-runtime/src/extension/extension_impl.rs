use rquickjs::{Ctx, Result};

use crate::event_loop::{ActiveHandles, EventLoopHandle};

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

    fn event_loop_handle<'js>(&self, ctx: &Ctx<'js>) -> EventLoopHandle {
        ctx.userdata::<EventLoopHandle>()
            .expect("EventLoopHandle userdata not registered")
            .clone()
    }

    fn active_handles<'js>(&self, ctx: &Ctx<'js>) -> ActiveHandles {
        self.event_loop_handle(ctx).active_handles
    }
}
