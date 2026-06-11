# event_loop 模块

QuickJS 不内建驱动机制，宿主（webAtom）需要在 Rust 侧主动推动引擎执行。本模块实现符合
[HTML Event Loop 规范](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)
的事件循环，保证 Promise / timer / I/O 的执行顺序与浏览器一致。

---

## 模块结构

| 文件 | 职责 |
|---|---|
| `timer.rs` | `TimerState`、`SharedTimers`、最小堆、`PendingTimer` |
| `task.rs` | `MacroTask` 枚举（当前只有 `Timer` 变体，后续扩展 I/O 回调） |
| `run.rs` | `EventLoop` 结构体、`tick` / `run` 驱动逻辑 |
| `event_loop.rs` | 入口：只做 `pub mod` 和 `pub use` |

---

## 任务优先级

每轮 tick 严格按照以下顺序执行：

```
1. drain_timers()          把到期 timer 移入 MacroTask 队列
2. 取出一个 MacroTask 执行
3. flush_microtasks()      清空所有 Microtask（含 microtask 中新产生的）
4. 回到 1
```

Microtask（Promise.then / async-await）始终在当前 MacroTask 完成后、下一个 MacroTask 开始前被清空，与浏览器行为一致。

**rAF（requestAnimationFrame）** 属于渲染步骤，优先级低于 Microtask，高于下一个 MacroTask。
渲染由 blitz 驱动，触发时机由渲染层通知 event loop（而非 event loop 自行定时触发）。
当前阶段暂不实现，待渲染层接入后补充。

---

## Timer 实现

```
setTimeout(fn, delay)    → 插入最小堆，deadline = now + delay
setInterval(fn, delay)   → 到期后重新插入堆（self-rescheduling）
clearTimeout/Interval    → 标记 cancelled = true，出堆时丢弃（lazy delete）
```

### 数据结构

- `TimerState.heap: BinaryHeap<TimerHeapEntry>`  
  标准库 `BinaryHeap` 是最大堆，`TimerHeapEntry` 的 `Ord` 实现取反，得到按 deadline 升序的最小堆。
- `TimerState.pending: Vec<PendingTimer>`  
  存储每个 timer 的回调函数（`Persistent<Function<'static>>`）和 interval 配置。
  heap 中只存 `(deadline, id)`，避免把 JS 值混入排序结构。
- `SharedTimers = Arc<Mutex<TimerState>>`  
  JS 回调（`setTimeout` / `clearTimeout`）和 EventLoop 共享同一份 state，通过 Mutex 同步。

### JS 回调注册

JS 侧调用 `setTimeout(fn, delay)` 时：

```
JS callback (setup_timer.rs)
  → func.ctx() 取得与 func 同一 'js 的 Ctx
  → Persistent::save(&ctx, func) 脱离 'js 生命周期
  → TimerState::schedule(persistent, delay_ms, interval_ms) 插入堆
  → 返回 timer id
```

`func.ctx()` 是解决 `Persistent::save` 生命周期约束（`Ctx<'js>` 与 `Function<'js>` 必须同一 `'js`）的关键——从函数值本身取出 ctx，两者天然同 lifetime。

---

## 线程模型（当前 & 规划）

### 当前（仅 timer）

```
JS Thread (单线程)
  └── EventLoop::run()
        ├── drain_timers()        检查堆顶，到期则入队
        ├── tick()                执行 MacroTask + flush microtasks
        └── thread::sleep()       等待下一个 timer 到期（阻塞）
```

`thread::sleep` 仅在没有任何 MacroTask 和 microtask 时才进入，不会阻塞正在运行的 JS。

### 规划（fetch / 文件 I/O 接入后）

```
JS Thread                     tokio Thread Pool
    │                               │
    │── fetch(url) ──→              │ 异步 HTTP
    │   返回 Promise                 │
    │                               │── 完成
    │   (event loop idle)           │── push IoCallback 进 channel
    │←── mpsc::Receiver 收到 ───────│
    │   enqueue MacroTask::IoCallback
    │   resolve Promise
    │   flush microtasks
```

`thread::sleep` 届时需替换为 `tokio::time::sleep`，`EventLoop::run` 改为 `async fn`，
配合 `tokio::select!` 同时等待 timer 和 I/O channel。JS 执行仍严格在单线程 `ctx.with()` 内。

---

## 关键约束

| 约束 | 说明 |
|---|---|
| JS 单线程 | QuickJS 上下文不可跨线程访问，所有 JS 执行必须在同一线程 |
| `Persistent<Function<'static>>` | JS 函数需脱离 `'js` 生命周期存储，通过 `Persistent::save` 实现 |
| setInterval 取消竞态 | lazy delete：timer 到期出堆时检查 `cancelled` 标志，已取消则丢弃 |
| MacroTask 只取一个 | 每轮 tick 只执行一个 MacroTask，保证 microtask 在两个 MacroTask 之间被完整清空 |
