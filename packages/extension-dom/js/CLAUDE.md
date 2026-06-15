# dom-extension JS 层设计

## 核心原则

- JS 层的 `Node`、`Element`、`HTMLElement`、`Document` 等类尽可能贴近 [W3C DOM Living Standard](https://dom.spec.whatwg.org/)，让用户代码无需适配即可运行。
- 所有 DOM 数据存储在 Rust 侧，JS 层不缓存任何节点数据，只持有不透明句柄（Handle）。

---

## 两个底层原语

通过 `import { DocumentHandle, NodeHandle } from './native'` 引入（native 模块由 Rust 注册）。

### NodeHandle（节点句柄）

- 对应 Rust 侧一个具体节点，完全不透明
- **不暴露任何属性或方法**，仅作为 token 传入 `DocumentHandle` 的方法以定位节点
- 由 QuickJS GC 管理生命周期

### DocumentHandle（文档门面）

- 持有 Rust 侧 `Document`（Slab）的引用
- 所有节点操作（创建、查询、树变更、属性读写）均为其实例方法，接受 `NodeHandle` 参数定位节点
- 每个节点在 `DocumentHandle` 内部有唯一对应的 `NodeHandle`（`get_or_create` 缓存）

---

## DocumentContext 接口（document.ts）

`DocumentContext` 定义在 `document.ts`，由 `Document.#docCtx` 实现：

```ts
export interface DocumentContext {
  readonly _docHandle: DocumentHandle;
  readonly _nodes: Set<Node>;
  readonly _handleNodeMap: WeakMap<NodeHandle, Node>;
}
```

- `_docHandle`：DocumentHandle 实例，所有节点操作的入口
- `_nodes`：强引用 Set，保活所有仍在 DOM 树中的 Node 实例，防止 GC 回收
- `_handleNodeMap`：WeakMap，从 NodeHandle → Node 实例，保证同一底层节点始终复用同一 JS 对象

`node.ts` 通过 `import type { DocumentContext } from './document'` 引入此接口（纯类型导入，运行时无循环）。

---

## Document.#docCtx 模式

`Document` 在构造时创建一个实现 `DocumentContext` 的普通对象，存入私有字段 `#docCtx`，并将其作为 `ctx` 传给 `super()`：

```ts
export class Document extends Node {
  #docCtx: DocumentContext;

  constructor(docHandle: DocumentHandle) {
    const docNode = docHandle.documentNode();
    const docCtx: DocumentContext = {
      _docHandle: docHandle,
      _nodes: new Set(),
      _handleNodeMap: new WeakMap(),
    };
    super(docCtx, docNode);   // Node._ctx = docCtx（永不为 null）
    this.#docCtx = docCtx;
    docCtx._handleNodeMap.set(docNode, this);
    docCtx._nodes.add(this);
  }
}
```

- 所有通过此 Document 创建/包装的子节点，`_ctx` 都指向同一个 `docCtx` 对象
- 子节点通过 `this._ctx._docHandle` 执行所有 Rust 侧操作，无需持有 `DocumentHandle` 本身

---

## JS 层节点生命周期与 Set 管理

```
Document.#docCtx._nodes: Set<Node>
  ├── 保存所有仍在 DOM 树中的 Node / Element / HTMLElement / ... 实例
  ├── 节点通过 appendChild / insertBefore 等方法进入树时加入此 Set
  └── 节点通过 removeChild 移出树时从此 Set 中删除
```

**为什么需要这个 Set：**

- 阻止 QuickJS GC 在节点仍在 DOM 树中时回收对应的 JS 对象
- 当节点移出树（`removeChild`），从 Set 中删除 → JS 对象可被 GC 回收 → `NodeHandle` 的 `Drop` 触发 → Rust 侧节点释放
- 保证同一 DOM 节点始终对应同一个 JS 实例（通过 `_handleNodeMap` 查找复用）

**生命周期流程：**

```
createElement / createTextNode
  → DocumentHandle 创建 Rust 节点，返回 NodeHandle
  → JS 层包装为 Node / Element 实例，存入 _handleNodeMap（暂不进 _nodes，处于游离状态）

appendChild / insertBefore
  → 节点进入 DOM 树
  → 加入 _nodes Set（防止 GC 回收）

removeChild
  → 节点移出 DOM 树
  → 从 _nodes Set 删除
  → 若无其他 JS 引用，GC 最终回收该 Node 实例
  → NodeHandle.drop() 触发 → Rust 侧节点释放
```

---

## 类层次与职责

```
EventTarget          ← 纯 JS 实现（event-target.ts）
  └── Node           ← 封装 NodeHandle + DocumentContext，实现 W3C Node 接口（node.ts）
        ├── Document ← 持有 #docCtx，实现 W3C Document 接口（document.ts）
        └── Element  ← 实现 W3C Element 接口（element.ts）
              └── HTMLElement  ← 实现 W3C HTMLElement 接口（html_element.ts）
```

| 类 | 持有 | 职责 |
|---|---|---|
| `Node` | `_ctx: DocumentContext`（非 null）、`_handle: NodeHandle` | nodeType / nodeValue / 树遍历 / 树变更 |
| `Document` | `#docCtx`（含 `_docHandle`、`_nodes`、`_handleNodeMap`） | createElement / createTextNode / getElementById / 生命周期管理 |
| `Element` | （继承 Node） | getAttribute / setAttribute / children / className / id |
| `HTMLElement` | （继承 Element） | style / dataset / 各具体标签属性 |

---

## 节点包装（wrapHandleWith）

定义在 `node.ts`，所有从 Rust 侧获取 NodeHandle 后的包装均走此函数：

```ts
function wrapHandleWith(ctx: DocumentContext, handle: NodeHandle | null): Node | null {
  if (!handle) return null;
  // 优先从 _handleNodeMap 复用已有实例
  const existing = ctx._handleNodeMap.get(handle);
  if (existing) return existing;
  // 根据 nodeType 选择正确的子类工厂
  const type = ctx._docHandle.nodeType(handle);
  const factory = nodeRegistry.get(type);
  const node = factory ? factory(ctx, handle) : new Node(ctx, handle);
  ctx._handleNodeMap.set(handle, node);
  return node;
}
```

新子类通过 `registerNodeType(nodeType, factory)` 注册工厂，`wrapHandleWith` 自动路由。

---

## 文件依赖关系

```
native (Rust)          → 导出 NodeHandle、DocumentHandle（运行时原语）
     ↓ import type
node.ts                → 导出 Node、wrapHandleWith、registerNodeType、NodeHandle(re-export)
     ↓ import
document.ts            → 定义 DocumentContext 接口、导出 Document
     ↓ import type (node.ts → document.ts，仅类型，运行时无循环)
element.ts / ...       → 扩展 Node
```

---

## 与 Web DOM 的对齐目标

- 属性和方法签名与 MDN / W3C 规范一致，不引入自定义前缀
- 错误时抛出标准 `DOMException`（`HierarchyRequestError`、`NotFoundError` 等）
- 实现顺序：`Node → Document → Element → HTMLElement → 具体标签类`，不跳过继承层级
