use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rquickjs::{Context, Ctx, Function, Persistent, Result, Runtime, Value};

use super::task::MacroTask;
use super::timer::{PendingTimer, SharedTimers, TimerHeapEntry, TimerState};

pub struct EventLoop {
    // Persistent<Function> holders must be dropped before the Runtime.
    macro_queue: VecDeque<MacroTask>,
    timers: SharedTimers,
    runtime: Runtime,
}

impl EventLoop {
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            macro_queue: VecDeque::new(),
            timers: Arc::new(Mutex::new(TimerState::new())),
        }
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn timers(&self) -> SharedTimers {
        self.timers.clone()
    }

    fn drain_timers(&mut self) {
        let now = Instant::now();
        let mut state = self.timers.lock().unwrap();
        while let Some(entry) = state.heap.peek() {
            if entry.deadline > now {
                break;
            }
            let entry = state.heap.pop().unwrap();
            let Some(idx) = state.pending.iter().position(|t| t.id == entry.id) else {
                continue;
            };
            if state.pending[idx].cancelled {
                state.pending.remove(idx);
                continue;
            }
            let timer = state.pending.remove(idx);
            self.macro_queue.push_back(MacroTask::Timer {
                id: timer.id,
                func: timer.func,
                interval: timer.interval,
            });
        }
    }

    fn flush_microtasks(&self) {
        loop {
            match self.runtime.execute_pending_job() {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    e.0.with(|ctx| {
                        if let Some(exc) = ctx.catch().as_exception() {
                            eprintln!("[EventLoop] uncaught rejection: {exc}");
                        }
                    });
                    break;
                }
            }
        }
    }

    fn tick<'js>(&mut self, ctx: &Ctx<'js>) -> Result<bool> {
        self.drain_timers();

        let Some(task) = self.macro_queue.pop_front() else {
            return Ok(false);
        };

        match task {
            MacroTask::Timer { id, func, interval } => {
                let f: Function<'js> = func.restore(ctx)?;
                f.call::<_, Value>(())?;

                if let Some(period) = interval {
                    let persistent = Persistent::save(ctx, f);
                    let mut state = self.timers.lock().unwrap();
                    state.pending.push(PendingTimer {
                        id,
                        func: persistent,
                        interval: Some(period),
                        cancelled: false,
                    });
                    state.heap.push(TimerHeapEntry {
                        deadline: Instant::now() + period,
                        id,
                    });
                }
            }
        }

        Ok(true)
    }

    pub fn is_pending(&self) -> bool {
        !self.macro_queue.is_empty()
            || self.timers.lock().unwrap().has_pending()
            || self.runtime.is_job_pending()
    }

    pub async fn run(&mut self, ctx: &Context) -> Result<()> {
        self.flush_microtasks();

        while self.is_pending() {
            if self.macro_queue.is_empty() && !self.runtime.is_job_pending() {
                let next = self.timers.lock().unwrap().next_deadline();
                if let Some(deadline) = next {
                    let now = Instant::now();
                    if deadline > now {
                        tokio::time::sleep(deadline - now).await;
                    }
                }
            }
            ctx.with(|js_ctx| self.tick(&js_ctx))?;
            // Flush microtasks outside ctx.with() to avoid RefCell double-borrow.
            self.flush_microtasks();
        }

        Ok(())
    }
}
