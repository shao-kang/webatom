use rquickjs::{Ctx, Function, Persistent, Result};

use crate::event_loop::timer::SharedTimers;
use crate::extension::Extension;

pub struct TimerExtension {
    timers: SharedTimers,
}

impl TimerExtension {
    pub fn new(timers: SharedTimers) -> Self {
        Self { timers }
    }
}

impl Extension for TimerExtension {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn globals<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        let globals = ctx.globals();

        let timers_st = self.timers.clone();
        globals.set(
            "setTimeout",
            Function::new(ctx.clone(), move |func: Function<'_>, delay: u64| {
                let ctx = func.ctx().clone();
                let persistent = Persistent::save(&ctx, func);
                let id = timers_st.lock().unwrap().schedule(persistent, delay, None);
                Ok::<u32, rquickjs::Error>(id)
            })?,
        )?;

        let timers_si = self.timers.clone();
        globals.set(
            "setInterval",
            Function::new(ctx.clone(), move |func: Function<'_>, delay: u64| {
                let ctx = func.ctx().clone();
                let persistent = Persistent::save(&ctx, func);
                let id = timers_si
                    .lock()
                    .unwrap()
                    .schedule(persistent, delay, Some(delay));
                Ok::<u32, rquickjs::Error>(id)
            })?,
        )?;

        let timers_ct = self.timers.clone();
        globals.set(
            "clearTimeout",
            Function::new(ctx.clone(), move |id: u32| {
                timers_ct.lock().unwrap().cancel(id);
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;

        let timers_ci = self.timers.clone();
        globals.set(
            "clearInterval",
            Function::new(ctx.clone(), move |id: u32| {
                timers_ci.lock().unwrap().cancel(id);
                Ok::<(), rquickjs::Error>(())
            })?,
        )?;

        Ok(())
    }
}
