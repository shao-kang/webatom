# dom-extension Rust 实现设计

## 原语导出方式

Rust 层通过 rquickjs Extension 机制注册一个 native module，JS 层用 ESM import 引入：

```js
import { DocumentHandle, NodeHandle } from 'webatom_ext_native:dom';
```

---

## 两个核心类型

### NodeHandle（不透明句柄）

- 只携带一个内部 `id`（不对外暴露），供 `DocumentHandle` 方法识别节点
- 一个 node 节点 对应唯一handle 
- **不挂载任何 DOM 方法**，JS 端不能直接通过 `NodeHandle` 操作节点
- 由 QuickJS GC 管理生命周期；GC 回收时 Rust 侧 可以直接删除对应节点

```ts
// JS 侧类型声明（Rust 注册的 native class）
declare class NodeHandle {
  // 无公开属性，仅作为不透明 token 传入 DocumentHandle 方法
}
```

### DocumentHandle（操作门面）

- 持有对 Rust 侧 `Document`（SlotMap / Arena）的引用
- 所有节点操作均通过 `DocumentHandle` 实例方法完成，传入 `NodeHandle` 定位节点
- JS 层通过 `DocumentHandle.create()` 创建一个新文档，返回 `DocumentHandle` 实例

```ts
declare class DocumentHandle {
  // --- 工厂 ---
  static create(): DocumentHandle;

  // --- 节点创建（返回 NodeHandle） ---
  createElement(tag: string): NodeHandle;
  createTextNode(data: string): NodeHandle;
  createComment(data: string): NodeHandle;
  createDocumentFragment(): NodeHandle;

  // --- 文档结构 ---
  documentElement(): NodeHandle | null;

  // --- 树操作 ---
  appendChild(parent: NodeHandle, child: NodeHandle): void;
  removeChild(parent: NodeHandle, child: NodeHandle): void;
  insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void;
  replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void;

  // --- 树查询 ---
  parentNode(node: NodeHandle): NodeHandle | null;
  firstChild(node: NodeHandle): NodeHandle | null;
  nextSibling(node: NodeHandle): NodeHandle | null;

  // --- 节点属性读写 ---
  nodeType(node: NodeHandle): number;
  tagName(node: NodeHandle): string;
  nodeValue(node: NodeHandle): string | null;
  setNodeValue(node: NodeHandle, value: string | null): void;

  // --- 元素属性读写 ---
  getAttribute(node: NodeHandle, name: string): string | null;
  setAttribute(node: NodeHandle, name: string, value: string): void;
  removeAttribute(node: NodeHandle, name: string): void;
  hasAttribute(node: NodeHandle, name: string): boolean;
  attributes(node: NodeHandle): [string, string][];   // [[name, value], ...]
}
```

---


## JS 层用法示例

```js
import { DocumentHandle } from 'webatom_ext_native:dom';

const doc = DocumentHandle.create();
const div = doc.createElement('div');
const text = doc.createTextNode('hello');
doc.appendChild(div, text);
doc.appendChild(doc.documentElement(), div);
```

JS 层的 `Document`、`Node`、`Element` 等 W3C 类均是对 `DocumentHandle` + `NodeHandle` 的包装，不直接持有任何 DOM 数据。

---

## Rust 实现要点

- `NodeHandle` → `Class<NodeHandle>` 注册，内部只存 `NodeId(u32)`，**不**存 `Rc<RefCell<Document>>`
- `DocumentHandle` → `Class<DocumentHandle>` 注册，内部存 `Rc<RefCell<Document>>`
- 每个 `DocumentHandle` 方法接收 `NodeHandle` 参数，通过 `doc.borrow().arena[id]` 定位节点
- 无效 `NodeId` 抛出 `DOMException`（见 CLAUDE.md 设计规则 §4）
