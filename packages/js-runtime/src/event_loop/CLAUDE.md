# event_loop 模块

QuickJS 不内建驱动机制，宿主（webAtom）需要在 Rust 侧主动推动引擎执行。本模块实现符合
[HTML Event Loop 规范](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)
的事件循环，保证 Promise / timer / I/O 的执行顺序与浏览器一致。

---

## 模块结构

| 文件 | 职责 |
|---|---|
| `event.rs` | `RuntimeEvent` 枚举及各事件 payload 类型 |
| `event_loop_impl.rs` | `EventLoop` 结构体、主循环、`process_event()` |
| `handle.rs` | `RuntimeKeepAlive`、`KeepAliveCount` 引用计数 |
| `task.rs` | `PostMicrotaskTask`、`RafTask`、`IdleTask`、`IdleDeadline` 类型定义 |
| `idle.rs` | `IdleQueue`（push 入口）、`IdleScheduler`（优先队列调度器） |
| `render_scheduler.rs` | `RenderScheduler` trait + `HeadlessRenderScheduler` Phase 0 实现 |
| `event_loop.rs` | 入口：只做 `pub mod` 和 `pub use` |

---

## 整体架构

```
Tokio tasks (tokio::spawn)
       ↓  mpsc::Sender<RuntimeEvent>（bounded 1024）  (跨线程：Timer / Fetch / WebSocket)
       ↓  watch::Sender<Option<VsyncSignal>>           (跨线程：渲染线程写)
EventLoop
  ├── normal_rx          Timer / Fetch / WebSocket IO 完成事件
  ├── vsync_rx           watch::Receiver（只取最新一帧，不积压）
  ├── tick_counter       u64（调度层单调计数，每次 process_vsync 前自增）
  ├── render_scheduler   Box<dyn RenderScheduler>（Phase 0: HeadlessRenderScheduler）
  ├── idle_scheduler     IdleScheduler（BinaryHeap+VecDeque + insertion_counter）
  ├── idle_queue         Arc<Mutex<IdleQueue>>（HostBridge.scheduler.push_idle 写入）
  └── raf_queue          Arc<Mutex<VecDeque>>（HostBridge.scheduler.push_raf 写入）
       ↓
context.with()
  ├── dispatch()                   执行 JS 回调（宏任务）
  └── microtask_checkpoint()       execute_pending_job + post_microtask 循环（MicrotaskBudget 双限）
```

**三类数据的归属原则：**

| 类型 | 归属 | 原因 |
|---|---|---|
| `event_tx` / `vsync_tx` / `keepalive_count` | `EventLoopHandle`（跨线程） | 需要被 tokio task / 渲染线程写入 |
| `raf_queue` / `idle_queue` | EventLoop 字段（Arc<Mutex<>>，同时由 HostBridge 持有 clone） | EventLoop 直接通过 `self.*` 访问；Extension 通过 HostBridge 方法 API 写入；Phase 0 不注册为 ctx.userdata |
| `idle_scheduler` / `phase` / `render_scheduler` / `tick_counter` / `render_thread_dead` | `EventLoop` 字段（JS Thread 私有） | 无需跨线程，不做 Arc |

**IdleQueue vs IdleScheduler 职责分离：**

| 结构 | 职责 | 位置 |
|---|---|---|
| `IdleQueue` | 纯 push buffer，`VecDeque<IdleTask>` wrapper，不含排序逻辑 | context userdata（Arc<Mutex<>>） |
| `IdleScheduler` | 调度逻辑：BinaryHeap（有 timeout）+ VecDeque（无 timeout）；持有 `insertion_counter` | EventLoop 私有字段 |
| `drain_idle_queue()` | inbox drain → IdleScheduler：赋 `inserted_at` → 路由 timed/untimed | EventLoop 方法 |

`insertion_counter` 归 `IdleScheduler` 而非 `IdleQueue`：插入序号是调度层关注点（用于 BinaryHeap 同 timeout 时的 FIFO 保证），inbox 只负责接收推送，不关心排序。

---

## 执行语义模型（Spec-Level State Machine）

对应 [HTML Event Loop Processing Model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)。这是一个 loop，不是 DAG。

```
tick:
  1. pick one macrotask (Timer / IO event)
  2. run JS callback
  3. drain microtasks (until empty)       ← mandatory，在 macrotask 与下一任务之间
  4. [if vsync] flush rAF callbacks (batch) + microtask checkpoint
  5. [if idle time] flush idle callbacks (budgeted)
  6. repeat
```

**全局状态机：**

```
        IDLE
          │  event arrives (Normal channel / vsync / idle timeout)
          ▼
    MACROTASK EXECUTION
          │  dispatch JS callback (Timer / IO)
          ▼
    MICROTASK DRAIN      ← 强制，直到 QJS job queue 空
          │  (execute_pending_job loop + post_microtask_queue drain)
          ▼
    [on vsync]
    RAF PHASE            ← batch flush raf_queue → microtask checkpoint
          ▼
    IDLE PHASE           ← budgeted flush；did_timeout 强制执行
          │
          └──────────────▶ IDLE
```

**三类任务来源及执行时机：**

| 来源 | 实现 | 执行时机 | 代表 API |
|---|---|---|---|
| Macrotask | `RuntimeEvent` channel | 每轮 tick 取一个，执行后强制 drain 微任务 | `setTimeout`、`setInterval`、Fetch/WS |
| Microtask | QJS job queue（单队列） | 每个 macrotask 后立即 drain，不可跳过 | `Promise.then`、`queueMicrotask`（Promise shim，FIFO） |
| rAF | `raf_queue`（VecDeque） | vsync 信号时 batch flush，帧内一次性执行全部 | `requestAnimationFrame` |
| Idle | `IdleScheduler`（BinaryHeap + VecDeque） | Normal 空闲时按 budget 执行；timeout 到期强制 | `requestIdleCallback` |

