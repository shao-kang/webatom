# runtime 模块

`runtime` 是 webAtom JS 运行时的顶层组装模块。它将 QuickJS 引擎、EventLoop、Extension 系统三者绑定，提供面向用户的 `JsRuntime` API。

**职责边界：**
- `event_loop/`：执行循环语义、队列调度、microtask checkpoint
- `extension/`：Web API 注册、HostBridge 接口
- `runtime/`：**组装上述两层**，管理生命周期，暴露 eval / run / handle API

---

## 模块结构

| 文件 | 职责 |
|---|---|
| `mod.rs` | `pub use`：对外导出 `JsRuntime`、`RuntimeBuilder` |
| `runtime.rs` | `JsRuntime` 结构体、`eval()`、`run()`、`handle()` |
| `builder.rs` | `RuntimeBuilder`：组装 QuickJS rt + EventLoop + Extensions |

---

## JsRuntime 生命周期（两态）

```
Active（eval 零次或多次）
  │
  └─ run()
        │
        ▼
     Shutdown（terminal state，不可重启）
```

`eval(&mut self)` 和 `run(&mut self)` 均取 `&mut self`，Rust 借用检查器天然保证两者不能并发——"run 期间 eval" 编译即错，无需 CAS / AtomicU8。`Shutdown` 后调 `eval()` / `run()` 返回 `Err(Shutdown)`；重启需重新 `RuntimeBuilder::build()`。

```rust
pub struct JsRuntime {
    rt: Runtime,
    ctx: AsyncContext,
    event_loop: EventLoop,
    shutdown: bool,   // false = Active，true = Shutdown（terminal）
}
```

---

## JsRuntime API

```rust
impl JsRuntime {
    /// eval JS 代码。Shutdown 后返回 Err；&mut self 保证不与 run() 并发（借用检查器）。
    /// Promise job 的执行由 EventLoop checkpoint 保证，不保证在 eval() 返回时已 drain。
    pub async fn eval<T>(&mut self, source: &str) -> Result<T, JsRuntimeError>
    where T: for<'js> FromJs<'js> + Send
    {
        if self.shutdown {
            return Err(JsRuntimeError::Shutdown);
        }
        Ok(self.ctx.with(|ctx| ctx.eval(source)).await?)
    }

    /// 驱动 EventLoop 直到 keepalive_count = 0（所有异步任务完成）。
    /// &mut self 防止与 eval() 并发；返回后进入 Shutdown（不可重启）。
    pub async fn run(&mut self) -> Result<(), JsRuntimeError> {
        if self.shutdown {
            return Err(JsRuntimeError::Shutdown);
        }
        // ShutdownGuard：无论正常返回还是 panic，保证 shutdown = true
        struct ShutdownGuard<'a>(&'a mut bool);
        impl Drop for ShutdownGuard<'_> {
            fn drop(&mut self) { *self.0 = true; }
        }
        let _guard = ShutdownGuard(&mut self.shutdown);
        self.event_loop.run(&self.ctx).await.map_err(Into::into)
    }

    /// 返回 EventLoopHandle clone（KeepAliveCount / Sender 均 Clone）。
    /// 调用方直接持有，无生命周期依赖。
    pub fn handle(&self) -> EventLoopHandle {
        self.event_loop.handle.clone()
    }
}
```

---

## RuntimeBuilder

