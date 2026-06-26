# extension 模块

Extension 系统是 JS 运行时与 Web API 之间的桥梁。每个 Extension 负责将一组相关的 Web API 注册到 QuickJS context，并通过 `HostBridge` 与事件循环交互。

**核心设计约束：** Extension 只依赖 `HostBridge`，不直接访问 `ctx.userdata` 中的 EventLoop 内部队列。`ctx.userdata` 仅用于 Phase 1 JS Class 实例的 Rust 侧状态。

---

## 模块结构

| 文件 | 职责 |
|---|---|
| `mod.rs` | `Extension` trait、`HostBridge`、`TaskToken`、`CancelSet`、`ExtensionSet` |
| `timer.rs` | `TimerExtension`：setTimeout / setInterval / clearTimeout / clearInterval |
| `console.rs` | `ConsoleExtension`：console.log / warn / error / debug / info |
| `performance.rs` | `PerformanceExtension`：performance.now() |
| `microtask.rs` | `MicrotaskExtension`：queueMicrotask |
| `raf.rs` | `AnimationFrameExtension`：requestAnimationFrame / cancelAnimationFrame |
| `idle.rs` | `IdleCallbackExtension`：requestIdleCallback / cancelIdleCallback |
| *(Phase 1)* | `fetch.rs`、`websocket.rs`、`crypto.rs`、`text_codec.rs` 等 |

---

## Extension 三层分类

Extension 按实现路径分为三层，语义不同，不应混用：

| 层 | 类型 | 代表 API | 特征 |
|---|---|---|---|
| **Layer 1: Global** | 纯同步 JS API | console、performance | 无 EventLoop 依赖，无 keepalive |
| **Layer 2: Scheduler** | 任务投递到 EventLoop 队列 | rAF、idle | 推入 host queue，EventLoop 决定执行时机 |
| **Layer 3: Bridge** | Rust 异步 + EventLoop 通知 | setTimeout、fetch、WebSocket | tokio::spawn + keepalive + task_tx |

所有三层均通过 `HostBridge` 与 EventLoop 交互，差异仅在使用 HostBridge 的哪些字段。

---

## HostBridge（Extension 的单一依赖入口）

`HostBridge` 是 EventLoop 向 Extension 暴露的唯一接口，分为三个职责分明的 sub-struct。Extension 通过方法 API 操作队列，不直接持有 `Arc<Mutex<>>` 引用。

```rust
/// Extension 的运行时依赖入口。Clone 后可在 tokio task / JS 闭包中持有。
#[derive(Clone)]
pub struct HostBridge {
    /// Layer 2 (Scheduler)：任务投递 API
    pub scheduler: SchedulerBridge,
    /// Layer 3 (Bridge)：IO 完成事件发送
    pub io: IoBridge,
    /// Layer 3 (Bridge)：Runtime 生命周期控制
    pub runtime: RuntimeBridge,
}

/// Layer 2 专用：推入调度队列。隐藏内部 Arc<Mutex<>>，避免 Extension 直接 lock。
#[derive(Clone)]
pub struct SchedulerBridge {
    raf:  Arc<Mutex<VecDeque<RafTask>>>,
    idle: Arc<Mutex<IdleQueue>>,
}

impl SchedulerBridge {
    pub fn push_raf(&self, task: RafTask) {
        self.raf.lock().push_back(task);
    }
    pub fn push_idle(&self, task: IdleTask) {
        self.idle.lock().push(task);
    }
}

/// Layer 3 专用：发送 MacroTask 闭包到 EventLoop（bounded channel，async send）
#[derive(Clone)]
pub struct IoBridge {
    pub task_tx: mpsc::Sender<MacroTask>,
}

/// Layer 3 专用：Runtime 生命周期（keepalive acquire/drop）
#[derive(Clone)]
pub struct RuntimeBridge {
    pub keepalive: KeepAliveCount,
}
```

**各层使用的 sub-bridge：**

| Layer | scheduler | io | runtime |
|---|---|---|---|
| Layer 1 (Global) | — | — | — |
| Layer 2 (Scheduler) | ✓ | — | — |
| Layer 3 (Bridge) | 可选 | ✓ | ✓ |