**四条关键排序保证：**

1. **microtask 优先于下一个 macrotask**：每个 macrotask 执行后强制 drain QJS job queue，默认无 budget 限制（规范行为）
2. **Promise ≈ microtask source**：`Promise.resolve().then(fn)` 经 QJS job queue 排队，与 `queueMicrotask` 同层，在同一 drain loop 中交替执行
3. **rAF 在 microtask drain 之后**：vsync → rAF callbacks → microtask checkpoint，rAF 回调内的 Promise 在帧内、下一 macrotask 之前完成
4. **idle 在 rAF 之后**：Normal 队列空时执行；`did_timeout = true` 时 `flush_timed_out` 强制绕过 budget

---

## 调度系统排序契约（Ordering Contract）

EventLoop 内存在三个独立的调度系统，执行时机由主循环强制排序，互不干扰：

| 系统 | 来源 | 执行时机 | 代表 API |
|---|---|---|---|
| **QJS Job Queue** | QuickJS 引擎内部 | 每个 macrotask 后立即 drain（不可跳过） | `Promise.then`、`queueMicrotask`（Promise shim） |
| **SchedulerBridge** | HostBridge（Extension push） | vsync → RAF；Normal 空闲 → Idle | `requestAnimationFrame`、`requestIdleCallback` |
| **Tokio（外部）** | `tokio::spawn` | 产生 RuntimeEvent，不直接执行 JS | `setTimeout`、`fetch`、`WebSocket` |

**单次 tick 全局执行顺序：**

```
① flush_microtasks()              ← run() 入口：drain eval() 产生的 Promise jobs
   ↓
② [loop]
   drain Normal events             ← macrotask（timer / IO 完成），time budget 内批处理
     └── microtask_checkpoint()    ← 每个 macrotask 后强制 drain QJS job queue
   ↓
③ flush_timed_out idle             ← 超时 idle 强制执行（did_timeout = true）
   flush idle（Normal 空时）        ← 协作式 idle，frame budget 内
   ↓
④ [on vsync]
     └── RAF batch flush           ← SchedulerBridge.raf_queue drain
     └── microtask_checkpoint()    ← rAF 内 Promise 帧内完成
   ↓
⑤ should_exit()                   ← keepalive=0 ∧ Normal空 ∧ QJS job空
```

**三系统边界规则：**

1. **QJS Job Queue 不可被外部插入**：只有 `execute_pending_job()` 推进，tokio 和 SchedulerBridge 均无法直接写入
2. **SchedulerBridge 只 push，不 execute**：Extension 投递任务，EventLoop 在正确相位 drain
3. **tokio 通过 channel 交互**：不直接调用 JS；keepalive 保证 JS thread 不提前退出

**数据流方向（单向，无反馈环）：**

```
tokio task ──RuntimeEvent──▶ mpsc::channel ──recv──▶ EventLoop（macrotask 相位）
Extension  ──push_raf()───▶ raf_queue      ──drain──▶ EventLoop（vsync 相位）
Promise    ──.then()──────▶ QJS job queue  ──exec──▶  EventLoop（microtask 相位）
```

每条路径只有一个写入方，EventLoop 是唯一的 drain / execute 决策者。`Ordering Contract` 的本质：**外部只能 push，不能 pull，不能跨相位执行**。

---

## EventLoopPhase

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventLoopPhase {
    Idle               = 0,
    Task               = 1,
    Microtask          = 2,
    Raf                = 3,
    ResizeObserver     = 4,
    IntersectionObserver = 5,
}
```

存储于 `EventLoop.phase: Cell<EventLoopPhase>`（JS Thread 单线程访问，无需 Atomic）。
`Cell<T>` 非 Sync，不放入 Handle。

---

## 事件优先级

| 优先级 | 类型 | 机制 |
|---|---|---|
| 1 Normal | Timer / Fetch / WebSocket | `mpsc::Receiver<RuntimeEvent>`（bounded 1024） |
| 2 Vsync  | 渲染帧同步信号 | `watch::Receiver<Option<VsyncSignal>>` |
| 3 Idle   | requestIdleCallback | `IdleScheduler`（有 timeout: BinaryHeap，无 timeout: VecDeque FIFO） |

```rust
const TASK_BUDGET_MS: f64 = 8.0;
const FRAME_BUDGET_MS: f64 = 16.6;
const IDLE_THRESHOLD_MS: f64 = 1.0;
const EVENT_CHANNEL_CAPACITY: usize = 1024;
```

---

## VsyncSignal 与 tick_id 分离

```rust
pub struct VsyncSignal {
    pub frame_id: u64,       // 渲染层概念：渲染线程分配，用于 LayoutRequest/Response 关联（Phase 1）
    pub timestamp_ms: f64,   // 传给 rAF 回调的时间戳
    // 不携带 resize/intersection entries：rAF 可能修改 DOM，layout 必须在 rAF 后重算
}
```

**tick_id（调度层）vs frame_id（渲染层）分离原则：**

| 概念 | 来源 | 分配者 | 用途 |
|---|---|---|---|
| `tick_id` | `EventLoop.tick_counter` | EventLoop 自增 | 调度标识，传给 `RenderScheduler` trait，headless/render 通用 |
| `frame_id` | `VsyncSignal.frame_id` | 渲染线程分配 | LayoutRequest/Response 关联，Phase 1 渲染层内部专用 |

**隔离原则：** `RenderScheduler` trait 只接收 `tick_id + timestamp_ms`（调度层语义）。`frame_id` 是渲染层内部概念，通过 `on_vsync()` 钩子隔离在 `BlitzRenderScheduler` 实现内部，不泄漏到 trait 接口。

### 规范帧内时序

```
①  rAF callbacks → microtask checkpoint
②  [Layout — 渲染层，LayoutRequest { frame_id } / LayoutResult { frame_id }]
③  ResizeObserver → microtask checkpoint
④  [Paint — 渲染层]
⑤  IntersectionObserver → microtask checkpoint
```

### 两阶段 Vsync 协议（Phase 1，BlitzRenderScheduler 内部）

```
渲染线程                                JS Thread
vsync_tx.send(VsyncSignal { frame_id, ts })
                    on_vsync(&signal) → self.current_frame_id = frame_id（渲染层内部）
                    tick_counter += 1 → tick_id（调度层）
                    update_rendering(tick_id, ts):
                                        ① rAF → microtask
                                        ② layout_request_tx.send(LayoutRequest { frame_id })
