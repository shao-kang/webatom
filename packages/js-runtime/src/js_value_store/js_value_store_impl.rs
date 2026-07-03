use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use rquickjs::{AsyncContext, Ctx, Persistent, Value};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

type Store = Mutex<HashMap<u64, Persistent<Value<'static>>>>;

struct HandleInner {
    id:      u64,
    drop_tx: UnboundedSender<u64>,
}

impl Drop for HandleInner {
    fn drop(&mut self) {
        // UnboundedSender::send 是同步的，可在任意线程调用
        let _ = self.drop_tx.send(self.id);
    }
}

/// 跨线程 JS 值句柄。所有克隆均 drop 后 ID 自动发往清理任务，由 JS 线程安全释放。
#[derive(Clone)]
pub struct JsValueHandle(Arc<HandleInner>);

/// 线程安全的 JS 值注册中心。
///
/// 使用步骤：
/// 1. `JsValueStore::new()` — 在 AsyncContext 之前创建
/// 2. `store.start(ctx).await` — 拿到 AsyncContext 后调用一次，
///    在 JS 线程 LocalSet 上挂起后台清理任务，handle drop 后自动回收 `Persistent`
#[derive(Clone, rquickjs::JsLifetime)]
pub struct JsValueStore {
    store:   Arc<Store>,
    next_id: Arc<AtomicU64>,
    drop_tx: UnboundedSender<u64>,
    // Option：start() 取走后置 None，防止二次调用
    drop_rx: Arc<Mutex<Option<UnboundedReceiver<u64>>>>,
}

impl JsValueStore {
    pub fn new() -> Self {
        let (drop_tx, drop_rx) = mpsc::unbounded_channel();
        Self {
            store:   Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(0)),
            drop_tx,
            drop_rx: Arc::new(Mutex::new(Some(drop_rx))),
        }
    }

    /// 启动后台清理任务。在 JS 线程 LocalSet 上 `spawn_local` 一个监听循环；
    /// 每当 handle drop 发来 ID，就通过 `async_with` 在 JS 线程安全地 drop 对应 `Persistent`。
    pub async fn start(&self, ctx: AsyncContext) {
        let store = Arc::clone(&self.store);
        let mut rx = self.drop_rx.lock().unwrap()
            .take()
            .expect("JsValueStore::start called more than once");

        let ctx_inner = ctx.clone();
        ctx.async_with(async move |_ctx| {
            // 此时在 JS 线程的 LocalSet 内，spawn_local 合法（!Send 全程 OK）
            tokio::task::spawn_local(async move {
                while let Some(id) = rx.recv().await {
                    let store2 = Arc::clone(&store);
                    ctx_inner.async_with(async move |_| {
                        store2.lock().unwrap().remove(&id);
                        // Persistent 在此 drop，JS 线程安全 ✓
                    }).await;
                }
            });
        }).await;
    }

    /// 在 JS context 内保存值，返回跨线程句柄。
    pub fn save<'js>(&self, ctx: &Ctx<'js>, value: Value<'js>) -> JsValueHandle {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.store.lock().unwrap().insert(id, Persistent::save(ctx, value));
        JsValueHandle(Arc::new(HandleInner {
            id,
            drop_tx: self.drop_tx.clone(),
        }))
    }

    /// 在 JS context 内还原值（句柄仍然有效）。
    pub fn get<'js>(&self, ctx: &Ctx<'js>, handle: &JsValueHandle) -> Option<Value<'js>> {
        self.store.lock().unwrap()
            .get(&handle.0.id)
            .and_then(|p| p.clone().restore(ctx).ok())
    }

    /// 取出并消耗（一次性语义）。
    pub fn take<'js>(&self, ctx: &Ctx<'js>, handle: JsValueHandle) -> Option<Value<'js>> {
        let p = self.store.lock().unwrap().remove(&handle.0.id)?;
        p.restore(ctx).ok()
    }
}

impl Default for JsValueStore {
    fn default() -> Self {
        Self::new()
    }
}
