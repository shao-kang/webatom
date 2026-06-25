use std::time::Instant;

use rquickjs::{Ctx, Function, Persistent, Result};

// ──────────────────────────────────────────────────────────────
// RAF task
// ──────────────────────────────────────────────────────────────

// RafTask holds Persistent<Function> inside the closure, which is not Send
// because QuickJS raw pointers are not Send. However, RafTask is only ever
// created, stored, and executed on the single JS thread, so this is safe.
unsafe impl Send for RafTask {}

pub struct RafTask {
    pub callback: Box<dyn for<'js> FnOnce(Ctx<'js>, f64) -> Result<()> + 'static>,
}

impl RafTask {
    pub fn from_js(func: Persistent<Function<'static>>) -> Self {
        Self {
            callback: Box::new(move |ctx, ts| {
                let f = func.restore(&ctx)?;
                let _ = f.call::<_, rquickjs::Value>((ts,));
                Ok(())
            }),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Idle task + deadline
// ──────────────────────────────────────────────────────────────

// IdleTask holds Persistent<Function> inside the closure (same reasoning as RafTask).
unsafe impl Send for IdleTask {}

pub struct IdleTask {
    /// If `Some`, the callback must be called no later than this instant
    /// (mirrors the `timeout` option of `requestIdleCallback`).
    pub timeout_at: Option<Instant>,
    pub callback: Box<dyn for<'js> FnOnce(Ctx<'js>, IdleDeadline) -> Result<()> + 'static>,
}

impl IdleTask {
    pub fn from_js(func: Persistent<Function<'static>>, timeout_at: Option<Instant>) -> Self {
        Self {
            timeout_at,
            callback: Box::new(move |ctx, _deadline| {
                let f = func.restore(&ctx)?;
                let _ = f.call::<_, rquickjs::Value>(());
                Ok(())
            }),
        }
    }
}

/// Passed to `requestIdleCallback` handlers; wraps a deadline instant
/// so JS can call `timeRemaining()`.
pub struct IdleDeadline {
    pub deadline: Instant,
}