**为什么不用 `ctx.userdata`：**
- `ctx.userdata` 是类型键值表，散点访问 → Extension 之间形成隐式耦合
- `HostBridge` 是显式参数 → 依赖关系可见，可单独 mock 用于测试
- `ctx.userdata` 保留给 Phase 1 JS Class 实例的 Rust 侧状态（Class 构造函数可将 `HostBridge` 存入实例字段）

**关于 Capability Token（CapabilityToken with bool fields）的决策：**
用 bool token 在运行时 check 是假安全：团队内部代码，只能在运行时 panic/error，比编译期类型错误更差。真正的 POLA 需要泛型 typestate（给 Layer 2 Extension 只传 `&SchedulerBridge`），但这打破了 `Box<dyn Extension>` 统一 trait。当前方案：类型分组 + 文档约定，Phase 0 足够。

---

## Extension trait

```rust
pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    /// 在 runtime 启动时于 context.with() 内调用一次。
    /// 只注册 native module（Module::declare_def），不写 globalThis。
    /// 不允许在此处 await；后台任务通过 tokio::spawn 启动。
    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()>;

    /// 可选 JS 胶水代码（ES module 语法）。
    /// 在所有 install() 完成后按顺序作为 module 执行，用于将 native module 导出绑定到 globalThis。
    fn js_glue(&self) -> Option<&'static str> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
```

**`install` 为 `&self`：** Extension 结构体只携带不可变配置（日志前缀、最小 delay 等）；可变运行时状态（TimerState 等）通过 `ctx.store_userdata` 注入 context，native module 函数通过 `ctx.userdata::<T>()` 读取。

---

## TaskRegistry（统一 cancel 机制）

统一替代 Phase 0 的 `TaskToken`（Layer 3）和 `CancelSet`（Layer 2）。两种取消路径通过同一个 `cancel(id)` 完成；底层语义由 `TaskEntry` variant 区分。

```rust
pub struct TaskRegistry {
    id_counter: i32,
    tasks: HashMap<i32, TaskEntry>,
}

pub enum TaskEntry {
    Bridge(oneshot::Sender<()>),  // Layer 3：drop Sender = cancel 信号
    Scheduler,                     // Layer 2：entry 存在 = active；remove = cancel
}

impl TaskRegistry {
    /// Layer 3：注册异步任务（timer / IO），返回 (id, cancel_rx)
    pub fn register_bridge(&mut self) -> (i32, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        let id = self.alloc_id();
        self.tasks.insert(id, TaskEntry::Bridge(tx));
        (id, rx)
    }

    /// Layer 2：注册调度任务（RAF / idle），返回 id
    pub fn register_scheduler(&mut self) -> i32 {
        let id = self.alloc_id();
        self.tasks.insert(id, TaskEntry::Scheduler);
        id
    }

    /// 通用 cancel（Layer 2 / Layer 3 均适用）
    /// - Bridge：remove → drop Sender → cancel_rx 关闭 → tokio select! 触发
    /// - Scheduler：remove → entry 消失 → is_active() = false → 执行时跳过
    pub fn cancel(&mut self, id: i32) {
        self.tasks.remove(&id);
    }

    /// Layer 2 执行时检查：entry 存在即 active（false = 已被 cancel）
    pub fn is_active(&self, id: i32) -> bool {
        self.tasks.contains_key(&id)
    }

    /// Layer 2 正常执行完成后清理（需配对调用，否则 entry 泄漏）
    pub fn complete(&mut self, id: i32) {
        self.tasks.remove(&id);
    }

    fn alloc_id(&mut self) -> i32 {
        let id = self.id_counter;
        self.id_counter = self.id_counter.wrapping_add(1);
        id
    }
}
```

**Layer 3（Timer/IO）使用 register_bridge：**

```rust
let (id, cancel_rx) = registry.lock().register_bridge();
tokio::spawn(async move {
    tokio::select! {
        _ = tokio::time::sleep(delay) => {
            let task: MacroTask = Box::new(move |ctx| {
                let f = persistent.restore(&ctx)?;
                let _ = f.call::<_, Value>(());
                Ok(())
                // keepalive 在此 drop
            });
            tokio::select! {
                _ = task_tx.send(task) => { registry2.lock().complete(id); }
                _ = cancel_rx => {}   // cancel 时 task（含 keepalive）drop，无泄漏
            }
        }
        _ = cancel_rx => {}
    }
});
// clearTimeout：
registry.lock().cancel(id);  // drop Sender → cancel_rx 关闭
```

