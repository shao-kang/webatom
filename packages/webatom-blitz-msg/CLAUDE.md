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
| `msg.rs` | 信道消息枚举：`DomMsg { Full / Patch / QueryLayout / LayoutNotifyRequest }` |
| `event.rs` | `Event`：Blitz → JS 的用户事件、`RafTick`、`LayoutResult`、`LayoutNotify` |
| `layout.rs` | `NodeLayout`：布局数据结构体 |
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

### DomMsg（JS → Blitz）

```rust
pub enum DomMsg {
    /// 首帧或强制重建时发送整棵树
    Full(DomSnapshot),
    /// 正常帧：一批有序的增量操作
    Patch(Vec<DomOp>),
    /// 异步查询指定节点布局；Blitz 完成本批次布局后通过 Event::LayoutResult 回调
    QueryLayout { node_id: usize },
    /// nextTick 语义：请求 Blitz 在当前 patch 应用并完成布局后发 Event::LayoutNotify 回调
    LayoutNotifyRequest,
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
    /// vsync 信号：Blitz 渲染完一帧后发出，JS 侧据此触发 requestAnimationFrame 回调
    /// 走独立的 bounded(1) channel，可跳帧，不混入 Event
    RafTick,  // 已移至独立 raf channel，此处仅保留文档说明
    /// 异步布局查询结果，对应 DomMsg::QueryLayout
    LayoutResult(NodeLayout),
    /// nextTick 回调：对应 DomMsg::LayoutNotifyRequest，当前 patch 已应用并完成布局
    LayoutNotify,
}
```

### NodeLayout（布局数据）

```rust
pub struct NodeLayout {
    pub node_id: usize,
    /// 相对于视口的位置与尺寸（对应 getBoundingClientRect）
    pub x:      f32,
    pub y:      f32,
    pub width:  f32,
    pub height: f32,
    /// 元素 scrollLeft / scrollTop
    pub scroll_left: f32,
    pub scroll_top:  f32,
    /// clientWidth / clientHeight（content + padding，不含滚动条）
    pub client_width:  f32,
    pub client_height: f32,
}
```

---

## Channel 设计

```rust
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};

pub struct JsSide {
    pub dom_tx:   Sender<DomMsg>,   // JS → Blitz：patch / query / notify request
    pub event_rx: Receiver<Event>,  // Blitz → JS：用户事件 / 布局回调（不可丢）
    pub raf_rx:   Receiver<()>,     // Blitz → JS：vsync 信号（bounded(1)，可跳帧）
}

pub struct BlitzSide {
    pub dom_rx:   Receiver<DomMsg>, // Blitz vsync 时取消息
    pub event_tx: Sender<Event>,    // 投递事件/布局结果到 JS（不可丢）
    pub raf_tx:   Sender<()>,       // vsync 后 try_send，满则静默跳帧
}

pub fn create_channels() -> (JsSide, BlitzSide) { ... }
```

### 语义

| 方向 | Channel | 消息 | 语义 |
|---|---|---|---|
| JS → Blitz | `dom_tx` (unbounded) | `DomMsg::Patch` | 按序合并，vsync 时 drain |
| JS → Blitz | `dom_tx` (unbounded) | `DomMsg::Full` | 触发全量重建，丢弃之前所有 Patch |
| JS → Blitz | `dom_tx` (unbounded) | `DomMsg::QueryLayout` | 异步查询，布局完成后回 `Event::LayoutResult` |
| JS → Blitz | `dom_tx` (unbounded) | `DomMsg::LayoutNotifyRequest` | nextTick，布局完成后回 `Event::LayoutNotify` |
| Blitz → JS | `event_tx` (unbounded) | `Event` | **全量投递，不可丢** |
| Blitz → JS | `raf_tx` (bounded(1)) | `()` | vsync tick，**可跳帧**；`try_send` 满则静默丢弃 |

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
JS 执行/微任务耗尽
  → send Patch(ops)          ──Sender<DomMsg>──→  drain → apply_patch → 布局
首帧
  → send Full(snap)          ──Sender<DomMsg>──→  apply_full → 布局
                                                         │
                                                         ↓ vsync（布局完成后）
raf_rx.try_recv() ← ()        ←──Sender<()>────────── try_send(())  ← 跳帧安全
rAF 回调（可能再次 send Patch）

// 异步获取布局（getBoundingClientRect 等）
send QueryLayout { node_id } ──Sender<DomMsg>──→  布局完成后
event_rx ← LayoutResult      ←──Sender<Event>──── 发 Event::LayoutResult(NodeLayout)

// nextTick：等当前 patch 应用并布局完成后回调
send LayoutNotifyRequest     ──Sender<DomMsg>──→  布局完成后
event_rx ← LayoutNotify      ←──Sender<Event>──── 发 Event::LayoutNotify

event_rx ← Click/Key/…       ←──Sender<Event>──── winit 用户事件
```