layout() → layout_result_tx.send(LayoutResult { frame_id })
                                        ③ recv LayoutResult（验证 frame_id 匹配）
                                        ④ ResizeObserver → microtask
                                        ⑤ paint_request_tx.send()
paint() → intersection_result_tx.send()
                                        ⑥ IntersectionObserver → microtask
```

---

## RenderScheduler（提前落位，调度层解耦）

```rust
pub trait RenderScheduler {
    /// vsync 信号到达时调用，默认 no-op。
    /// BlitzRenderScheduler 覆盖此方法捕获 frame_id（渲染层内部概念，不泄漏到 update_rendering 签名）。
    fn on_vsync(&mut self, _signal: &VsyncSignal) {}

    /// tick_id：调度层单调计数，每次 vsync/tick 前由 EventLoop 自增传入。
    /// timestamp_ms：传给 rAF 回调的时间戳（DOMHighResTimeStamp）。
    /// 不接收 VsyncSignal：frame_id 是渲染层关注点，由 on_vsync 单独捕获。
    /// post_microtask_queue 已从签名移除：queueMicrotask 通过 QJS job queue 实现，
    /// microtask_checkpoint 只需 ctx。Phase 1 Observer 通知由 BlitzRenderScheduler 内部直接调用。
    fn update_rendering(
        &mut self,
        ctx: &Ctx<'_>,
        raf_queue: &RafQueue,
        tick_id: u64,
        timestamp_ms: f64,
    ) -> rquickjs::Result<()>;
}

/// Phase 0：headless stub，仅执行 rAF，无 layout / paint / Observer
pub struct HeadlessRenderScheduler;

impl RenderScheduler for HeadlessRenderScheduler {
    // on_vsync：默认 no-op（headless 无渲染层，frame_id 无意义）

    fn update_rendering(
        &mut self,
        ctx: &Ctx<'_>,
        raf_queue: &RafQueue,
        _tick_id: u64,
        timestamp_ms: f64,
    ) -> rquickjs::Result<()> {
        let tasks: Vec<_> = raf_queue.lock().drain(..).collect();
        for task in tasks { task.call(ctx, timestamp_ms)?; }
        microtask_checkpoint(ctx, MicrotaskBudget::unlimited())
    }
}

/// Phase 1：BlitzRenderScheduler（接入 blitz 后实现）
/// on_vsync() 捕获 frame_id → 存入 self.current_frame_id
/// update_rendering() 使用 self.current_frame_id 发 LayoutRequest，完整两阶段协议
```

**EventLoop 调用顺序（process_vsync）：**
1. `render_scheduler.on_vsync(&signal)` — 通知渲染层捕获 frame_id
2. `tick_counter += 1` — 推进调度层 tick
3. `render_scheduler.update_rendering(tick_counter, signal.timestamp_ms)` — 执行帧内 JS 工作

Phase 1 替换 `BlitzRenderScheduler` 实现，EventLoop 调用序列不变。

---

## 主循环结构

```rust
pub async fn run(&mut self, context: &AsyncContext) -> rquickjs::Result<()> {
    self.flush_microtasks(context).await?;

    loop {
        // ① drain IdleQueue inbox → IdleScheduler（赋 inserted_at，路由 timed/untimed）
        self.drain_idle_queue();

        // ② 时间预算批处理 Normal 事件
        let batch_start = Instant::now();
        loop {
            if batch_start.elapsed().as_secs_f64() * 1000.0 >= TASK_BUDGET_MS { break; }
            match self.normal_rx.try_recv() {
                Ok(event) => self.process_event(context, event).await?,
                Err(_) => break,
            }
        }

        // ③ Idle 提前检查（Normal 空时立即处理）
        if self.normal_rx.is_empty() {
            self.idle_scheduler.flush_timed_out(context).await?;
            if self.normal_rx.is_empty() && !self.idle_scheduler.is_empty() {
                self.idle_scheduler.flush(context, FRAME_BUDGET_MS).await?;
            }
        }

        // ④ 检查 vsync
        match self.vsync_rx.has_changed() {
            Ok(true) => {
                let signal = self.vsync_rx.borrow_and_update().clone();
                if let Some(signal) = signal {
                    self.process_vsync(context, signal).await?;
                }
            }
            Ok(false) => {}
            Err(_) => { self.render_thread_dead = true; }
        }

        // ⑤ 检查退出
        if self.should_exit(context).await? {
            self.idle_scheduler.flush(context, f64::MAX).await?;
            break;
        }

        // ⑥ 阻塞等待
        self.phase.set(EventLoopPhase::Idle);
        let next_idle_deadline = self.idle_scheduler.next_timeout();

        if !self.render_thread_dead {
            tokio::select! {
                biased;
                Some(event) = self.normal_rx.recv() => {
                    self.process_event(context, event).await?;
                }
                Ok(()) = self.vsync_rx.changed() => {}
                _ = sleep_until_opt(next_idle_deadline) => {}
                _ = self.handle.keepalive_count.wait_idle() => {}
            }
        } else {
            tokio::select! {
                biased;
                Some(event) = self.normal_rx.recv() => {
                    self.process_event(context, event).await?;
                }
                _ = sleep_until_opt(next_idle_deadline) => {}
                _ = self.handle.keepalive_count.wait_idle() => {}
            }
        }
    }
    Ok(())
}