```rust
pub struct RuntimeBuilder {
    extensions: ExtensionSet,
    render_scheduler: Box<dyn RenderScheduler>,
    event_loop_config: EventLoopConfig,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            render_scheduler: Box::new(HeadlessRenderScheduler),
            event_loop_config: EventLoopConfig::default(),
        }
    }

    pub fn with_extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    /// 添加默认 Extension 集合（console / performance / microtask / timer / rAF / idle）
    pub fn with_default_extensions(mut self) -> Self {
        self.extensions.extend([
            Box::new(ConsoleExtension::default())   as Box<dyn Extension>,
            Box::new(PerformanceExtension)          as Box<dyn Extension>,
            Box::new(MicrotaskExtension)            as Box<dyn Extension>,
            Box::new(TimerExtension { min_delay_ms: 1 }),
            Box::new(AnimationFrameExtension),
            Box::new(IdleCallbackExtension),
        ]);
        self
    }

    /// Phase 1：替换渲染调度器（接入 blitz）
    pub fn with_render_scheduler(mut self, scheduler: impl RenderScheduler + 'static) -> Self {
        self.render_scheduler = Box::new(scheduler);
        self
    }

    pub async fn build(self) -> rquickjs::Result<JsRuntime> {
        let rt = Runtime::new()?;
        let ctx = AsyncContext::full(&rt).await?;

        let event_loop = EventLoop::new(self.event_loop_config, self.render_scheduler);
        let host = event_loop.host.clone();
        // event_loop.host 是 pub(crate) 字段；clone() 无额外分配（均为 Arc clone）

        ctx.with(|ctx| {
            // Phase 0：EventLoop 和 Extension 均不通过 ctx.userdata 访问队列
            //   - EventLoop 直接通过 self.* 访问队列字段
            //   - Extension 通过 HostBridge（方法 API，不经 ctx.userdata）
            // Phase 1：Class 方法需 ctx.userdata 时，在此注册 HostBridge / Realm 资源

            for ext in &self.extensions {
                ext.install(&ctx, &host)?;
            }
            Ok(())
        }).await?;

        Ok(JsRuntime { rt, ctx, event_loop, shutdown: false })
    }
}
```

---

## EventLoop::new — Arc 共享设计

`EventLoop` 在构造时一次性创建所有 Arc，并在内部持有 `pub(crate) host: HostBridge` 字段作为固定的 Capability View。`RuntimeBuilder` 通过 `event_loop.host.clone()` 取得 HostBridge，无需额外包装方法。

```rust
pub struct EventLoop {
    // ...主循环字段（normal_rx, vsync_rx, handle, ...）
    raf_queue: Arc<Mutex<VecDeque<RafTask>>>,
    idle_queue: Arc<Mutex<IdleQueue>>,
    pub(crate) host: HostBridge,  // 与上面 Arc 共享；Builder 通过 .host.clone() 取得
}

impl EventLoop {
    pub fn new(config: EventLoopConfig, render_scheduler: Box<dyn RenderScheduler>) -> Self {
        let raf_queue            = Arc::new(Mutex::new(VecDeque::new()));
        let idle_queue           = Arc::new(Mutex::new(IdleQueue::new()));
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (vsync_tx, vsync_rx) = watch::channel(None);
        let keepalive_count      = KeepAliveCount::new();

        let host = HostBridge {
            scheduler: SchedulerBridge {
                raf:  Arc::clone(&raf_queue),
                idle: Arc::clone(&idle_queue),
            },
            io:      IoBridge      { event_tx: event_tx.clone() },
            runtime: RuntimeBridge { keepalive: keepalive_count.clone() },
        };

        EventLoop {
            normal_rx: event_rx,
            vsync_rx,
            handle: EventLoopHandle { keepalive_count, event_tx, vsync_tx },
            raf_queue,
            idle_queue,
            host,      // 固定视图
            config,
            render_scheduler,
            // ...其余 JS Thread 私有字段
        }
    }

    /// process_event / process_vsync 内部直接使用 self.raf_queue 等字段，
    /// 不通过 ctx.userdata。ctx.userdata 在 Phase 0 不用于 EventLoop 内部访问。
}
```

**Arc 共享不变量：**

| EventLoop 字段 | HostBridge 对应字段 | 关系 |
|---|---|---|
| `raf_queue` | `host.scheduler.raf` | 相同 Arc |
| `idle_queue` | `host.scheduler.idle` | 相同 Arc |
| `handle.keepalive_count` | `host.runtime.keepalive` | 相同 Arc 包装 |
| `handle.event_tx` | `host.io.event_tx` | 相同 channel Sender clone |

---

## ctx.userdata 使用策略

| 阶段 | 注册内容 | 原因 |
|---|---|---|
| Phase 0 | 无 | EventLoop 直接访问字段；Extension 通过 HostBridge |
| Phase 1 | HostBridge（按需） | Class 方法内无法访问外部捕获变量时，通过 `ctx.userdata::<HostBridge>()` |
| Phase 1 | RealmHost（TBD） | Worker Realm 出现后，可能取代 HostBridge 注册 |

