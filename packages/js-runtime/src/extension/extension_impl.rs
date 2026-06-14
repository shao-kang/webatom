use rquickjs::{ Ctx, Result};

use crate::event_loop::{ActiveHandles, EventLoopHandle};




pub trait Extension {
    fn name(&self) -> &'static str;
    fn native_module_name(&self) -> String {
        format!("webatom_ext_native:{}", self.name())
    }
    fn module_name(&self) -> String {
        format!("webatom_ext_js:{}", self.name())
    }


    fn event_loop_handle<'js>(&self, ctx: &Ctx<'js>) -> EventLoopHandle {
        ctx.userdata::<EventLoopHandle>()
            .expect("EventLoopHandle userdata not registered")
            .clone()
    }

    fn active_handles<'js>(&self, ctx: &Ctx<'js>) -> ActiveHandles {
        self.event_loop_handle(ctx).active_handles
    }
    
    // fn native_module(&self) -> Option<fn(&Ctx<'_>) -> rquickjs::Result<()>> { None }
    fn native_module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()>{ Ok(()) }
    fn js_module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }


}