async fn process_vsync(&mut self, context: &AsyncContext, signal: VsyncSignal) -> rquickjs::Result<()> {
    // Step 1: 通知渲染层（BlitzRenderScheduler 存 frame_id，Headless no-op）
    self.render_scheduler.on_vsync(&signal);
    // Step 2: 推进调度层 tick（与渲染层 frame_id 解耦）
    self.tick_counter += 1;
    let tick_id = self.tick_counter;
    let timestamp_ms = signal.timestamp_ms;
    // Step 3: 执行帧内 JS 工作
    // EventLoop 直接持有 raf_queue 字段，无需通过 ctx.userdata 获取
    let raf = Arc::clone(&self.raf_queue);
    context.with(|ctx| {
        self.render_scheduler.update_rendering(&ctx, &*raf.lock(), tick_id, timestamp_ms)
    }).await
}

async fn sleep_until_opt(deadline: Option<Instant>) {
    match deadline {
        Some(t) => tokio::time::sleep_until(t.into()).await,
        None => futures::future::pending().await,
    }
}
```

---

## RuntimeEvent（仅 IO 完成通知）

```rust
pub enum RuntimeEvent {
    Timer(TimerEvent),
    // 未来：Fetch(FetchEvent), WebSocket(WsEvent)
    // ★ requestIdleCallback 不走此 channel：Extension 直接推入 IdleQueue userdata
}
```

---

## context userdata 注册

**Phase 0：不注册任何 userdata。**

- EventLoop 通过 `self.raf_queue` / `self.idle_queue` 直接访问队列字段
- Extension 通过 `HostBridge`（显式参数）访问队列，不经 `ctx.userdata`
- `EventLoopHandle` 不需要注册为 userdata（Extension 通过 `HostBridge.runtime.keepalive` 和 `HostBridge.io.event_tx` 访问）

**Phase 1（按需启用）：**

```rust
// Class 方法内无法访问外部捕获的 HostBridge 时，注册为 userdata
ctx.store_userdata(host.clone())?;  // HostBridge

// Worker Realm 出现后，可能替换为 RealmHost（TBD）
```

旧式 `ctx.userdata::<PostMicrotaskQueue>()` / `ctx.userdata::<RafQueue>()` 等访问模式已废弃，Extension 均通过 HostBridge 方法 API 操作队列。

---

## IdleTask / IdleDeadline

```rust
pub struct IdleTask {
    pub callback: Box<dyn for<'js> FnOnce(Ctx<'js>, IdleDeadline) -> rquickjs::Result<()> + Send + 'static>,
    pub timeout_at: Option<Instant>,
    pub inserted_at: u64,  // 由 IdleScheduler 在 drain 时赋值，不由 IdleQueue 负责
}

impl IdleTask {
    pub fn is_timed_out(&self) -> bool {
        self.timeout_at.map_or(false, |t| Instant::now() >= t)
    }
}

pub struct IdleDeadline {
    pub time_remaining_ms: f64,
    pub did_timeout: bool,
}
```

---

## IdleQueue（纯 push buffer，context userdata）

`IdleQueue` 只负责接收推送，不含排序或计数逻辑：

```rust
pub struct IdleQueue {
    pending: VecDeque<IdleTask>,
    // 无 insertion_counter：插入序号是调度关注点，归 IdleScheduler 持有
}

impl IdleQueue {
    pub fn push(&mut self, task: IdleTask) {
        // inserted_at 占位值，drain_idle_queue() 中由 IdleScheduler.insertion_counter 覆盖
        self.pending.push_back(task);
    }
}
```

---

## IdleScheduler（优先队列调度器，EventLoop 私有）

`IdleScheduler` 持有 `insertion_counter`，在 drain 时赋值并路由：

```rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;

struct TimedEntry {
    timeout_at: Instant,
    inserted_at: u64,
    task: IdleTask,
}
impl PartialEq for TimedEntry { fn eq(&self, o: &Self) -> bool { self.cmp(o).is_eq() } }
impl Eq for TimedEntry {}
impl PartialOrd for TimedEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for TimedEntry {
    fn cmp(&self, o: &Self) -> Ordering {
        // BinaryHeap 是 max-heap，Reverse 语义使其变为 min-heap
        o.timeout_at.cmp(&self.timeout_at)
            .then(o.inserted_at.cmp(&self.inserted_at))
    }
}