**Layer 2（RAF/Idle）使用 register_scheduler：**

```rust
let id = registry.lock().register_scheduler();
scheduler.push_raf(RafTask {
    callback: Box::new(move |ctx, ts| {
        if !registry2.lock().is_active(id) { return Ok(()); }
        callback.restore(&ctx)?.call::<(f64,), ()>((ts,))?;
        registry2.lock().complete(id);
        Ok(())
    }),
});
// cancelAnimationFrame：
registry.lock().cancel(id);
```

**与 Phase 0 对比：**

| Phase 0 | Phase 1 TaskRegistry |
|---|---|
| `TaskToken` + `TimerState` HashMap | `TaskEntry::Bridge(Sender<()>)` |
| `CancelSet` HashSet | `TaskEntry::Scheduler` |
| `ts.lock().register()` | `registry.lock().register_bridge()` |
| `cs.lock().next_id()` | `registry.lock().register_scheduler()` |
| `cs.lock().consume_cancelled(id)` | `!registry.lock().is_active(id)` + `.complete(id)` |
| 两个结构各自 cancel | 统一 `registry.lock().cancel(id)` |

**ID 命名空间**：每个 Extension 持有独立 `Arc<Mutex<TaskRegistry>>`（Timer / RAF / Idle ID 互不干扰，符合 Web 规范命名空间隔离）。

---

## 安装流程

```
JsRuntime::build()
  ├── 1. 创建 QuickJS Runtime + AsyncContext
  ├── 2. 创建 EventLoop（持有 raf_queue / idle_queue 的 Arc）
  ├── 3. event_loop.host.clone() 取 HostBridge
  └── 4. context.with(|ctx| {
              // Step A：注册所有 native module（无 globalThis 写入）
              for ext in &extensions {
                  ext.install(&ctx, &host)?;
              }
              // Step B：执行 JS 胶水代码（globalThis 绑定，按 install 顺序）
              for ext in &extensions {
                  if let Some(glue) = ext.js_glue() {
                      let name = format!("<{}-glue>", ext.name());
                      Module::evaluate(ctx.clone(), name, glue)?
                          .into_future::<()>().await?;
                  }
              }
              Ok(())
          }).await?;
```

**Step A 完成后所有 native module 均可 resolve**，Step B 的 glue import 不会出现"模块未注册"错误。EventLoop 持有的 Arc 与 HostBridge 持有的 Arc 是同一组对象，无锁竞争（JS Thread 单侧执行）。

---

## 三种扩展模式

Extension 不直接写 `globalThis`。`install()` 只注册 native module；如需全局绑定，由 `js_glue()` 提供 ES module 胶水代码在 JS 侧完成。

### 模式 A：Native Module（无 globalThis，仅供 import）

适用：仅被其他模块 import 的工具库（内部 utility、不需要 Web 兼容全局）

```rust
fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
    Module::declare_def::<js_my_module::Js_my_module, _>(ctx.clone(), "@webatom/my-module")?;
    Ok(())
}
// js_glue() 默认返回 None
```

JS 端：
```js
import { foo, bar } from '@webatom/my-module';
```

---

### 模式 B：Native Module + JS Glue（globalThis 绑定由 JS 完成）

适用：console、performance、queueMicrotask（需要 Web 兼容全局变量）

```rust
fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
    Module::declare_def::<js_console::Js_console, _>(ctx.clone(), "@webatom/console")?;
    Ok(())
}

fn js_glue(&self) -> Option<&'static str> {
    Some(r#"
import * as _console from '@webatom/console';
globalThis.console = _console;
"#)
}
```

JS 端（两种均可用）：
```js
// 方式 1：module import（推荐）
import { log, warn } from '@webatom/console';

// 方式 2：globalThis（Web 兼容）
console.log('hello');
```

---

### 模式 C：Bridge Module + JS Glue（tokio::spawn + keepalive + task_tx）

适用：setTimeout、fetch、WebSocket（需要跨线程异步 + Web 兼容全局）

```rust
fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
    // host 捕获进 native module 函数闭包（通过 ctx userdata 传递）
    ctx.store_userdata(host.clone())?;
    Module::declare_def::<js_timer::Js_timer, _>(ctx.clone(), "@webatom/timer")?;
    Ok(())
}

fn js_glue(&self) -> Option<&'static str> {
    Some(r#"
import { setTimeout, clearTimeout, setInterval, clearInterval } from '@webatom/timer';
globalThis.setTimeout = setTimeout;
globalThis.clearTimeout = clearTimeout;
globalThis.setInterval = setInterval;
globalThis.clearInterval = clearInterval;
"#)
}
```

