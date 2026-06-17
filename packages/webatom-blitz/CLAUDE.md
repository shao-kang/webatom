# webatom-blitz

## 职责

在主线程上运行 Blitz 渲染引擎，接收 JS 线程发来的 `DomMsg`，将其应用到 `blitz-dom` 的 `BaseDocument`，并将窗口事件（点击、键盘等）通过 `BlitzEvent` 反馈给 JS 线程。

依赖 `webatom-blitz-msg` 获取消息类型，依赖 `blitz-shell` + `blitz-dom` 驱动渲染。

---

## 线程模型

| 线程 | 职责 | 约束 |
|---|---|---|
| **主线程**（winit） | 运行 `BlitzApplication`、布局、绘制 | macOS 强制要求渲染在主线程 |
| **JS 线程**（tokio） | QuickJS 执行、事件循环、DOM 变更、发送 `DomMsg` | 单线程，不可阻塞 |

JS 线程发送 `DomMsg` 后，通过 `EventLoopProxy::send_event` 唤醒主线程处理。

---

## 模块

| 文件 | 职责 |
|---|---|
| `src/renderer.rs` | `WebAtomRenderer`：持有 `BaseDocument`，应用 `DomMsg`（Full / Patch） |
| `src/app.rs` | `WebAtomApp`：包装 `BlitzApplication`，处理自定义事件（DOM 更新唤醒、关闭等） |
| `src/event.rs` | `convert_event()`：winit `WindowEvent` → `BlitzEvent`，投递到 `event_tx` |
| `src/lib.rs` | crate 根，导出 `run()` 入口函数 |

---

## 核心类型

### WebAtomRenderer

```rust
pub struct WebAtomRenderer {
    doc: BaseDocument,
    /// webAtom node id → blitz node id（Full 时重建，Patch 时维护）
    id_map: HashMap<usize, usize>,
}

impl WebAtomRenderer {
    pub fn apply_full(&mut self, snap: &DomSnapshot) { ... }
    pub fn apply_patch(&mut self, ops: &[DomOp]) { ... }
    pub fn document(&self) -> &BaseDocument { ... }
    pub fn document_mut(&mut self) -> &mut BaseDocument { ... }
}
```

`apply_full`：清空 blitz root 的所有子节点，按 `DomSnapshot` 重建整棵树，重置 `id_map`。

`apply_patch`：按 `DomOp` 顺序调用 `DocumentMutator` API，维护增量 `id_map`。

### blitz-dom API 对照

| DomOp | DocumentMutator 方法 |
|---|---|
| `CreateElement { tag, attrs }` | `mutator.create_element(QualName, Vec<Attribute>)` |
| `CreateText { content }` | `mutator.create_text_node(&str)` |
| `CreateComment { content }` | `mutator.create_comment_node()` |
| `AppendChild { parent, child }` | `mutator.append_children(parent_id, &[child_id])` |
| `InsertBefore { parent, child, before }` | `mutator.insert_nodes_before(before_id, &[child_id])` |
| `RemoveChild { parent, child }` | `mutator.remove_node(child_id)` |
| `DropNode { id }` | `mutator.remove_and_drop_node(node_id)` |
| `SetAttribute { node, name, value }` | `mutator.set_attribute(node_id, QualName, &str)` |
| `SetAttributes { node, attrs }` | 多次调用 `set_attribute` |
| `RemoveAttribute { node, name }` | `mutator.clear_attribute(node_id, QualName)` |
| `SetTextContent { node, content }` | `mutator.set_node_text(node_id, &str)` |

> `DocumentMutator` Drop 时自动 flush（重算样式 + 布局），不需要手动调用。

---

## 事件流

```
winit WindowEvent
    │
    ▼
window.handle_winit_event(event)     ← 由 blitz-shell 内部处理点击命中测试
    │
    ├─ 非用户事件（resize/redraw）→ 正常渲染流程
    │
    └─ 用户事件（click/key）→ convert_event() → BlitzEvent
                                                   │
                                          event_tx.send(BlitzEvent)
                                                   │
                                          JS 线程 event_rx.recv()
```

---

## Channel 说明

| Channel | 类型 | 用途 |
|---|---|---|
| `dom_rx: Receiver<DomMsg>` | **crossbeam-channel** | JS → Blitz，DOM 更新消息 |
| `event_tx: Sender<BlitzEvent>` | **crossbeam-channel** | Blitz → JS，用户事件回传 |
| `BlitzApplication::event_queue` | `std::sync::mpsc`（blitz-shell 内部） | winit 事件循环唤醒，**不可更改**，仅用于触发主线程 wake up |

webAtom 自己的消息（`DomMsg` / `BlitzEvent`）**全部走 crossbeam-channel**。`std::sync::mpsc` 只作为 winit 事件循环的唤醒触发器，不传递业务数据。

### BlitzEvent 回传机制

Blitz 主线程命中测试后调用 `event_tx.send(BlitzEvent)`（crossbeam，无锁，`Send + Sync`）。JS 侧不订阅推送，而是在事件循环每个 tick 的 idle 阶段主动轮询：

```rust
// JS 事件循环 idle/tick 阶段
while let Ok(evt) = event_rx.try_recv() {
    dispatch_to_js(evt); // 转为 JS 事件对象投入队列
}
```

`try_recv()` 是非阻塞无锁操作，与浏览器事件循环的轮询语义一致。阶段 1 使用此方式，无需 tokio 桥接。

---

## 唤醒机制

JS 线程发送 `DomMsg` 后**必须**唤醒 Blitz 主线程，否则消息会堆积到下次 vsync：

```rust
// JS 侧（crossbeam-channel 发消息，再用 EventLoopProxy 唤醒主线程）
blitz_side.dom_tx.send(DomMsg::Patch(ops))?;
event_loop_proxy.send_event(BlitzShellEvent::Embedder(Arc::new(WakeUpEvent)))?;

// Blitz 侧 WebAtomApp::proxy_wake_up 中
BlitzShellEvent::Embedder(evt) if evt.is::<WakeUpEvent>() => {
    // 从 crossbeam Receiver 排干消息
    let msgs = self.blitz_side.drain_dom_msgs();
    for msg in msgs {
        match msg {
            DomMsg::Full(snap) => self.renderer.apply_full(&snap),
            DomMsg::Patch(ops) => self.renderer.apply_patch(&ops),
        }
    }
    window.request_redraw();
}
```

---

## run() 入口

```rust
pub fn run(blitz_side: BlitzSide, proxy: EventLoopProxy, config: WindowConfig) {
    let event_loop = create_default_event_loop();

    // blitz-shell 内部事件队列（std::sync::mpsc，仅供 winit wake up 使用）
    let (shell_tx, shell_rx) = std::sync::mpsc::channel::<BlitzShellEvent>();
    let blitz_proxy = BlitzShellProxy::new(shell_tx, event_loop.create_proxy());

    let mut app = WebAtomApp::new(blitz_proxy, shell_rx, blitz_side);
    app.add_window(config);
    event_loop.run_app(&mut app).unwrap();
}
```

---

## 依赖

```toml
[dependencies]
webatom-blitz-msg = { path = "../webatom-blitz-msg" }
blitz-shell       = { path = "../../ext/blitz/packages/blitz-shell" }
blitz-dom         = { path = "../../ext/blitz/packages/blitz-dom" }
blitz             = { path = "../../ext/blitz/packages/blitz" }
crossbeam-channel = "0.5"
markup5ever       = "0.11"
```
