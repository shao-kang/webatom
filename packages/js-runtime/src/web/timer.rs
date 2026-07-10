use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{Ctx, Function, Persistent, Result, Value, module::{Declarations, Exports, ModuleDef}};
use tokio::sync::oneshot;

use crate::event_loop::{};
use crate::extension::Extension;

// Persistent<T> holds a *mut JSRuntime which is !Send by default.
// MacroTask closures are always executed inside context.with() on the JS thread,
// so moving a Persistent across threads to deliver it is safe.
struct SendPersistent<T>(Persistent<T>);
unsafe impl<T> Send for SendPersistent<T> {}

impl<T: rquickjs::JsLifetime<'static>> SendPersistent<T> {
    fn restore<'js>(self, ctx: &Ctx<'js>) -> rquickjs::Result<T::Changed<'js>> {
        self.0.restore(ctx)
    }
}

impl<T: Clone> Clone for SendPersistent<T> {
    fn clone(&self) -> Self {
        SendPersistent(self.0.clone())
    }
}

// ──────────────────────────────────────────────────────────────
// TimerState — shared between JS callbacks and tokio tasks
// ──────────────────────────────────────────────────────────────

struct TimerState {
    next_id: i32,
    cancel_senders: HashMap<i32, oneshot::Sender<()>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for TimerState {
    type Changed<'to> = TimerState;
}

impl TimerState {
    fn new() -> Self {
        Self { next_id: 1, cancel_senders: HashMap::new() }
    }

    fn register(&mut self) -> (i32, oneshot::Receiver<()>) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let (tx, rx) = oneshot::channel();
        self.cancel_senders.insert(id, tx);
        (id, rx)
    }

    fn cancel(&mut self, id: i32) {
        if let Some(tx) = self.cancel_senders.remove(&id) {
            let _ = tx.send(());
        }
    }

    fn complete(&mut self, id: i32) {
        self.cancel_senders.remove(&id);
    }
}

// ──────────────────────────────────────────────────────────────
// spawn helpers
// ──────────────────────────────────────────────────────────────

fn spawn_timeout<'js>(
    ctx: &Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
    host: &HostBridge,
) -> Result<i32> {
    let (id, mut cancel_rx) = state.lock().unwrap().register();
    let keepalive = match host.runtime.keepalive.acquire() {
        Some(g) => g,
        None => return Ok(id),
    };
    let persistent = SendPersistent(Persistent::save(ctx, func));
    let task_tx = host.io.task_tx.clone();
    let state2 = state.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                let task: MacroTask = Box::new(move |ctx| {
                    let f = persistent.restore(&ctx)?;
                    let _ = f.call::<_, Value>(());
                    drop(keepalive);
                    Ok(())
                });
                tokio::select! {
                    _ = task_tx.send(task) => { state2.lock().unwrap().complete(id); }
                    _ = &mut cancel_rx => {}
                }
            }
            _ = &mut cancel_rx => {}
        }
    });
    Ok(id)
}

fn spawn_interval<'js>(
    ctx: &Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &Arc<Mutex<TimerState>>,
    host: &HostBridge,
) -> Result<i32> {
    let (id, cancel_rx) = state.lock().unwrap().register();
    let keepalive = match host.runtime.keepalive.acquire() {
        Some(g) => g,
        None => return Ok(id),
    };
    let persistent = SendPersistent(Persistent::save(ctx, func));
    let task_tx = host.io.task_tx.clone();
    let state2 = state.clone();

    tokio::spawn(async move {
        let _keepalive = keepalive;
        let mut cancel_rx = cancel_rx;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                    let func = persistent.clone();
                    let task: MacroTask = Box::new(move |ctx| {
                        let f = func.restore(&ctx)?;
                        let _ = f.call::<_, Value>(());
                        Ok(())
                    });
                    tokio::select! {
                        _ = task_tx.send(task) => {}
                        _ = &mut cancel_rx => { break; }
                    }
                }
                _ = &mut cancel_rx => { break; }
            }
        }
        state2.lock().unwrap().complete(id);
    });
    Ok(id)
}

// ──────────────────────────────────────────────────────────────
// TimerModule — native module exposed as "@webatom/timer"
// ──────────────────────────────────────────────────────────────

pub struct TimerModule;

impl ModuleDef for TimerModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("setTimeout")?;
        decl.declare("clearTimeout")?;
        decl.declare("setInterval")?;
        decl.declare("clearInterval")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        let state = ctx.userdata::<Arc<Mutex<TimerState>>>()
            .expect("TimerState not registered")
            .clone();
        let host = ctx.userdata::<HostBridge>()
            .expect("HostBridge not registered")
            .clone();

        let set_timeout = {
            let state = state.clone();
            let host = host.clone();
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, func: Function<'js>, delay: u64| {
                spawn_timeout(&ctx, func, delay, &state, &host)
            })?
        };

        let set_interval = {
            let state = state.clone();
            let host = host.clone();
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, func: Function<'js>, delay: u64| {
                spawn_interval(&ctx, func, delay, &state, &host)
            })?
        };

        let clear_timeout = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: i32| {
                state.lock().unwrap().cancel(id);
                Result::<()>::Ok(())
            })?
        };

        let clear_interval = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: i32| {
                state.lock().unwrap().cancel(id);
                Result::<()>::Ok(())
            })?
        };

        exports.export("setTimeout", set_timeout)?;
        exports.export("clearTimeout", clear_timeout)?;
        exports.export("setInterval", set_interval)?;
        exports.export("clearInterval", clear_interval)?;
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────
// TimerExtension
// ──────────────────────────────────────────────────────────────

pub struct TimerExtension;

impl Extension for TimerExtension {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &["@webatom/timer"]
    }

    fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        ctx.store_userdata(Arc::new(Mutex::new(TimerState::new())))?;
        rquickjs::Module::declare_def::<TimerModule, _>(ctx.clone(), "@webatom/timer")?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(concat!(
            "import { setTimeout, clearTimeout, setInterval, clearInterval } from '@webatom/timer';\n",
            "globalThis.setTimeout = setTimeout;\n",
            "globalThis.clearTimeout = clearTimeout;\n",
            "globalThis.setInterval = setInterval;\n",
            "globalThis.clearInterval = clearInterval;\n",
        ))
    }
}