JS 端（两种均可用）：
```js
// 方式 1：module import
import { setTimeout } from '@webatom/timer';

// 方式 2：globalThis（Web 兼容）
setTimeout(() => {}, 100);
```

---

## Phase 0 Extension 列表

| Extension | 模式 | native module | js_glue globalThis 绑定 | host 字段使用 | cancel |
|---|---|---|---|---|---|
| `ConsoleExtension` | B | `@webatom/console` | `console` | 无 | — |
| `PerformanceExtension` | B | `@webatom/performance` | `performance` | 无 | — |
| `MicrotaskExtension` | B | `@webatom/queue-microtask` | `queueMicrotask` | 无（QJS job queue shim） | — |
| `AnimationFrameExtension` | C | `@webatom/animation-frame` | `requestAnimationFrame` / `cancelAnimationFrame` | `host.scheduler` | `TaskRegistry` |
| `IdleCallbackExtension` | C | `@webatom/idle-callback` | `requestIdleCallback` / `cancelIdleCallback` | `host.scheduler` | `TaskRegistry` |
| `TimerExtension` | C | `@webatom/timer` | `setTimeout` / `clearTimeout` / `setInterval` / `clearInterval` | `host.runtime.keepalive` + `host.io.task_tx` | `TaskRegistry` |

---

## ConsoleExtension

```rust
pub struct ConsoleExtension {
    pub prefix: Option<String>,
}

pub struct ConsoleConfig { pub prefix: String }

impl Extension for ConsoleExtension {
    fn name(&self) -> &'static str { "console" }

    fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        // prefix 通过 userdata 传入 native module 函数，无需闭包捕获
        ctx.store_userdata(ConsoleConfig {
            prefix: self.prefix.clone().unwrap_or_default(),
        })?;
        Module::declare_def::<js_console::Js_console, _>(ctx.clone(), "@webatom/console")?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import * as _console from '@webatom/console';
globalThis.console = _console;
"#)
    }
}
```

**native module 内部**（`#[rquickjs::module] mod js_console`）通过 `ctx.userdata::<ConsoleConfig>()` 读取 prefix，log/warn/error/debug 函数实现不变。

---

## PerformanceExtension

```rust
pub struct PerformanceState { pub startup_instant: Instant }

impl Extension for PerformanceExtension {
    fn name(&self) -> &'static str { "performance" }

    fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        ctx.store_userdata(PerformanceState { startup_instant: Instant::now() })?;
        Module::declare_def::<js_performance::Js_performance, _>(ctx.clone(), "@webatom/performance")?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import * as _performance from '@webatom/performance';
globalThis.performance = _performance;
"#)
    }
}
```

**native module 内部** `now()` 通过 `ctx.userdata::<PerformanceState>()?.startup_instant.elapsed()` 计算，`Instant` 不再需要 Copy 捕获。

---

## MicrotaskExtension

`queueMicrotask` 的正确实现：将 callback 包装为 `Promise.resolve().then(callback)`，进入 QJS job queue。这保证与 `Promise.then` 的 FIFO 顺序，符合 Web 规范。不使用 `PostMicrotaskQueue`。

```rust
impl Extension for MicrotaskExtension {
    fn name(&self) -> &'static str { "queue-microtask" }

    fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        Module::declare_def::<js_queue_microtask::Js_queue_microtask, _>(
            ctx.clone(), "@webatom/queue-microtask",
        )?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import { queueMicrotask } from '@webatom/queue-microtask';
globalThis.queueMicrotask = queueMicrotask;
"#)
    }
}
```

**native module 内部** `queueMicrotask(callback)` 实现不变：`Promise::resolve().then(callback)` 入 QJS job queue。

**queueMicrotask(A); Promise.resolve().then(B) → A 先，B 后（FIFO，注册顺序）**

---

## TimerExtension

