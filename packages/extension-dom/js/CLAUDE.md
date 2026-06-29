# dom-extension JS 层设计

## 核心原则

- JS 层的 `Node`、`Element`、`HTMLElement`、`Document` 等类尽可能贴近 [W3C DOM Living Standard](https://dom.spec.whatwg.org/)，让用户代码无需适配即可运行。
- 所有 DOM 数据存储在 Rust 侧，JS 层不缓存任何节点数据，只持有不透明句柄（Handle）。

---

## 两个底层原语

通过 `import { DocumentHandle, NodeHandle } from 'webatom_ext_native:dom'` 引入（native 模块由 Rust 注册）。

### NodeHandle（节点句柄）

- 对应 Rust 侧一个具体节点，完全不透明
- **不暴露任何属性或方法**，仅作为 token 传入 `DocumentHandle` 的方法以定位节点
- 由 QuickJS GC 管理生命周期；Drop 时 Rust 侧可直接回收对应节点（若已脱离树）

### DocumentHandle（文档门面）

- 持有 Rust 侧 `Document`（Slab/SlotMap）的引用
- 所有节点操作（创建、查询、树变更、属性读写）均为其实例方法，接受 `NodeHandle` 参数定位节点
- **由 `DocumentContext` 创建并持有**，外部代码不直接 `new DocumentHandle()`

---

## DocumentContext 桥接层（document-context.ts）

`DocumentContext` 是一个 **bridge class**，同时承担三个职责：

1. **创建并持有** `DocumentHandle`（整个文档唯一一个）
2. 将 `_docHandle` 的 `NodeHandle` 级别原语**封装为 `Node` 级别**的桥接方法
3. 持有 `_nodes`（保活 Set）和 `_handleNodeMap`（handle → Node 实例缓存）

```ts
// src/interface/document-context.ts
import { DocumentHandle, NodeHandle } from './native';
import { Node, wrapHandleWith } from './node';  // 运行时单向导入

export class DocumentContext {
  readonly _docHandle: DocumentHandle;
  readonly _nodes: Set<Node> = new Set();
  readonly _handleNodeMap: WeakMap<NodeHandle, Node> = new WeakMap();

  constructor() {
    this._docHandle = new DocumentHandle();  // 唯一一次 new DocumentHandle()
  }
}
```

### 方法职责划分

| 方法类型 | 入参 | 返回值 | 说明 |
|---------|------|--------|------|
| 标量查询 | `NodeHandle` | 基础类型 | 直接透传 `_docHandle`，无包装 |
| 树遍历 | `NodeHandle` | `Node \| null` | 调用 `_docHandle` + `wrapHandleWith` 包装 |
| 文档结构查询 | — | `Node \| null` | `documentElement / body / head`，同上 |
| 树变更 | `NodeHandle, NodeHandle` | `void` | 直接透传，`_nodes` 由上层 Node 方法维护 |
| 节点创建 | — | `NodeHandle` | 返回原始句柄（游离，不进 `_nodes`） |
| Bootstrap | — | `NodeHandle` | `documentNode()`，仅供 `Document` 构造使用 |

### 完整方法签名

```ts
class DocumentContext {
  // 标量查询
  nodeType(node: NodeHandle): number
  tagName(node: NodeHandle): string | null
  nodeValue(node: NodeHandle): string | null
  setNodeValue(node: NodeHandle, value: string | null): void
  getAttribute(node: NodeHandle, name: string): string | null
  setAttribute(node: NodeHandle, name: string, value: string): void
  removeAttribute(node: NodeHandle, name: string): void
  hasAttribute(node: NodeHandle, name: string): boolean
  attributes(node: NodeHandle): [string, string][]

  // 树遍历（内部调用 wrapHandleWith，返回 Node | null）
  parentNode(node: NodeHandle): Node | null
  firstChild(node: NodeHandle): Node | null
  lastChild(node: NodeHandle): Node | null
  nextSibling(node: NodeHandle): Node | null
  previousSibling(node: NodeHandle): Node | null

  // 文档结构（内部调用 wrapHandleWith）
  documentElement(): Node | null
  body(): Node | null
  head(): Node | null

  // 树变更（NodeHandle 入参，_nodes 由 Node 层维护）
  appendChild(parent: NodeHandle, child: NodeHandle): void
  removeChild(parent: NodeHandle, child: NodeHandle): void
  insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void
  replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void

  // 节点创建（返回 NodeHandle，游离状态）
  createElement(tagName: string): NodeHandle
  createTextNode(data: string): NodeHandle
  createComment(data: string): NodeHandle

  // Bootstrap 专用（仅 Document 构造时调用）
  documentNode(): NodeHandle
}
```

---

## 模块依赖与运行时求值顺序

```
webatom_ext_native:dom  (Rust native)
        ↓
native.ts               re-export NodeHandle, DocumentHandle
        ↓  运行时导入
node.ts                 定义 Node、wrapHandleWith
                        import type { DocumentContext }  ← 仅类型，运行时无依赖
        ↓  运行时导入 Node + wrapHandleWith
document-context.ts     定义 DocumentContext
        ↓
document.ts             定义 Document（new DocumentContext()）
        ↓
element.ts              定义 Element，registerNodeType(ELEMENT_NODE)
        ↓
window.ts               入口，组装 window 全局对象
```

**循环分析**：
- `node.ts` 对 `document-context.ts` 只做 `import type`，运行时零依赖
- `document-context.ts` 运行时导入 `node.ts`（单向）
- 运行时求值顺序线性无环

---

## Document.constructor 模式

```ts
// src/interface/document.ts
export class Document extends Node {
  constructor() {
    const docCtx = new DocumentContext();     // DocumentContext 内部 new DocumentHandle()
    const docNode = docCtx.documentNode();    // bootstrap：取文档根 NodeHandle
    super(docCtx, docNode);                   // Node._ctx = docCtx, _handle = docNode
    docCtx._handleNodeMap.set(docNode, this); // 文档根句柄 → Document 实例
    docCtx._nodes.add(this);                  // 保活
  }
}
```

`Document` 完全不感知 `DocumentHandle`，只与 `DocumentContext` 交互。

---

## Node 构造与遍历

```ts
class Node extends EventTarget {
  _ctx: DocumentContext;   // 同文档所有节点共享同一个 ctx
  _handle: NodeHandle;

  constructor(ctx: DocumentContext, handle: NodeHandle) {
    super();
    this._ctx = ctx;
    this._handle = handle;
  }

  // 遍历：直接调用 _ctx bridge 方法，返回 Node | null
  get parentNode(): Node | null  { return this._ctx.parentNode(this._handle); }
  get firstChild(): Node | null  { return this._ctx.firstChild(this._handle); }
  get nextSibling(): Node | null { return this._ctx.nextSibling(this._handle); }

  get childNodes(): Node[] {
    const result: Node[] = [];
    let child = this._ctx.firstChild(this._handle);
    while (child) {
      result.push(child);
      child = this._ctx.nextSibling(child._handle);  // 用子节点 _handle 继续遍历
    }
    return result;
  }

  // 变更：_ctx 做 Rust 操作，Node 层维护 _nodes
  appendChild<T extends Node>(node: T): T {
    this._ctx.appendChild(this._handle, node._handle);
    this._ctx._nodes.add(node);
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    this._ctx.removeChild(this._handle, child._handle);
    this._ctx._nodes.delete(child);
    return child;
  }
}
```

---

## wrapHandleWith（node.ts 内部函数）

```ts
export function wrapHandleWith(ctx: DocumentContext, handle: NodeHandle | null): Node | null {
  if (!handle) return null;
  // 优先复用已有 JS 实例
  const existing = ctx._handleNodeMap.get(handle);
  if (existing) return existing;
  // 按 nodeType 选择正确子类工厂
  const type = ctx._docHandle.nodeType(handle);
  const factory = getNodeFactory(type);
  const node = factory ? factory(ctx, handle) : new Node(ctx, handle);
  ctx._handleNodeMap.set(handle, node);
  // HTML 解析产生的树内节点，首次 JS 访问时立即保活
  if (ctx._docHandle.parentNode(handle) !== null) {
    ctx._nodes.add(node);
  }
  return node;
}
```

`DocumentContext` 的树遍历方法均调用此函数。`Node` 层通常不直接调用 `wrapHandleWith`，除 `textContent` setter 等需直接操作 NodeHandle 的场合。

---

## 节点生命周期

```
new DocumentContext()
  → new DocumentHandle()（Rust 创建 Document 及根节点）

createElement / createTextNode
  → DocumentContext.createElement() → NodeHandle（游离，不进 _nodes）
  → Document.createElement() 调用 wrapHandleWith → 存入 _handleNodeMap

HTML 解析产生的树内节点
  → 首次通过 DocumentContext 遍历方法访问
  → wrapHandleWith 检测 parentNode !== null → 加入 _nodes

appendChild / insertBefore
  → Node 层：_ctx 做 Rust 树操作 + _ctx._nodes.add(node)

removeChild
  → Node 层：_ctx 做 Rust 树操作 + _ctx._nodes.delete(child)
  → 若无其他 JS 引用 → GC → NodeHandle.drop() → Rust 侧节点回收
```

---

## 类层次与职责

```
EventTarget              event-target.ts
  └── Node               node.ts          _ctx + _handle，W3C Node 接口
        ├── Document     document.ts      new DocumentContext()，W3C Document 接口
        └── Element      element.ts       W3C Element 接口
              └── HTMLElement            W3C HTMLElement（后续实现）
```

| 类 | 构造参数 | 关键职责 |
|---|---|---|
| `DocumentContext` | `()` | new DocumentHandle，桥接所有节点操作 |
| `Node` | `ctx, handle` | 树遍历、树变更、nodeValue/textContent |
| `Document` | `()` | new DocumentContext，createElement/getElementById |
| `Element` | `ctx, handle` | tagName/id/className/classList/getAttribute |

---

## 节点工厂注册（html/index.ts）

```ts
import type { DocumentContext } from '@/interface/document-context';
import type { NodeHandle } from '@/interface/native';
import type { Node } from '@/interface/node';

type NodeFactory = (ctx: DocumentContext, handle: NodeHandle) => Node;

const nodeRegistry = new Map<number, NodeFactory>();
export function registerNodeType(nodeType: number, factory: NodeFactory): void
export function getNodeFactory(nodeType: number): NodeFactory | undefined
```

`element.ts` 底部注册：`registerNodeType(Node.ELEMENT_NODE, (ctx, h) => new Element(ctx, h))`

`window.ts` 必须在 `new Document()` 之前执行 `import '@/interface/element'`（side-effect），确保工厂已注册。

---

## Element 接口（element.ts）

```ts
export class Element extends Node {
  get tagName(): string                               // 大写
  get id(): string;       set id(v: string)
  get className(): string; set className(v: string)
  get classList(): DOMTokenList                       // 最小 shim

  getAttribute(name: string): string | null
  setAttribute(name: string, value: string): void
  removeAttribute(name: string): void
  hasAttribute(name: string): boolean
  getAttributeNames(): string[]

  get children(): Element[]
  get firstElementChild(): Element | null
  get lastElementChild(): Element | null
  get childElementCount(): number
  get nextElementSibling(): Element | null
  get previousElementSibling(): Element | null

  append(...nodes: (Node | string)[]): void
  prepend(...nodes: (Node | string)[]): void
  remove(): void
}
```
