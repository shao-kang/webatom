use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};

use tokio::sync::{Notify, mpsc, watch};

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use tokio::sync::Notify;

/// 线程安全的引用计数内核（带有唤醒通知器）
#[derive(Debug)]
pub struct KeepaliveCounter {
    // 原子计数器
    count: AtomicI32,
    // 异步通知器：用于跨线程唤醒主事件循环
    notify: Notify,
}

impl KeepaliveCounter {
    pub fn new() -> Self {
        Self {
            count: AtomicI32::new(0),
            notify: Notify::new(),
        }
    }

    /// 获取当前的存活引用数
    pub fn count(&self) -> i32 {
        self.count.load(Ordering::Acquire)
    }

    /// 暴露给事件循环的异步等待接口
    /// 如果计数器不为 0，事件循环可以挂起在这里等待被唤醒
    pub async fn wait_idle(&self) {
        self.notify.notified().await;
    }
}

// ──────────────────────────────────────────────────────────────
// KeepaliveGuard — 带有主动唤醒逻辑的保活守卫（0 unsafe）
// ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct KeepaliveGuard {
    inner: Arc<KeepaliveCounter>,
}

impl KeepaliveGuard {
    /// 内部构造方法，由 EventLoop 调用
    pub fn new(counter: Arc<KeepaliveCounter>) -> Self {
        counter.count.fetch_add(1, Ordering::SeqCst);
        Self { inner: counter }
    }
}

/// 克隆时计数器 +1
impl Clone for KeepaliveGuard {
    fn clone(&self) -> Self {
        self.inner.count.fetch_add(1, Ordering::SeqCst);
        Self { inner: self.inner.clone() }
    }
}

/// 销毁时计数器 -1，并在归零时触发跨线程唤醒
impl Drop for KeepaliveGuard {
    fn drop(&mut self) {
        // fetch_sub 返回的是减一【之前】的值
        let previous = self.inner.count.fetch_sub(1, Ordering::SeqCst);
        
        // 🎯 核心唤醒逻辑：如果减之前是 1，说明这一次 drop 导致计数【彻底归零】了！
        if previous == 1 {
            // 瞬间发射信号，把正在休眠的事件循环“一巴掌拍醒”
            self.inner.notify.notify_one();
        }

        // 防御性安全检查，杜绝下溢
        if previous <= 0 {
            self.inner.count.store(0, Ordering::SeqCst);
            panic!("KeepaliveGuard 计数发生下溢 (Underflow)！");
        }
    }
}

// ──────────────────────────────────────────────────────────────
// RuntimeBridge
// ──────────────────────────────────────────────────────────────

/// Exposes the keepalive mechanism to Extensions.
#[derive(Clone)]
pub struct RuntimeBridge {
    pub keepalive: KeepAliveCount,
}

// HostBridge — unified view exposed to Extensions
// ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HostBridge {
    pub runtime: RuntimeBridge,
    pub io: RuntimeIo,
    pub scheduler: SchedulerBridge,
}
