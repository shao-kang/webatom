use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{Ctx, Function, Persistent, Result, Value};
use tokio::sync::oneshot;

use crate::event_loop::ActiveHandles;

struct TimerState {
    next_id: u32,
    cancel_senders: HashMap<u32, oneshot::Sender<()>>,
}

impl TimerState {
    fn new() -> Self {
        Self { next_id: 1, cancel_senders: HashMap::new() }
    }

    fn register(&mut self) -> (u32, oneshot::Receiver<()>) {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::channel();
        self.cancel_senders.insert(id, tx);
        (id, rx)
    }

    fn cancel(&mut self, id: u32) {
        if let Some(tx) = self.cancel_senders.remove(&id) {
            let _ = tx.send(());
        }
    }

    fn fired(&mut self, id: u32) {
        self.cancel_senders.remove(&id);
    }
}

fn spawn_timeout<'js>(
    ctx: Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
    handles: &ActiveHandles,
) -> Result<u32> {
    let (id, cancel_rx) = state.lock().unwrap().register();
    let guard = handles.acquire();
    let persistent = Persistent::save(&ctx, func);
    let state2 = state.clone();
    let ctx2 = ctx.clone();
    ctx.spawn(async move {
        let _guard = guard;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                state2.lock().unwrap().fired(id);
                if let Ok(f) = persistent.restore(&ctx2) {
                    let _ = f.call::<_, Value>(());
                }
            }
            _ = cancel_rx => {}
        }
    });
    Ok(id)
}

fn spawn_interval<'js>(
    ctx: Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
    handles: &ActiveHandles,
) -> Result<u32> {
    let (id, cancel_rx) = state.lock().unwrap().register();
    let guard = handles.acquire();
    let mut persistent = Persistent::save(&ctx, func);
    let ctx2 = ctx.clone();
    ctx.spawn(async move {
        let _guard = guard;
        let mut cancel_rx = cancel_rx;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                    let Ok(f) = persistent.restore(&ctx2) else { break };
                    persistent = Persistent::save(&ctx2, f.clone());
                    let _ = f.call::<_, Value>(());
                }
                _ = &mut cancel_rx => { break; }
            }
        }
    });
    Ok(id)
}

pub struct TimerExtension;

impl crate::extension::Extension for TimerExtension {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn globals<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        let handles = self.active_handles(ctx);
        let state = Arc::new(Mutex::new(TimerState::new()));

        let set_timeout = {
            let state = state.clone();
            let handles = handles.clone();
            Function::new(ctx.clone(), move |ctx, func, delay: u64| {
                spawn_timeout(ctx, func, delay, &state, &handles)
            })?
        };

        let set_interval = {
            let state = state.clone();
            let handles = handles.clone();
            Function::new(ctx.clone(), move |ctx, func, delay: u64| {
                spawn_interval(ctx, func, delay, &state, &handles)
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