pub struct IdleScheduler {
    timed: BinaryHeap<TimedEntry>,    // 有 timeout，O(log N) push/pop，min-heap by timeout_at
    untimed: VecDeque<IdleTask>,      // 无 timeout，O(1) push/pop，FIFO
    insertion_counter: u64,           // 归属于调度层，drain 时赋给 task.inserted_at
}

impl IdleScheduler {
    pub fn push(&mut self, mut task: IdleTask) {
        // 在此赋 inserted_at，而非在 IdleQueue.push() 中
        task.inserted_at = self.insertion_counter;
        self.insertion_counter += 1;
        match task.timeout_at {
            Some(t) => self.timed.push(TimedEntry { timeout_at: t, inserted_at: task.inserted_at, task }),
            None    => self.untimed.push_back(task),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.timed.is_empty() && self.untimed.is_empty()
    }

    /// 最早的 timeout 截止时刻（用于 select! sleep_until_opt，O(1)）
    pub fn next_timeout(&self) -> Option<Instant> {
        self.timed.peek().map(|e| e.timeout_at)
    }

    /// 强制执行所有已超时任务（did_timeout = true，不受 budget 限制，O(k log N)）
    pub async fn flush_timed_out(&mut self, context: &AsyncContext) -> rquickjs::Result<()> {
        let mut timed_out = Vec::new();
        while let Some(entry) = self.timed.peek() {
            if entry.task.is_timed_out() {
                timed_out.push(self.timed.pop().unwrap().task);
            } else {
                break; // heap 顶未超时，后面的也不会超时
            }
        }
        for task in timed_out {
            context.with(|ctx| (task.callback)(ctx, IdleDeadline {
                time_remaining_ms: 0.0,
                did_timeout: true,
            })).await?;
        }
        Ok(())
    }

    /// 在 budget 内按优先级执行普通任务（先检查 budget 再 pop）
    /// 顺序：有 timeout 的优先 → 无 timeout FIFO
    pub async fn flush(&mut self, context: &AsyncContext, budget_ms: f64) -> rquickjs::Result<()> {
        let start = Instant::now();
        loop {
            let remaining_ms = budget_ms - start.elapsed().as_secs_f64() * 1000.0;
            if remaining_ms <= 0.0 { break; }

            let task = match (self.timed.peek(), self.untimed.front()) {
                (Some(_), _)    => self.timed.pop().unwrap().task,
                (None, Some(_)) => self.untimed.pop_front().unwrap(),
                (None, None)    => break,
            };

            context.with(|ctx| (task.callback)(ctx, IdleDeadline {
                time_remaining_ms: remaining_ms,
                did_timeout: false,
            })).await?;
        }
        Ok(())
    }
}
```

**EventLoop.drain_idle_queue()：**

```rust
impl EventLoop {
    fn drain_idle_queue(&mut self) {
        // IdleQueue inbox → IdleScheduler：赋 inserted_at，路由 timed/untimed，O(k log N)
        let tasks: Vec<_> = self.idle_queue.lock().pending.drain(..).collect();
        for task in tasks {
            self.idle_scheduler.push(task);  // push() 内赋 inserted_at 并路由
        }
    }
}
```

---

## microtask checkpoint（MicrotaskBudget 双限保护）

```rust
/// 双 budget：时间限制防慢微任务（一个昂贵回调就够卡）；迭代限制防无限微任务（queueMicrotask(self)）。
/// 两个条件独立触发截断，任一满足即 warn + break。
pub struct MicrotaskBudget {
    pub max_iterations: Option<u32>,  // None = 不限迭代次数
    pub max_time_ms: Option<f64>,     // None = 不限执行时间
}

impl MicrotaskBudget {
    pub fn unlimited() -> Self {
        Self { max_iterations: None, max_time_ms: None }
    }
}

/// queueMicrotask 通过 Promise shim 进入 QJS job queue，与 Promise.then FIFO 共享队列。
/// microtask_checkpoint 只需 drain QJS job queue；无 PostMicrotaskQueue。
fn microtask_checkpoint(
    ctx: &Ctx,
    budget: MicrotaskBudget,
) -> rquickjs::Result<bool> {   // bool: true = 被 budget 截断（仍有未完成微任务）
    let start = Instant::now();
    let mut count = 0u32;
    loop {
        if !ctx.runtime().execute_pending_job()? { return Ok(false); }
        count += 1;

        // 迭代 budget：防无限微任务（Promise.resolve().then(() => Promise.resolve().then(...))）
        if let Some(max_iter) = budget.max_iterations {
            if count >= max_iter {
                log::warn!(
                    "microtask_checkpoint: exceeded {} iterations (starvation guard). \
                     Remaining microtasks will run in next event loop iteration.",
                    max_iter
                );
                return Ok(true);
            }
        }

        // 时间 budget：防慢微任务（一个昂贵回调即可触发）
        if let Some(max_ms) = budget.max_time_ms {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            if elapsed_ms >= max_ms {
                log::warn!(
                    "microtask_checkpoint: exceeded {:.1}ms time budget after {} iterations. \
                     Remaining microtasks will run in next event loop iteration.",
                    max_ms, count
                );
                return Ok(true);
            }
        }
    }
}
```

**双 budget 的必要性：**

| 场景 | 触发条件 | 单 budget 的盲区 |
|---|---|---|
| `Promise.resolve().then(() => Promise.resolve().then(...))` 无限循环 | 迭代次数超限 | 只有时间限制时：每次执行很快，时间不一定超 |
| 单个昂贵微任务（大数组 sort 等） | 时间超限 | 只有迭代限制时：一次迭代就能卡满帧 |

**截断后行为（与之前一致）：**
- None + None（默认）= 完全符合规范，可能卡死
- 截断后微任务在下一轮 process_event 前重新执行，Timer / Vsync 得以响应
- 若再次触发 budget，再次截断，形成"让步式无限微任务"而非完全卡死

---

## `process_event`

```rust
struct ProcessEventGuard<'a>(&'a Cell<bool>);
impl Drop for ProcessEventGuard<'_> {
    fn drop(&mut self) { self.0.set(false); }
}

