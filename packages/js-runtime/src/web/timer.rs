use std::any::Any;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{
    Context, Ctx, Function, JsLifetime, Persistent, Result, Value,
    module::{Declarations, Exports, ModuleDef},
};
use tokio::sync::oneshot;

use crate::event_loop::event_loop_impl::{EventPortRegistrar, TaskType};
use crate::extension::{ContextHandle, Extension, ExtensionEnv};

// ──────────────────────────────────────────────────────────────
// TimerState — shared between JS callbacks and tokio tasks
// ──────────────────────────────────────────────────────────────

struct TimerStateInner {
    next_id: i32,
    cancel_senders: HashMap<i32, oneshot::Sender<()>>,
}

#[derive(Clone)]
struct TimerState(Arc<Mutex<TimerStateInner>>);

unsafe impl<'js> JsLifetime<'js> for TimerState {
    type Changed<'to> = TimerState;
}

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
    ctx: &Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &TimerState,
    context: &Context,
    registrar: &EventPortRegistrar,
) -> Result<i32> {
    let (id, mut cancel_rx) = state.register();

    let persistent: Persistent<Function<'static>> = Persistent::save(ctx, func);
    let handler_context = context.clone();
    let mut registrar = registrar.clone();
    let sender = registrar.register_event_port(TaskType::Macro, move |_payload: &dyn Any| {
        let persistent = persistent.clone();
        let _ = handler_context.with(|ctx| -> Result<()> {
            let f = persistent.restore(&ctx)?;
            f.call::<_, Value>(())?;
            Ok(())
        });
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
    ctx: &Ctx<'js>,
    func: Function<'js>,
    delay: u64,
    state: &TimerState,
    context: &Context,
    registrar: &EventPortRegistrar,
) -> Result<i32> {
    let (id, mut cancel_rx) = state.register();

    let persistent: Persistent<Function<'static>> = Persistent::save(ctx, func);
    let handler_context = context.clone();
    let mut registrar = registrar.clone();
    let sender = registrar.register_event_port(TaskType::Macro, move |_payload: &dyn Any| {
        let persistent = persistent.clone();
        let _ = handler_context.with(|ctx| -> Result<()> {
            let f = persistent.restore(&ctx)?;
            f.call::<_, Value>(())?;
            Ok(())
        });
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
        let context: Context = ExtensionEnv::get_state::<ContextHandle>(ctx)
            .expect("ContextHandle not registered")
            .deref()
            .0
            .clone();
        let registrar: EventPortRegistrar = ExtensionEnv::event_port_registrar(ctx)
            .expect("EventPortRegistrar not registered")
            .deref()
            .clone();

        let set_timeout = {
            let state = state.clone();
            let context = context.clone();
            let registrar = registrar.clone();
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, func: Function<'js>, delay: u64| {
                spawn_timeout(&ctx, func, delay, &state, &context, &registrar)
            })?
        };

        let set_interval = {
            let state = state.clone();
            let context = context.clone();
            let registrar = registrar.clone();
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, func: Function<'js>, delay: u64| {
                spawn_interval(&ctx, func, delay, &state, &context, &registrar)
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
        env.set_state(ContextHandle(env.context()));
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
