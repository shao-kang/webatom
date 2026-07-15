use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{
    Ctx, Function, JsLifetime, Persistent, Result, Value,
    module::{Declarations, Exports, ModuleDef},
};
use tokio::sync::oneshot;

use crate::event_loop::event_loop_impl::{EventPortRegistrar, QueueKind};
use crate::extension::{Extension, ExtensionEnv};

// ──────────────────────────────────────────────────────────────
// TimerState — shared between JS callbacks and tokio tasks
// ──────────────────────────────────────────────────────────────

struct TimerStateInner {
    next_id: i32,
    cancel_senders: HashMap<i32, oneshot::Sender<()>>,
}

#[derive(Clone, JsLifetime)]
struct TimerState(Arc<Mutex<TimerStateInner>>);


impl TimerState {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(TimerStateInner {
            next_id: 1,
            cancel_senders: HashMap::new(),
        })))
    }

    fn register(&self) -> (i32, oneshot::Receiver<()>) {
        let mut inner = self.0.lock().unwrap();
        let id = inner.next_id;
        inner.next_id = inner.next_id.wrapping_add(1);
        let (tx, rx) = oneshot::channel();
        inner.cancel_senders.insert(id, tx);
        (id, rx)
    }

    fn cancel(&self, id: i32) {
        if let Some(tx) = self.0.lock().unwrap().cancel_senders.remove(&id) {
            let _ = tx.send(());
        }
    }

    fn complete(&self, id: i32) {
        self.0.lock().unwrap().cancel_senders.remove(&id);
    }
}

// ──────────────────────────────────────────────────────────────
// spawn helpers
// ──────────────────────────────────────────────────────────────

fn spawn_timeout<'js>(
    func: Function<'js>,
    delay: u64,
    state: &TimerState,
    registrar: &EventPortRegistrar,
) -> Result<i32> {
    let (id, mut cancel_rx) = state.register();

    let ctx = func.ctx().clone();
    let persistent: Persistent<Function<'static>> = Persistent::save(&ctx, func);
    let mut registrar = registrar.clone();
    let sender = registrar.register_js_event_port(QueueKind::Macro, move |ctx, _payload| {
        let persistent = persistent.clone();
        let f = persistent.restore(&ctx)?;
        f.call::<_, Value>(())?;
        Ok(())
    });

    let state = state.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                sender.send(());
                state.complete(id);
            }
            _ = &mut cancel_rx => {}
        }
    });
    Ok(id)
}

fn spawn_interval<'js>(
    func: Function<'js>,
    delay: u64,
    state: &TimerState,
    registrar: &EventPortRegistrar,
) -> Result<i32> {
    let (id, mut cancel_rx) = state.register();

    let ctx = func.ctx().clone();
    let persistent: Persistent<Function<'static>> = Persistent::save(&ctx, func);
    let mut registrar = registrar.clone();
    let sender = registrar.register_js_event_port(QueueKind::Macro, move |ctx, _payload| {
        let persistent = persistent.clone();
        let f = persistent.restore(&ctx)?;
        f.call::<_, Value>(())?;
        Ok(())
    });

    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                    sender.send(());
                }
                _ = &mut cancel_rx => { break; }
            }
        }
        state.complete(id);
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
        let state: TimerState = ExtensionEnv::get_state::<TimerState>(ctx)
            .expect("TimerState not registered")
            .deref()
            .clone();
        let registrar: EventPortRegistrar = EventPortRegistrar::from_ctx(ctx)
            .expect("EventPortRegistrar not registered")
            .deref()
            .clone();

        let set_timeout = {
            let state = state.clone();
            let registrar = registrar.clone();
            Function::new(ctx.clone(), move |func: Function<'js>, delay: u64| {
                spawn_timeout(func, delay, &state, &registrar)
            })?
        };

        let set_interval = {
            let state = state.clone();
            let registrar = registrar.clone();
            Function::new(ctx.clone(), move |func: Function<'js>, delay: u64| {
                spawn_interval(func, delay, &state, &registrar)
            })?
        };

        let clear_timeout = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: i32| {
                state.cancel(id);
                Result::<()>::Ok(())
            })?
        };

        let clear_interval = {
            let state = state.clone();
            Function::new(ctx.clone(), move |_ctx: Ctx, id: i32| {
                state.cancel(id);
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

    fn native_setup(&self, env: &mut ExtensionEnv<'_>) {
        env.set_state(TimerState::new());
        env.declare_native_module::<TimerModule>("@webatom/timer");
    }

    fn global_js(&self) -> Option<&'static str> {
        Some(concat!(
            "import { setTimeout, clearTimeout, setInterval, clearInterval } from '@webatom/timer';\n",
            "globalThis.setTimeout = setTimeout;\n",
            "globalThis.clearTimeout = clearTimeout;\n",
            "globalThis.setInterval = setInterval;\n",
            "globalThis.clearInterval = clearInterval;\n",
        ))
    }
}