```rust
pub struct TimerExtension {
    pub min_delay_ms: u32,
}

pub struct TimerState {
    pub min_delay_ms: u32,
    pub registry: Arc<Mutex<TaskRegistry>>,
    pub host: HostBridge,
}

impl Extension for TimerExtension {
    fn name(&self) -> &'static str { "timer" }

    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
        ctx.store_userdata(TimerState {
            min_delay_ms: self.min_delay_ms,
            registry: Arc::new(Mutex::new(TaskRegistry::default())),
            host: host.clone(),
        })?;
        Module::declare_def::<js_timer::Js_timer, _>(ctx.clone(), "@webatom/timer")?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import { setTimeout, clearTimeout, setInterval, clearInterval } from '@webatom/timer';
globalThis.setTimeout = setTimeout;
globalThis.clearTimeout = clearTimeout;
globalThis.setInterval = setInterval;
globalThis.clearInterval = clearInterval;
"#)
    }
}
```

**native module 内部** `setTimeout` / `clearTimeout` 通过 `ctx.userdata::<TimerState>()` 读取 `min_delay_ms`、`registry`、`host`；tokio::spawn 逻辑不变（见 TaskRegistry 章节）。

---

## AnimationFrameExtension

```rust
pub struct AnimationFrameState {
    pub registry: Arc<Mutex<TaskRegistry>>,
    pub scheduler: Arc<SchedulerBridge>,
}

impl Extension for AnimationFrameExtension {
    fn name(&self) -> &'static str { "animation-frame" }

    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
        ctx.store_userdata(AnimationFrameState {
            registry: Arc::new(Mutex::new(TaskRegistry::default())),
            scheduler: host.scheduler.clone(),
        })?;
        Module::declare_def::<js_animation_frame::Js_animation_frame, _>(
            ctx.clone(), "@webatom/animation-frame",
        )?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import { requestAnimationFrame, cancelAnimationFrame } from '@webatom/animation-frame';
globalThis.requestAnimationFrame = requestAnimationFrame;
globalThis.cancelAnimationFrame = cancelAnimationFrame;
"#)
    }
}
```

**native module 内部** `requestAnimationFrame` 通过 `ctx.userdata::<AnimationFrameState>()` 读取 scheduler + registry；RafTask 闭包中 `is_active` + `complete` 逻辑不变。

---

## IdleCallbackExtension

```rust
pub struct IdleCallbackState {
    pub registry: Arc<Mutex<TaskRegistry>>,
    pub scheduler: Arc<SchedulerBridge>,
}

impl Extension for IdleCallbackExtension {
    fn name(&self) -> &'static str { "idle-callback" }

    fn install(&self, ctx: &Ctx<'_>, host: &HostBridge) -> rquickjs::Result<()> {
        ctx.store_userdata(IdleCallbackState {
            registry: Arc::new(Mutex::new(TaskRegistry::default())),
            scheduler: host.scheduler.clone(),
        })?;
        Module::declare_def::<js_idle_callback::Js_idle_callback, _>(
            ctx.clone(), "@webatom/idle-callback",
        )?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(r#"
import { requestIdleCallback, cancelIdleCallback } from '@webatom/idle-callback';
globalThis.requestIdleCallback = requestIdleCallback;
globalThis.cancelIdleCallback = cancelIdleCallback;
"#)
    }
}
```

---

## Extension 与 EventLoop 的交互边界

| 操作 | Extension 使用的入口 | 归属 |
|---|---|---|
| 发送宏任务 | `host.io.task_tx.send(task)` | HostBridge |
| 阻止 Runtime 退出 | `host.runtime.keepalive.acquire()` | HostBridge |
| 推入 JS 微任务 | `Promise::resolve().then(callback)`（QJS job queue） | QJS 内部，不经 HostBridge |
| 推入 rAF 任务 | `host.scheduler.push_raf(task)` | HostBridge |
| 推入 idle 任务 | `host.scheduler.push_idle(task)` | HostBridge |
| 启动后台任务 | `tokio::spawn(async { ... })` | Tokio runtime |

**Extension 通过 `ctx.store_userdata` 注入状态，native module 函数通过 `ctx.userdata::<T>()` 读取**；但不通过 userdata 访问 EventLoop 内部队列：
- `install()` 调用 `ctx.store_userdata(ExtensionState { ... })` 将 config、registry、host 引用注入 context
- native module 只读取本 Extension 注入的类型，不访问 EventLoop 内部字段
- EventLoop 内部队列（raf_queue / idle_queue）只由 EventLoop 自身操作，Extension 只通过 `HostBridge` API push

---

## RuntimeBuilder（组装入口）