**Phase 0 不注册任何 userdata 的理由：**
- EventLoop process_event/process_vsync 直接通过 `self.raf_queue` / `self.idle_queue` 访问队列
- Extension install 时通过显式传入的 `&HostBridge` 参数访问队列
- `ctx.userdata` 是隐式全局访问通道，Phase 0 无需要

---

## Microtask Checkpoint 顺序（规范固化）

**核心原则：`queueMicrotask` 和 `Promise.then` 使用同一个 QJS job queue，FIFO 顺序，符合 Web 规范。**

`queueMicrotask(callback)` 的正确实现不是推入独立的 `PostMicrotaskQueue`，而是将 callback 包装为 QJS job（通过 `Promise.resolve().then(callback)` shim 或 rquickjs job scheduling API），进入与 `Promise.then` 相同的队列。

```
microtask_checkpoint():
  execute_pending_job() loop
  until is_job_pending() = false
  （单队列，无 PostMicrotaskQueue interleave）
```

**排序保证（固化，符合 HTML 规范）：**

```
queueMicrotask(A);
Promise.resolve().then(B);

→ A 先（注册在前，FIFO）
→ B 后

queueMicrotask(A);
Promise.resolve().then(B);
queueMicrotask(C);

→ A → B → C（严格 FIFO）
```

**queueMicrotask 实现方式：**

```rust
// MicrotaskExtension 正确实现（不推 PostMicrotaskQueue）
ctx.globals().set("queueMicrotask", Function::new(ctx.clone(), move |ctx: Ctx<'_>, callback: Function<'_>| {
    // 包装为 resolved Promise job，进入 QJS job queue
    // 保证与 Promise.then 的 FIFO 顺序
    let callback = callback.into_persistent();
    Promise::resolve(ctx.clone(), ())?
        .then(ctx.clone(), Function::new(ctx.clone(), move |ctx: Ctx<'_>| {
            callback.restore(&ctx)?.call::<(), ()>(())?;
            Ok::<(), rquickjs::Error>(())
        })?)?;
    Ok::<(), rquickjs::Error>(())
})?)?;
```

**eval() 阶段产生的 Promise job：** `ctx.eval()` 返回时 QJS job queue 未被 drain。`run()` 的第一次 `flush_microtasks` 执行 checkpoint，这是这些 Promise 的最早执行时机。

---

## eval 与 run 分离

```
runtime.eval::<()>(r#"
    queueMicrotask(A);
    Promise.resolve().then(B);
    setTimeout(C, 0);
"#).await?;
  │  queueMicrotask(A) → Promise::resolve().then(A) → QJS job queue（未 drain）
  │  Promise.then(B)   → QJS job queue（未 drain）
  │  setTimeout(C, 0)  → tokio::spawn → keepalive acquire
  │  eval() 返回，QJS job queue 未 drain
  ▼
runtime.run().await?;
  │  flush_microtasks：execute_pending_job → A → B（同一 QJS job queue，FIFO）
  │  Timer event → dispatch C → microtask checkpoint（空）
  │  keepalive = 0 → should_exit() → begin_shutdown()
  ▼  返回
```

---

## Shutdown 语义

```
EventLoop.run()
  └── loop { ... should_exit()? ... }

should_exit() 全部满足才退出：
  ① keepalive_count.count() == 0   ← 所有 tokio task 完成
  ② normal_rx.is_empty()           ← 无待处理 IO 事件
  ③ !is_job_pending()              ← 无待执行 QJS jobs
        ↓
  keepalive_count.begin_shutdown()  ← 之后所有 acquire() 返回 None
  idle_scheduler.flush(MAX)         ← 退出前清空 idle callbacks
  EventLoop.run() 返回
```

| 模式 | 触发方式 | 适用场景 |
|---|---|---|
| 自然退出 | keepalive = 0，EventLoop 自检 | CLI 脚本、测试 |
| 强制 Shutdown（Phase 1） | `runtime.handle().keepalive_count.begin_shutdown()` | 关闭标签页、Worker terminate |

---

## Phase 1 扩展点

