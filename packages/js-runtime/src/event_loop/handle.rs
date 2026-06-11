use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::sync::Notify;

// ActiveHandles contains only 'static data (Arc<_>), no QuickJS lifetime.
unsafe impl<'js> rquickjs::JsLifetime<'js> for ActiveHandles {
    type Changed<'to> = ActiveHandles;
}

pub struct HandleGuard {
    count: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Drop for HandleGuard {
    fn drop(&mut self) {
        let prev = self.count.fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            self.notify.notify_one();
        }
    }
}

#[derive(Clone)]
pub struct ActiveHandles {
    count: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl ActiveHandles {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn acquire(&self) -> HandleGuard {
        self.count.fetch_add(1, Ordering::Relaxed);
        HandleGuard {
            count: self.count.clone(),
            notify: self.notify.clone(),
        }
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Resolves when active handle count drops to zero.
    pub async fn wait_idle(&self) {
        loop {
            let notified = self.notify.notified();
            if self.count.load(Ordering::Relaxed) == 0 {
                return;
            }
            notified.await;
        }
    }
}
