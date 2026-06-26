use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};

use tokio::sync::{Notify, mpsc, watch};

use super::task::{IdleTask, MacroTask, RafTask};
use super::idle::IdleQueue;
use super::render_scheduler::VsyncSignal;

// ──────────────────────────────────────────────────────────────
// KeepAliveCount + KeepaliveGuard
// ──────────────────────────────────────────────────────────────

struct KeepAliveInner {
    count: AtomicI32,
    idle: Notify,
}

/// RAII guard: increment on creation, decrement on drop.
/// Returned only by `KeepAliveCount::acquire`, so the event loop
/// can rely on count==0 meaning "no more pending work".
pub struct KeepaliveGuard(Arc<KeepAliveInner>);

impl Drop for KeepaliveGuard {
    fn drop(&mut self) {
        let prev = self.0.count.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            // Count just reached 0 — wake the event loop.
            self.0.idle.notify_one();
        }
    }
}

/// Reference-counted keepalive tracker (packed into one AtomicI32).
/// The high bit is used as a shutdown sentinel: once `begin_shutdown`
/// is called the counter is set to `i32::MIN` and `acquire` returns
/// `None` for all subsequent callers.
#[derive(Clone)]
pub struct KeepAliveCount(Arc<KeepAliveInner>);

impl KeepAliveCount {
    pub fn new() -> Self {
        Self(Arc::new(KeepAliveInner {
            count: AtomicI32::new(0),
            idle: Notify::new(),
        }))
    }

    /// Returns a guard that keeps the loop alive, or `None` after shutdown.
    pub fn acquire(&self) -> Option<KeepaliveGuard> {
        let prev = self.0.count.fetch_add(1, Ordering::Acquire);
        if prev < 0 {
            // Already shut down — undo the increment and refuse.
            self.0.count.fetch_sub(1, Ordering::Relaxed);
            None
        } else {
            Some(KeepaliveGuard(self.0.clone()))
        }
    }

    /// Current count (≥0 means active; negative means shutdown).
    pub fn count(&self) -> i32 {
        self.0.count.load(Ordering::Acquire)
    }

    /// Signal shutdown. After this, all new `acquire()` calls return `None`.
    pub fn begin_shutdown(&self) {
        self.0.count.store(i32::MIN, Ordering::Release);
        self.0.idle.notify_one();
    }

    /// Resolves when the active count drops to zero (or shutdown begins).
    pub async fn wait_idle(&self) {
        loop {
            let notified = self.0.idle.notified();
            let c = self.0.count.load(Ordering::Acquire);
            if c <= 0 {
                return;
            }
            notified.await;
        }
    }
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for KeepAliveCount {
    type Changed<'to> = KeepAliveCount;
}

// ──────────────────────────────────────────────────────────────
// RuntimeBridge
// ──────────────────────────────────────────────────────────────

/// Exposes the keepalive mechanism to Extensions.
#[derive(Clone)]
pub struct RuntimeBridge {
    pub keepalive: KeepAliveCount,
}

// ──────────────────────────────────────────────────────────────
// RuntimeIo
// ──────────────────────────────────────────────────────────────

/// Cross-thread channel for posting macro-tasks (timer callbacks, fetch completions, …)
/// back to the JS event loop.
#[derive(Clone)]
pub struct RuntimeIo {
    pub task_tx: mpsc::Sender<MacroTask>,
}

// ──────────────────────────────────────────────────────────────
// SchedulerBridge
// ──────────────────────────────────────────────────────────────

/// Extensions use this to enqueue RAF and idle callbacks without
/// touching EventLoop internals directly.
#[derive(Clone)]
pub struct SchedulerBridge {
    pub raf: Arc<Mutex<VecDeque<RafTask>>>,
    pub idle: Arc<Mutex<IdleQueue>>,
}

impl SchedulerBridge {
    pub fn push_raf(&self, task: RafTask) {
        self.raf.lock().unwrap().push_back(task);
    }

    pub fn push_idle(&self, task: IdleTask) {
        self.idle.lock().unwrap().push(task);
    }
}

// ──────────────────────────────────────────────────────────────
// HostBridge — unified view exposed to Extensions
// ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HostBridge {
    pub runtime: RuntimeBridge,
    pub io: RuntimeIo,
    pub scheduler: SchedulerBridge,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for HostBridge {
    type Changed<'to> = HostBridge;
}

// ──────────────────────────────────────────────────────────────
// EventLoopHandle — clone-able cross-thread handle
// ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EventLoopHandle {
    pub keepalive_count: KeepAliveCount,
    pub task_tx: mpsc::Sender<MacroTask>,
    pub vsync_tx: Arc<watch::Sender<Option<VsyncSignal>>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for EventLoopHandle {
    type Changed<'to> = EventLoopHandle;
}