async fn process_event(
    &mut self,
    context: &AsyncContext,
    event: RuntimeEvent,
) -> rquickjs::Result<()> {
    // reentrancy guard：结构上不可能发生（main loop await process_event 期间无法再次调用），
    // assert! 保留作为不变量文档，debug 和 release 均有效
    assert!(!self.in_process_event.get(), "process_event: unexpected reentrancy");
    self.in_process_event.set(true);
    let _guard = ProcessEventGuard(&self.in_process_event);
    self.phase.set(EventLoopPhase::Task);
    let handle = self.handle.clone();
    let budget = self.config.microtask_budget;

    context.with(move |ctx| {
        dispatch(&ctx, &handle, event)?;
        microtask_checkpoint(&ctx, budget)?;
        Ok(())
    }).await
}
```

---

## 退出条件

```rust
async fn should_exit(&self, context: &AsyncContext) -> rquickjs::Result<bool> {
    if self.handle.keepalive_count.count() > 0 { return Ok(false); }
    if !self.normal_rx.is_empty()               { return Ok(false); }

    let has_jobs = context.with(|ctx| {
        Ok::<bool, rquickjs::Error>(ctx.runtime().is_job_pending())
    }).await?;
    if has_jobs { return Ok(false); }

    self.handle.keepalive_count.begin_shutdown();
    Ok(true)
}
```

### raf_queue 与退出（headless vs render mode）

| 模式 | rAF 行为 | 退出判断 |
|---|---|---|
| **headless**（当前） | rAF 永不执行 | raf_queue 不参与退出 |
| **render mode**（Phase 1） | rAF 正常执行 | `raf_queue` 非空时阻止退出 |

---

## RuntimeKeepAlive — packed atomic

```rust
const SHUTDOWN_BIT: usize = 1 << (usize::BITS - 1);
const COUNT_MASK: usize = !SHUTDOWN_BIT;

impl KeepAliveCount {
    pub fn acquire(&self) -> Option<RuntimeKeepAlive> {
        let prev = self.packed.fetch_add(1, Ordering::AcqRel);
        if prev & SHUTDOWN_BIT != 0 {
            self.packed.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(RuntimeKeepAlive { packed: Arc::clone(&self.packed), idle_notify: Arc::clone(&self.idle_notify) })
    }

    pub fn begin_shutdown(&self) {
        self.packed.fetch_or(SHUTDOWN_BIT, Ordering::AcqRel);
        self.idle_notify.notify_waiters();
    }

    pub fn count(&self) -> usize { self.packed.load(Ordering::Acquire) & COUNT_MASK }

    pub async fn wait_idle(&self) {
        let notified = self.idle_notify.notified();
        if self.count() == 0 { return; }
        notified.await;
    }
}

impl Drop for RuntimeKeepAlive {
    fn drop(&mut self) {
        let prev = self.packed.fetch_sub(1, Ordering::AcqRel);
        if (prev & COUNT_MASK) == 1 { self.idle_notify.notify_waiters(); }
    }
}
```

| 场景 | 持有者 | 释放时机 |
|---|---|---|
| `setTimeout` 正常触发 | `Timeout { keepalive }` event 本身 | event drop 时 |
| `setTimeout` 被 clear | tokio cancel 分支（嵌套 select 处理） | task 结束 |
| `setInterval` 运行中 | tokio loop | clearInterval → cancel → loop break |
| `fetch` / `WebSocket` | tokio task | 请求结束 / 连接关闭 |
| `rAF` | **不持有** | headless 下不阻止退出 |
| Idle callback | **不持有** | 退出前 flush |

---

## EventLoopHandle（跨线程接口）

```rust
pub struct EventLoopHandle {
    pub keepalive_count: KeepAliveCount,
    pub event_tx: mpsc::Sender<RuntimeEvent>,          // bounded 1024
    pub vsync_tx: watch::Sender<Option<VsyncSignal>>,
}
```

---

## Timer 实现（bounded channel + 嵌套 select retry）

### setTimeout

```rust
tokio::spawn(async move {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(delay)) => {
            let event = RuntimeEvent::Timer(TimerEvent {
                kind: TimerEventKind::Timeout { func, keepalive }
            });
            // 嵌套 select：send 等待空间 OR cancel 响应
            // cancel 时 event drop → keepalive drop → count-- 无泄漏
            tokio::select! {
                _ = event_tx.send(event) => {}   // channel 满时等待，keepalive 阻止 Runtime 退出
                _ = cancel_rx => {}              // clearTimeout 在等待发送期间触发
            }
        }
        _ = cancel_rx => {}  // clearTimeout 在 sleep 期间触发
    }
});
```

**为什么需要嵌套 select：**
外层 select 处理"sleep 期间 cancel"。若 sleep 触发后 channel 满，内层 select 处理"等待发送期间 cancel"。没有嵌套 select 时，cancel 会被忽略直到 send 完成。

### setInterval

```rust
tokio::spawn(async move {
    let _keepalive = keepalive;
    let mut next_fire = Instant::now() + Duration::from_millis(delay);
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(next_fire.into()) => {
                let event = RuntimeEvent::Timer(TimerEvent { kind: TimerEventKind::Interval { id } });
                tokio::select! {
                    _ = event_tx.send(event) => {}
                    _ = cancel_rx => { break; }  // cancel 在等待发送期间触发
                }
                next_fire = Instant::now() + Duration::from_millis(delay);
            }
            _ = &mut cancel_rx => { break; }
        }
    }
});
```

---

## EventLoop 字段

```rust
struct EventLoop {
    normal_rx: mpsc::Receiver<RuntimeEvent>,
    vsync_rx: watch::Receiver<Option<VsyncSignal>>,
    handle: EventLoopHandle,
    config: EventLoopConfig,