| 特性 | 扩展方式 | 接入点 |
|---|---|---|
| 渲染接入（blitz） | `with_render_scheduler(BlitzRenderScheduler::new(...))` | `RuntimeBuilder` |
| vsync 信号注入 | `runtime.handle().vsync_tx.send(Some(signal))` | `handle()` clone |
| ES Module 加载 | `ctx.runtime().set_loader(resolver, loader)` | `RuntimeBuilder::build()` |
| 强制 Shutdown | `runtime.handle().keepalive_count.begin_shutdown()` | `handle()` clone |
| HostBridge userdata（Class 方法） | `ctx.store_userdata(host.clone())` | `RuntimeBuilder::build()` 按需启用 |
| 多 Realm（Worker） | 新 `AsyncContext` + 独立 `EventLoop::new()`；HostBridge 可能演化为 `RealmHost` | Phase 1 独立设计 |

---

## 关键约束

| 约束 | 说明 |
|---|---|
| eval()/run() 不并发 | 均为 `&mut self`；借用检查器编译期保证，无运行时 CAS |
| Shutdown 返回 Err | eval()/run() 在 shutdown=true 时返回 Err；不是 undefined behavior |
| run() panic 安全 | ShutdownGuard 确保 panic 时 shutdown=true，不残留 Active 状态 |
| EventLoop::new() 唯一 Arc 来源 | RuntimeBuilder 通过 `event_loop.host.clone()` 取 HostBridge，不接触 Arc 类型 |
| ctx.userdata Phase 0 为空 | EventLoop 直接访问字段；Extension 通过 HostBridge；不注册任何 userdata |
| Microtask 单队列 | queueMicrotask 通过 Promise shim 进 QJS job queue；与 Promise.then FIFO 共享 |
| Promise job 由 run() checkpoint 保证 | eval() 返回时 Promise 未必 drain |
| handle() 返回 clone | EventLoopHandle: Clone；调用方无生命周期依赖 |
| EventLoop.host pub(crate) 字段 | Builder 直接访问 .host.clone()，无需 host_bridge() 包装方法 |

---

## 验证场景

```
① 最小 runtime 启动退出
   RuntimeBuilder::new().build().await → run() → keepalive=0 → 立即退出

② Ready 状态多次 eval
   eval("globalThis.x = 1").await?;
   eval("globalThis.x += 1").await?;
   run() → x = 2

③ Shutdown 后 run() 返回 Err
   runtime.run().await?;               // 正常运行并关闭
   runtime.run().await → Err(Shutdown) // shutdown=true，再次调用即返回 Err
   // 注：eval 与 run 并发在编译期即被 &mut self 阻止，不需要运行时检查

④ Shutdown 状态 eval 返回 Err
   runtime.run().await?;
   runtime.eval::<()>("1+1").await → Err(Shutdown)

⑤ Microtask 顺序验证（FIFO，符合 Web 规范）
   eval("queueMicrotask(A); Promise.resolve().then(B)")
   run() → A 先（注册先，同一 QJS job queue FIFO）→ B 后

⑥ Promise drain 时机
   eval("Promise.resolve().then(() => console.log('p'))")
   → eval 返回时未输出
   run() → flush_microtasks → "p" 输出

⑦ with_default_extensions + eval + run
   RuntimeBuilder::new().with_default_extensions().build().await
   eval("setTimeout(() => console.log('t'), 100); requestAnimationFrame(cb)")
   run() → 100ms 后 t 输出；rAF 等 vsync（headless 下不触发）

⑧ handle() vsync 注入
   let h = runtime.handle();
   tokio::spawn(async move { loop { sleep(16ms).await; h.vsync_tx.send(Some(sig))?; } });
   run() → rAF 按帧执行

⑨ ctx.userdata 封装验证（Phase 0）
   RuntimeBuilder::build() 内 ctx.with 闭包中无 store_userdata 调用
   → 无 Arc<Mutex<VecDeque<RafTask>>> 等类型出现在 RuntimeBuilder 代码中
```

---

## 参考资料

- event_loop/CLAUDE.md（EventLoop 结构、主循环、Microtask Checkpoint 实现）
- extension/CLAUDE.md（Extension trait、HostBridge 三层 sub-struct）
- [rquickjs AsyncContext](https://docs.rs/rquickjs/latest/rquickjs/struct.AsyncContext.html)
- [rquickjs Runtime::execute_pending_job](https://docs.rs/rquickjs/latest/rquickjs/struct.Runtime.html)