```rust
pub struct RuntimeBuilder {
    extensions: ExtensionSet,
    event_loop_config: EventLoopConfig,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self { extensions: Vec::new(), event_loop_config: EventLoopConfig::default() }
    }

    pub fn with_extension(mut self, ext: impl Extension) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub async fn build(self) -> rquickjs::Result<JsRuntime> {
        let rt = Runtime::new()?;
        let ctx = AsyncContext::full(&rt).await?;
        let event_loop = EventLoop::new(self.event_loop_config, self.render_scheduler);
        let host = event_loop.host.clone();

        ctx.with(|ctx| {
            // Step A：注册所有 native module（不写 globalThis）
            for ext in &self.extensions {
                ext.install(&ctx, &host)?;
            }
            // Step B：执行 JS 胶水代码（globalThis 绑定，按 install 顺序）
            for ext in &self.extensions {
                if let Some(glue) = ext.js_glue() {
                    let name = format!("<{}-glue>", ext.name());
                    Module::evaluate(ctx.clone(), name, glue)?
                        .into_future::<()>().await?;
                }
            }
            Ok(())
        }).await?;

        Ok(JsRuntime { rt, ctx, event_loop, shutdown: false })
    }
}

pub fn default_extensions() -> ExtensionSet {
    vec![
        Box::new(ConsoleExtension::default()),
        Box::new(PerformanceExtension),
        Box::new(MicrotaskExtension),
        Box::new(TimerExtension { min_delay_ms: 1 }),
        Box::new(AnimationFrameExtension),
        Box::new(IdleCallbackExtension),
    ]
}
```

---

## Extension ↔ EventLoop 约束不变量

约束图（ownership + directionality）：

```
EventLoop (single-threaded owner)
    │ owns (exclusive drain / execute / reorder)
    ▼
  Queues
  ├── raf_queue        (VecDeque)
  ├── idle_queue       (VecDeque)
  └── macrotask_queue  (MacroTask channel)
    ▲
    │ push only（通过 method API，不暴露 Arc<Mutex<>>）
HostBridge
  ├── scheduler.push_raf/push_idle()   ← Layer 2 唯一写入路径
  └── io.task_tx.send()               ← Layer 3 唯一写入路径
    ▲
    │ install(&ctx, &host) 捕获
Extension
```

**Inv-1 Ownership**：EventLoop 独占执行决策；Extension 只能 submit，不能 execute / reorder / pop。

```
∀ ext ∈ Extensions:
    write(ext) ⊆ HostBridge.API
    write(ext) ∩ EventLoop.internal_state = ∅
```

**Inv-2 Directionality**：单向数据流，禁止反向读取 EventLoop 内部状态。

```
Extension ──push──▶ HostBridge ──mutate──▶ Queues ──drained──▶ JS execution

禁止：
  Extension ──read──▶ EventLoop.phase / EventLoop.queues   ❌
  Extension ──execute task directly────────────────────────❌
```

**Inv-3 Single Writer**：队列的 drain / pop / execute 仅由 EventLoop JS Thread 执行。Extension 只调用 `push_*()` 方法，不调用 `lock().pop_front()` 或类似操作。

**Inv-4 Lifetime Coupling（Layer 3）**：keepalive 必须在 `tokio::spawn` 前 acquire，随 `MacroTask` 闭包一同投递或在 cancel 时 drop；不得在 EventLoop 处理完成后仍持有。

```
acquire_keepalive()
    │
    ▼
tokio::spawn(async {
    sleep / IO
    ├── complete → MacroTask { keepalive }  → task_tx.send → EventLoop execute → drop keepalive
    └── cancel  → drop keepalive（cancel_rx 触发 → task drop）
})
```

---

## 关键约束