    // JS Thread 私有
    phase: Cell<EventLoopPhase>,
    in_process_event: Cell<bool>,
    render_thread_dead: bool,
    tick_counter: u64,                           // 调度层计数，process_vsync 前自增
    idle_scheduler: IdleScheduler,               // BinaryHeap(timed) + VecDeque(untimed) + insertion_counter
    render_scheduler: Box<dyn RenderScheduler>,

    // 与 HostBridge 共享的 Arc（零竞争：JS Thread 单侧 drain，Extension 通过 HostBridge push）
    idle_queue: Arc<Mutex<IdleQueue>>,   // 纯 push buffer
    raf_queue: Arc<Mutex<VecDeque<RafTask>>>,
}

pub struct EventLoopConfig {
    /// 微任务双 budget：两个条件独立，任一触发则 warn + 截断（下轮继续）。
    /// 默认均为 None（规范行为）。
    pub microtask_budget: MicrotaskBudget,
}

pub struct MicrotaskBudget {
    pub max_iterations: Option<u32>,  // None = 规范行为（可无限迭代）
    pub max_time_ms: Option<f64>,     // None = 规范行为（可无限时间）
    // 建议生产值：max_iterations = Some(100_000)，max_time_ms = Some(4.0)（半帧）
}
```

---

## 长期架构：RenderScheduler Phase 1

```
Phase 0: HeadlessRenderScheduler
  └── update_rendering(ctx, raf_queue, tick_id, ts): flush_raf → microtask_checkpoint(ctx)

Phase 1: BlitzRenderScheduler
  ├── on_vsync(&signal): self.current_frame_id = signal.frame_id（渲染层内部）
  └── update_rendering(ctx, raf_queue, tick_id, ts):
        ├── flush_raf → microtask_checkpoint(ctx)
        ├── layout_request_tx.send(LayoutRequest { frame_id: self.current_frame_id })
        ├── await layout_result_rx → verify frame_id
        ├── notify_resize_observers() → microtask_checkpoint(ctx)
        │   （ResizeObserver 回调直接调用，非 PostMicrotaskQueue）
        ├── paint_request_tx.send()
        └── notify_intersection_observers() → microtask_checkpoint(ctx)
```

Phase 1 同步引入 headless / render mode 区分（render mode 下 raf_queue 非空时阻止退出）。

---

## 线程模型

```
JS Thread (tokio current_thread)
  └── EventLoop
        ├── phase / in_process_event / tick_counter / render_thread_dead （无锁，单线程）
        ├── idle_scheduler（BinaryHeap + VecDeque + insertion_counter，无锁，单线程）
        ├── render_scheduler（Box<dyn>，on_vsync + update_rendering，单线程）
        ├── normal_rx + vsync_rx
        └── context.with()
              ├── dispatch()
              └── execute_pending_job() loop（MicrotaskBudget 双限：iter + time）
                  （queueMicrotask 通过 Promise shim 进入 QJS job queue，与 Promise.then FIFO 共享）

tokio Thread Pool
  └── event_tx.send(event).await（bounded 1024，back-pressure）
      嵌套 select：send + cancel_rx 竞争，keepalive 保证 Runtime 不提前退出

渲染线程
  └── vsync_tx.send(VsyncSignal { frame_id, ts })
      → on_vsync() 隔离 frame_id 于 BlitzRenderScheduler 内部
      → tick_counter 自增，update_rendering(tick_id, ts) 执行帧内 JS 工作
