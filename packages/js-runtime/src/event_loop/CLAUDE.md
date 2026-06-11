# event_loop 模块

QuickJS 不内建驱动机制，宿主（webAtom）需要在 Rust 侧主动推动引擎执行。本模块实现符合
[HTML Event Loop 规范](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)
的事件循环，保证 Promise / timer / I/O 的执行顺序与浏览器一致。

---

## 模块结构

| 文件 | 职责 |
|---|---|
| `run.rs` | `EventLoop` 结构体、主循环驱动逻辑 |
| `handle.rs` | `HandleGuard`、`active_handles` 引用计数 |
| `task.rs` | `FrameTask`、`IdleTask` 类型定义 |
| `event_loop.rs` | 入口：只做 `pub mod` 和 `pub use` |

---

## 终止条件：Active Handle 计数（借鉴 Node.js libuv）

Event loop 的核心退出判断：**`active_handles == 0`**，即没有任何需要等待的异步操作时退出。

每个"有意义的等待"都持有一个 `HandleGuard`，drop 时自动减计数：

```rust
pub struct HandleGuard(Arc<AtomicUsize>);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
```

### 各类 Handle 的计数规则

| Handle 类型 | count +1 时机 | count -1 时机 |
|---|---|---|
| `setTimeout` | 注册时 | 回调执行后（Guard 随 spawn future drop） |
| `setInterval` | 注册时 | `clearInterval` 时 |
| `fetch` | 发起请求时 | resolve / reject 时 |
| `WebSocket` | 连接建立时 | `close()` 或连接断开时 |
| `requestIdleCallback` | 注册时 | 回调执行后 |
| `requestAnimationFrame` | **不计入** | 帧率由渲染线程驱动，不阻止退出 |

---

## 主循环结构

```rust
pub async fn run(&self, context: &AsyncContext) -> rquickjs::Result<()> {
    loop {
        tokio::select! {
            // A: 推进 timer / I/O spawn'd futures，顺带清空 microtask
            _ = self.runtime.drive() => {}

            // B: 渲染线程通知帧完成（headless 模式下此 channel 不发送）
            Some(frame) = self.frame_rx.recv() => {
                self.flush_frame_tasks(context, frame).await?;
            }
        }

        // 检查退出条件：无任何活跃异步操作
        if self.active_handles.load(Ordering::Relaxed) == 0 {
            break;
        }
    }
    Ok(())
}
```

`drive()` vs `idle()`：
- `drive()` — 每次 poll 推进一步，配合 `active_handles` 精确控制退出，适合长生命周期任务（WebSocket、setInterval）
- `idle()` — 等所有 future 完成才返回，仅适合全部任务可终止的场景

---

## 任务优先级

每轮 tick 严格按照以下顺序：

```
① microtasks（QuickJS job queue）
     Promise.then / async-await / queueMicrotask
     → 清空为止

② timer / I/O 回调（ctx.spawn 直接在 JS 线程执行）
     → 执行后产生新 microtask → 回到 ①

③ 帧后任务（渲染线程 notify 后才触发）
     rAF 回调 / ResizeObserver / IntersectionObserver
     → 执行后产生新 microtask → 回到 ①

④ Idle 回调（active_handles == 0 前的空闲时间）
     requestIdleCallback
```

微任务优先于帧任务的原因：帧任务通常需要读 layout 结果，微任务执行完毕后 DOM 状态才稳定，此时读到的 layout 才准确，与浏览器规范一致。

---

## Timer 实现

```
setTimeout(fn, delay)    → acquire HandleGuard，ctx.spawn 异步 sleep，到期执行后 Guard drop
setInterval(fn, delay)   → acquire HandleGuard，ctx.spawn 无限循环 sleep，clearInterval 时 cancel
clearTimeout/Interval    → 标记 cancelled = true（AtomicBool），spawn loop 检测后 break
```

Timer 回调直接在 `ctx.spawn` 的 future 中调用，不经过 `macro_queue` 中转。

---

## 帧任务接入（渲染层）

帧任务由渲染线程（blitz）驱动，event loop 通过 `mpsc::Receiver<FrameNotify>` 被动接收：

```
渲染线程完成一帧
  → send FrameNotify 到 event loop
  → event loop 在 tokio::select! 的 B 分支收到
  → flush rAF 队列 → flush microtasks
  → 渲染线程可安全读取 layout 结果
```

rAF 不持有 HandleGuard，纯帧驱动，不阻止进程退出。

---

## 线程模型

```
JS Thread (tokio current_thread)
  └── EventLoop::run()
        ├── tokio::select!
        │     ├── runtime.drive()      推进 timer / I/O spawn'd futures
        │     └── frame_rx.recv()      等待渲染帧通知
        └── active_handles == 0 → 退出

tokio Thread Pool（非 JS 线程）
  ├── fetch / 文件 I/O 异步执行
  └── 完成后通过 ctx.spawn 将回调 post 回 JS 线程
```

所有 JS 执行严格在单线程 `context.with()` 内，跨线程操作只传递数据（不传 JS 值）。

---

## 关键约束

| 约束 | 说明 |
|---|---|
| JS 单线程 | QuickJS 上下文不可跨线程访问，所有 JS 执行必须在同一线程 |
| `Persistent<Function<'static>>` | JS 函数需脱离 `'js` 生命周期存储，通过 `Persistent::save` 实现 |
| HandleGuard 所有权 | Guard 必须随 spawn future 一起 move，不得提前 drop |
| setInterval 取消竞态 | lazy cancel：AtomicBool，loop 每次 sleep 前检查 |
| microtask 优先于帧任务 | 帧任务执行前必须先清空 QuickJS job queue |