| 约束 | 说明 |
|---|---|
| Extension 只依赖 HostBridge | install 签名显式传入 host，不通过 ctx.userdata 访问 EventLoop 内部队列 |
| ctx.userdata 仅用于 Class 实例 | Phase 0 无 userdata 注册；Phase 1 Class 实例的 Rust 状态进 userdata |
| Extension 结构体只含不可变配置 | 可变运行时状态（TimerState 等）通过 Arc<Mutex<T>> 捕获在闭包内 |
| install 不能 async | 同步 context.with() 闭包；后台任务通过 tokio::spawn |
| Layer 1 不持有 host | _host 参数忽略；无 keepalive，无 queue push |
| Layer 2 只推入 host queue | 不 tokio::spawn，不持有 keepalive；EventLoop 决定执行时机 |
| Layer 3 必须 acquire keepalive | tokio::spawn 前必须获得 keepalive；shutdown 后 acquire 返回 None → 拒绝注册 |
| TaskRegistry 统一 cancel | `cancel(id)` remove entry；Bridge: drop Sender → cancel_rx 关闭；Scheduler: is_active()=false → 执行时跳过 |
| TaskRegistry ID 命名空间 | 每个 Extension 持有独立 registry；Timer / RAF / Idle ID 互不干扰（符合 Web 规范） |
| 不持有 ctx 跨越 await | into_persistent() 保存 JS 值；dispatch 时 restore(&ctx) 恢复 |
| task_tx 嵌套 select | Layer 3 send 必须嵌套 select，防止 channel 满时 cancel 被忽略 |
| HostBridge Arc 共享 | EventLoop 直接持有 queue Arc；HostBridge 持有相同 Arc clone；无锁竞争（JS Thread 单侧） |

---

## 验证场景

```
① console.log 输出（Layer 1，host 未使用）
   console.log("hello", 42) → 日志后端输出 "hello 42"

② performance.now() 单调递增（Layer 1，startup 直接闭包捕获）
   const t0 = performance.now(); /* work */; const t1 = performance.now(); → t1 > t0

③ queueMicrotask 执行顺序（Layer 1，QJS job queue shim）
   queueMicrotask(A); Promise.resolve().then(B);
   → A 先（注册先，FIFO）→ B 后；符合 Web 规范微任务顺序

④ setTimeout 基本功能（Layer 3，host.runtime.keepalive + host.io.task_tx）
   setTimeout(fn, 100) → 100ms 后 fn 执行

⑤ clearTimeout 在 sleep 期间（TaskToken drop）
   const id = setTimeout(fn, 1000); clearTimeout(id);
   → TimerState.cancel(id) → drop TaskToken → cancel_rx 关闭 → tokio task 退出

⑥ clearTimeout 在 channel 满时（嵌套 select）
   高频 setTimeout → channel 满 → send 等待 → clearTimeout
   → 内层 select cancel_rx 臂触发 → event drop → keepalive drop

⑦ requestAnimationFrame cancel（CancelSet lazy）
   const id = requestAnimationFrame(cb); cancelAnimationFrame(id);
   → cs.cancel(id) → cb 在 RafQueue 内 → flush_raf 时 consume_cancelled(id) = true → 跳过

⑧ requestIdleCallback + timeout（Layer 2，host.idle）
   requestIdleCallback(cb, { timeout: 50 })
   → 50ms 到期 → flush_timed_out → did_timeout = true

⑨ cancelIdleCallback（CancelSet lazy）
   cancelIdleCallback(id) → 标记 cancelled → 执行时跳过

⑩ Runtime 无 keepalive 正常退出（Layer 1/2 不持有）
   仅注册 console + rAF → keepalive_count = 0 → should_exit() = true

⑪ Timer shutdown 后拒绝注册（acquire 返回 None）
   begin_shutdown() 后调用 setTimeout → host.keepalive.acquire() = None → 返回 error

⑫ Timer 未到期时 shutdown（keepalive 计数）
   setTimeout(fn, 10000) → begin_shutdown() → tokio task 持有 keepalive
   → task 正常退出或被 cancel → keepalive drop → count = 0 → 退出

⑬ HostBridge clone 独立（Layer 3 并发 timer）
   10 个 setTimeout 并发 → 各自捕获 host.clone() → 互不干扰
   → 各自 keepalive handle → 全部完成后 count = 0

⑭ Phase 1 Class 通过 userdata 访问 HostBridge
   WebSocketClass 实例构造时存 host；或 ctx.userdata::<HostBridge>()
   → 方法内可 host.io.task_tx.send(MacroTask)
```

---

## 参考资料

- [HTML Timers 规范](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html)
- [requestIdleCallback 规范](https://w3c.github.io/requestidlecallback/)
- [requestAnimationFrame 规范](https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html#animation-frames)
- [rquickjs Function::new](https://docs.rs/rquickjs/latest/rquickjs/struct.Function.html)
- event_loop/CLAUDE.md（HostBridge 对应的 EventLoop 内部队列、keepalive 模式、Timer 实现）