```

---

## 关键约束

| 约束 | 说明 |
|---|---|
| JS 单线程 | 所有 JS 执行必须在 `context.with()` 内 |
| Handle 只含跨线程资源 | `raf_queue` / `idle_queue` 不在 Handle 上 |
| phase 不出 EventLoop | `Cell<T>` 非 Sync，不放入 Handle |
| packed atomic | 单个 `AtomicUsize`（高位=shutdown, 低位=count），消灭 TOCTOU |
| acquire() 回滚 | fetch_add 后检高位，已 shutdown 则 fetch_sub 回滚返回 None |
| begin_shutdown() | should_exit() 确认退出时调用，之后所有 acquire() 返回 None |
| bounded channel 1024 | 防 OOM；async send 提供 back-pressure |
| 嵌套 select send+cancel | 解决 channel 满时 cancel 无法响应的问题 |
| tick_id vs frame_id 分离 | tick_id 是调度层计数（EventLoop 自增）；frame_id 是渲染层概念（VsyncSignal，隔离在 BlitzRenderScheduler 内） |
| on_vsync() 隔离渲染层 | RenderScheduler trait 签名只含 tick_id + timestamp_ms；frame_id 不泄漏到 trait 接口 |
| IdleQueue 纯 buffer | 无 insertion_counter；push() 不赋序号，排序关注点完全在 IdleScheduler |
| insertion_counter 归 IdleScheduler | drain_idle_queue() 赋 inserted_at 后路由，确保序号连续且无语义混淆 |
| IdleScheduler O(log N) | 有 timeout → BinaryHeap min-heap；无 timeout → VecDeque FIFO；无全量排序 |
| flush_timed_out 早终止 | heap 顶未超时即 break，O(k log N)，k 为超时任务数 |
| MicrotaskBudget 双限 | max_iterations 防无限循环微任务；max_time_ms 防单个慢微任务；独立触发 |
| 双 budget 截断语义 | warn + break，下轮继续；形成"让步式无限微任务"而非卡死 |
| assert! 替代 debug_assert! | reentrancy 结构上不可能，assert! 作为不变量文档，debug/release 均有效 |
| RuntimeEvent 仅含 IO 完成 | idle 注册通过 HostBridge.scheduler.push_idle 直接推 IdleQueue，不走 RuntimeEvent |
| IdleTask.inserted_at 由调度器赋值 | Extension push 时为占位值，drain 时由 IdleScheduler.insertion_counter 覆盖 |
| Vsync 只取最新 | watch + borrow_and_update()，积压帧丢弃 |
| VsyncSignal 不携带 entries | rAF 可能修改 DOM，layout 必须在 rAF 后 |
| RenderScheduler 提前落位 | Phase 0 stub，Phase 1 替换，EventLoop 调用序列不变 |
| Mutex 零竞争 | 队列字段全程 JS Thread 访问，Arc<Mutex<>> 仅为 Send 约束（HostBridge clone 需要 Sync） |
| Idle 提前检查 | Normal 空时立即执行，避免多等一轮 |
| process_event 不可重入 | assert! + RAII Guard；Drop 保证 release 路径也重置标志 |
| 不使用 ctx.spawn | IO 全用 tokio::spawn，rquickjs 无内部 Future |

---

## 验证场景

```
① Promise 顺序
   Promise.resolve().then(A).then(B) → A、B 在宏任务后、下一 setTimeout 前执行

② MutationObserver
   dom.mutate() → setTimeout(fn, 0) → MutationObserver 先执行

③ setTimeout 嵌套
   setTimeout(() => setTimeout(fn, 0), 0) → 两轮 tick 依次执行

④ rAF + microtask
   requestAnimationFrame(() => Promise.resolve().then(fn)) → fn 在 rAF 后、下一帧前

⑤ requestIdleCallback 无 timeout
   → Normal 空时主循环顶部执行；did_timeout = false

⑥ requestIdleCallback 有 timeout
   requestIdleCallback(fn, { timeout: 100 })
   → 100ms 到期：sleep_until 臂触发 flush_timed_out → did_timeout = true

⑦ Idle 优先级排序（O(log N)）
   requestIdleCallback(a, { timeout: 5000 })
   requestIdleCallback(b, { timeout: 100 })
   → b 在 BinaryHeap 顶，flush 时先于 a 执行，无全量排序
   → inserted_at 在 drain_idle_queue() 中赋值，FIFO 保证同 timeout_at 时按注册顺序

⑧ Bounded channel back-pressure
   高频 setTimeout → channel 满 → 嵌套 select send 等待
   → keepalive 持有，EventLoop 持续消费 → 最终发送成功，无 OOM

⑨ clearTimeout 在 send 等待期间触发
   setTimeout(fn, 0) → channel 满 → send 等待 → clearTimeout()
   → 嵌套 select cancel_rx 臂触发 → event drop → keepalive drop → 正确退出

⑩ acquire() 在 shutdown 后返回 None
   should_exit() → begin_shutdown() → 之后 acquire() 立即返回 None

⑪ tick_id / frame_id 解耦验证
   HeadlessRenderScheduler: on_vsync() 无感知，update_rendering(_tick_id, ts) 只用 ts
   BlitzRenderScheduler: on_vsync() 存 frame_id，update_rendering 用 self.current_frame_id 发 LayoutRequest
   → EventLoop 调用序列相同，frame_id 不出现在 trait 接口

⑫ 跳帧检测（BlitzRenderScheduler 内部）
   渲染线程发 frame 3、4、5；watch 跳过 3、4，EventLoop 处理 5
   → on_vsync({ frame_id: 5 }) → BlitzRenderScheduler 检测跳帧（frame_id 不连续）
   → LayoutRequest { frame_id: 5 } 与 LayoutResult 匹配

⑬ MicrotaskBudget 迭代截断
   Promise.resolve().then(function loop() { Promise.resolve().then(loop); }) → 超 max_iterations → warn + break
   → 下轮 EventLoop 迭代继续未完成微任务；Timer / Vsync 在截断间隙得以响应

⑭ MicrotaskBudget 时间截断
   microtask 中执行大数组 sort（单次耗时 10ms） → max_time_ms = 4.0 触发
   → 1 次迭代后截断（迭代 budget 未到）；下轮继续
```

---

## 参考资料

- [HTML Event Loop 规范](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)
- [HTML Update the Rendering 步骤](https://html.spec.whatwg.org/multipage/webappapis.html#update-the-rendering)
- [requestIdleCallback 规范](https://w3c.github.io/requestidlecallback/)
- [LLRT 事件循环实现](https://github.com/awslabs/llrt)
- [tokio::select! biased](https://docs.rs/tokio/latest/tokio/macro.select.html#fairness)
- [tokio::sync::watch](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)
