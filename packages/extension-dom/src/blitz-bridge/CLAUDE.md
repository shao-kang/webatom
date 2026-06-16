# Blitz 渲染绑定设计

## 目标

将 webAtom 的轻量 DOM（`extension-dom` 中的 `Document`）与 Blitz 渲染引擎绑定，实现跨端 UI 渲染。

---

## 架构：帧同步双树模型

```
JS 线程                                  Blitz 线程
────────────────────────────────         ──────────────────────────────
自己的轻量 Document (Slab)                blitz-dom Document
  │                                           │
  │  DOM 有变更时覆盖写入共享槽               │  vsync 到来
  └──────────→ DomSlot（共享槽）←── take() ──┘
               Arc<Mutex<Option<             │
                 DomSnapshot>>>           apply_snapshot()
                                              │
                                           重排（Taffy 布局）
                                              │
                                           重绘（WGPU）
```

**核心原则**：JS 线程永远不等待渲染。DOM 操作全部在 JS 线程本地完成，有变更时覆盖写入共享槽；Blitz 线程由 vsync 驱动，每帧主动拉取最新快照。JS 一帧内多次修改 DOM，Blitz 只消费最终状态，不处理中间状态。

---

## 线程模型

| 线程 | 职责 | 约束 |
|---|---|---|
| JS 线程（tokio） | QuickJS 执行、事件循环、DOM 读写 | 单线程，不可阻塞 |
| Blitz 线程（winit） | 窗口管理、布局、绘制 | macOS 要求渲染在主线程 |

两线程通过 `tokio::sync::mpsc` / `crossbeam-channel` 单向通信（JS → Blitz），Blitz 的用户事件（点击、键盘）通过反向 channel 发回 JS 线程触发事件回调。

---

## 数据流

### 共享槽（JS 写 / Blitz 读）

```rust
/// 共享槽：JS 覆盖写，Blitz vsync 时 take
pub struct DomSlot(Arc<Mutex<Option<DomSnapshot>>>);

impl DomSlot {
    /// JS 线程：DOM 有变更时调用，旧值直接丢弃
    pub fn write(&self, snap: DomSnapshot) {
        *self.0.lock().unwrap() = Some(snap);
    }
    /// Blitz 线程：vsync 时调用，取走最新快照
    pub fn take(&self) -> Option<DomSnapshot> {
        self.0.lock().unwrap().take()
    }
}
```

### DomSnapshot 结构

```rust
pub struct DomSnapshot {
    pub nodes: Vec<SnapshotNode>,
    pub root: usize,
}

pub struct SnapshotNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub data: SnapshotNodeData,
}

pub enum SnapshotNodeData {
    Document,
    Element { tag: String, attrs: Vec<(String, String)> },
    Text { content: String },
    Comment { content: String },
}
```

### Blitz → JS（事件回调）

```rust
pub enum BlitzEvent {
    Click { node_id: usize, x: f32, y: f32 },
    KeyDown { key: String, modifiers: u32 },
    Resize { width: u32, height: u32 },
    // ...
}
```

---

## 快照写入时机（JS 侧）

JS 线程在以下时机序列化 DOM 并写入共享槽（有脏标记才写，避免无谓序列化）：

1. **requestAnimationFrame 回调执行完毕后** — 与浏览器语义一致
2. **事件循环空闲（idle）** — 无待处理 macro task 时
3. **显式 `flush()` 调用** — 测试或 SSR 场景

## 快照读取时机（Blitz 侧）

Blitz 线程在 winit `RedrawRequested` 事件（vsync）触发时调用 `DomSlot::take()`，若有新快照则 `apply_snapshot()` 后重排重绘，无新快照则跳过。

---

## 这个目录的职责

`blitz-bridge` 模块负责：

1. **`snapshot.rs`** — 将 `Document` 序列化为 `DomSnapshot`
2. **`diff.rs`** — （后期）对比两次 snapshot 生成 `Vec<DomOp>`（增量 diff）
3. **`blitz_renderer.rs`** — 独立线程：接收 `DomPatch`，调用 blitz-dom API 应用变更，驱动 winit 重绘
4. **`event_bridge.rs`** — Blitz 用户事件 → JS 事件对象，投递到 JS 事件队列

---

## 演进路径

| 阶段 | 实现 |
|---|---|
| 1. 基础渲染 | 全量 `FullSync`，每帧序列化整棵树推给 Blitz |
| 2. 增量同步 | DOM 操作打脏标记，diff 后只推变更节点 |
| 3. 事件回传 | Blitz 点击/键盘事件 → JS DOM 事件分发 |
| 4. 样式支持 | 同步 CSS class / style 属性，触发 Blitz 样式重算 |


## 参考项目
- [blitz](https://github.com/dioxuslabs/blitz) 可以查看目录 ext/blitz  我下载下来了，可以直接搜索
