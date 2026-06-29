# webatom-blitz-msg

## 职责

定义 JS 线程（rquickjs）与 Blitz 渲染线程之间通信的**共享消息类型与 channel 工厂**。

纯数据结构 crate，不依赖 `blitz-dom`、`rquickjs` 或任何运行时。
线程间通信**统一使用 `crossbeam_channel`**。

---

## 模块

| 文件 | 职责 |
|---|---|
| `snapshot.rs` | 全量快照类型：`DomSnapshot`、`SnapshotNode`、`SnapshotNodeData` |
| `patch.rs` | 增量操作类型：`DomOp` |
| `msg.rs` | 信道消息枚举：`DomMsg { Full / Patch }` |
| `event.rs` | `Event`：Blitz → JS 的用户事件 |
| `channel.rs` | `create_channels()` 工厂 + 端口结构体 |

> `dom_slot.rs` / `renderer.rs` 不属于这个 crate。

---

## 核心类型

### DomSnapshot（全量，首帧 / 重建用）

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

### DomOp（增量，覆盖 DOM 全部变更路径）

```rust
pub enum DomOp {
    // ── 节点创建（JS 分配 id，先创建再插入）──────────────────────────
    CreateElement { id: usize, tag: String, attrs: Vec<(String, String)> },
    CreateText    { id: usize, content: String },
    CreateComment { id: usize, content: String },

    // ── 树结构变更 ────────────────────────────────────────────────────
    /// Node.appendChild(parent, child)
    AppendChild  { parent: usize, child: usize },
    /// Node.insertBefore(parent, child, before_sibling)
    InsertBefore { parent: usize, child: usize, before: usize },
    /// Node.removeChild(parent, child)  —— 只从树中摘除，节点仍存在
    RemoveChild  { parent: usize, child: usize },
    /// 节点从 JS 侧彻底释放（无任何 JS 引用）
    DropNode     { id: usize },

    // ── 属性变更 ──────────────────────────────────────────────────────
    SetAttribute    { node: usize, name: String, value: String },
    SetAttributes   { node: usize, attrs: Vec<(String, String)> },
    RemoveAttribute { node: usize, name: String },

    // ── 文本内容变更 ──────────────────────────────────────────────────
    /// CharacterData.data = / Text / Comment 节点内容
    SetTextContent { node: usize, content: String },
}
```

### DomMsg（信道消息，两种模式合并为一个枚举）

```rust
pub enum DomMsg {
    /// 首帧或强制重建时发送整棵树
    Full(DomSnapshot),
    /// 正常帧：一批有序的增量操作
    Patch(Vec<DomOp>),
}
```

### Event（Blitz → JS）

```rust
pub enum Event {
    Click    { node_id: usize, x: f32, y: f32 },
    KeyDown  { key: String, modifiers: u32 },
    KeyUp    { key: String, modifiers: u32 },
    Resize   { width: u32, height: u32 },
    Focus    { node_id: usize },
    Blur     { node_id: usize },
}
```

---

## Channel 设计

```rust
use crossbeam_channel::{unbounded, Receiver, Sender};

pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,       // JS → Blitz：发快照 / patch
    pub event_rx: Receiver<Event>, // Blitz → JS：接用户事件
}

pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>,     // Blitz vsync 时取消息
    pub event_tx: Sender<Event>,   // 投递用户事件到 JS
}

pub fn create_channels() -> (JsSide, BlitzSide) { ... }
```

### 语义

| 方向 | 消息 | 语义 |
|---|---|---|
| JS → Blitz | `DomMsg` | **最新批次优先**：vsync 时排干队列，`Patch` 按序合并，`Full` 触发后丢弃之前所有 `Patch` |
| Blitz → JS | `Event` | **全量投递**，事件不可丢 |

**Blitz vsync 消费逻辑**：

```rust
fn drain_dom_msgs(rx: &Receiver<DomMsg>) -> Option<DomMsg> {
    let mut result: Option<DomMsg> = None;
    while let Ok(msg) = rx.try_recv() {
        match msg {
            // Full 重置一切
            DomMsg::Full(_) => result = Some(msg),
            DomMsg::Patch(ops) => match &mut result {
                // Full 之后收到 Patch：先全量建树，Patch 留到下一帧
                Some(DomMsg::Full(_)) => { rx.requeue(ops); break; } // 伪码
                // 合并 Patch
                Some(DomMsg::Patch(acc)) => acc.extend(ops),
                None => result = Some(DomMsg::Patch(ops)),
            },
        }
    }
    result
}
```

> 实际 requeue 可用一个临时 `Vec` 暂存，不依赖 channel 的 requeue 能力。

---

## 依赖约束

- 允许：`crossbeam-channel`、`serde`（按需 feature）
- 禁止：`blitz-dom`、`rquickjs`、`tokio`、`winit`
- 所有类型须实现 `Clone + Debug`

---

## 数据流

```
JS 线程                                               Blitz 线程
──────────────────────                                ────────────────────
DOM 变更 → 记录 DomOp
rAF 结束 → send Patch(ops)  ──Sender<DomMsg>──────→  drain → apply_full / apply_patch
首帧      → send Full(snap)                               │
                                                          ↓
event_rx.recv()  ←──────────Sender<Event>─────── winit 事件
```
