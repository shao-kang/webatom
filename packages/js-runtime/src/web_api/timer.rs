use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rquickjs::{Ctx, Function, Persistent, Result, Value};
use crate::event_loop::task::MacroTask;

struct TimerState {
    next_id: u32,
    cancelled: std::collections::HashMap<u32, Arc<AtomicBool>>,
}

impl TimerState {
    fn new() -> Self {
        Self { next_id: 1, cancelled: Default::default() }
    }

    fn register(&mut self) -> (u32, Arc<AtomicBool>) {
        let id = self.next_id;
        self.next_id += 1;
        let flag = Arc::new(AtomicBool::new(false));
        self.cancelled.insert(id, flag.clone());
        (id, flag)
    }

    fn cancel(&mut self, id: u32) {
        if let Some(flag) = self.cancelled.remove(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

fn spawn_timeout<'js>(
    ctx: Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
    macro_queue: &Rc<RefCell<VecDeque<MacroTask>>>,
) -> Result<u32> {
    let (id, cancelled) = state.lock().unwrap().register();
    let persistent = Persistent::save(&ctx, func);
    let queue = macro_queue.clone();
    ctx.spawn(async move {
        tokio::time::sleep(Duration::from_millis(delay)).await;
        if cancelled.load(Ordering::Relaxed) { return; }
        queue.borrow_mut().push_back(MacroTask { func: persistent });
    });
    Ok(id)
}

fn spawn_interval<'js>(
    ctx: Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
) -> Result<u32> {
    let (id, cancelled) = state.lock().unwrap().register();
    let mut persistent = Persistent::save(&ctx, func);
    let ctx2 = ctx.clone();
    ctx.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            if cancelled.load(Ordering::Relaxed) { break; }
            let Ok(f) = persistent.restore(&ctx2) else { break };
            persistent = Persistent::save(&ctx2, f.clone());
            let _ = f.call::<_, Value>(());
        }
    });
    Ok(id)
}

pub struct TimerExtension {
    macro_queue: Rc<RefCell<VecDeque<MacroTask>>>,
}

impl TimerExtension {
    pub fn new(macro_queue: Rc<RefCell<VecDeque<MacroTask>>>) -> Self {
        Self { macro_queue }
    }
}

impl crate::extension::Extension for TimerExtension {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn globals<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        let state = Arc::new(Mutex::new(TimerState::new()));
        let queue = self.macro_queue.clone();

        let set_timeout = {
            let state = state.clone();
            let queue = queue.clone();
            Function::new(ctx.clone(), move |ctx, func, delay: u64| {
                spawn_timeout(ctx, func, delay, &state, &queue)
            })?
        };

        let set_interval = {
            let state = state.clone();
            Function::new(ctx.clone(), move |ctx, func, delay: u64| {
                spawn_interval(ctx, func, delay, &state)
            })?
        };

        let clear_timeout = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: u32| {
                state.lock().unwrap().cancel(id);
                Result::<()>::Ok(())
            })?
        };

        let clear_interval = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: u32| {
                state.lock().unwrap().cancel(id);
                Result::<()>::Ok(())
            })?
        };

        let globals = ctx.globals();
        globals.set("setTimeout", set_timeout)?;
        globals.set("setInterval", set_interval)?;
        globals.set("clearTimeout", clear_timeout)?;
        globals.set("clearInterval", clear_interval)?;
        Ok(())
    }
}
