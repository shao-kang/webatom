# dom-extension JS 层设计

## 核心原则

- JS 层的 `Node`、`Element`、`HTMLElement`、`Document` 等类尽可能贴近 [W3C DOM Living Standard](https://dom.spec.whatwg.org/)，让用户代码无需适配即可运行。
- 所有 DOM 数据存储在 Rust 侧，JS 层不缓存任何节点数据，只持有不透明句柄（Handle）。

---

## 两个底层原语

通过 `import { DocumentHandle, NodeHandle } from 'webatom_ext_native:dom'` 引入。

### NodeHandle（节点句柄）

- 对应 Rust 侧一个具体节点，完全不透明
- **不暴露任何属性或方法**，仅作为 token 传入 `DocumentHandle` 的方法以定位节点
- 由 QuickJS GC 管理生命周期

### DocumentHandle（文档门面）

- 持有 Rust 侧 `Document`（Arena/SlotMap）的引用
- 所有节点操作（创建、查询、树变更、属性读写）均为其实例方法，接受 `NodeHandle` 参数定位节点
- 每个节点在 `DocumentHandle` 内部有唯一对应的 `NodeHandle`（`get_or_create` 缓存）

---

## JS 层节点生命周期与 Set 管理

```
Document._nodes: Set<Node>
  ├── 保存所有仍在 DOM 树中的 Node / Element / HTMLElement / ... 实例
  ├── 节点通过 appendChild / insertBefore 等方法进入树时加入此 Set
  └── 节点通过 removeChild 移出树时从此 Set 中删除
```

**为什么需要这个 Set：**

- 阻止 QuickJS GC 在节点仍在 DOM 树中时回收对应的 JS 对象
- 当节点移出树（`removeChild`），从 Set 中删除 → JS 对象可被 GC 回收 → `NodeHandle` 的 `Drop` 触发 → Rust 侧节点释放
- 保证同一 DOM 节点始终对应同一个 JS 实例（通过 `DocumentHandle.get_or_create` + Set 配合）

**生命周期流程：**

```
createElement / createTextNode
  → DocumentHandle 创建 Rust 节点，返回 NodeHandle
  → JS 层包装为 Node / Element 实例（暂不进 Set，处于游离状态）

appendChild / insertBefore
  → 节点进入 DOM 树
  → 加入 Document._nodes Set（防止 GC 回收）

removeChild
  → 节点移出 DOM 树
  → 从 Document._nodes Set 删除
  → 若无其他 JS 引用，GC 最终回收该 Node 实例
  → NodeHandle.drop() 触发 → Rust 侧节点释放
```

---

## 类层次与职责

```
EventTarget          ← 纯 JS 实现（event-target.ts）
  └── Node           ← 封装 NodeHandle，实现 W3C Node 接口（node.ts）
        ├── Document ← 封装 DocumentHandle，实现 W3C Document 接口（document.ts）
        │              持有 Document._nodes Set
        └── Element  ← 实现 W3C Element 接口（element.ts）
              └── HTMLElement  ← 实现 W3C HTMLElement 接口（html_element.ts）
```

| 类 | 持有 | 职责 |
|---|---|---|
| `Node` | `NodeHandle` | nodeType / nodeValue / 树遍历 / 树变更 |
| `Document` | `DocumentHandle` + `_nodes: Set` + `_handleNodeMap: WeakMap` | createElement / createTextNode / getElementById / `_nodes` 生命周期管理 |
| `Element` | （继承 Node） | getAttribute / setAttribute / children / className / id |
| `HTMLElement` | （继承 Element） | style / dataset / 各具体标签属性 |

---

## 节点包装规则

- 每次从 Rust 侧拿到 `NodeHandle`，通过 `nodeRegistry`（按 `nodeType` 注册的工厂 Map）实例化为正确的 JS 子类
- 同一底层节点始终返回同一 JS 实例（`DocumentHandle.get_or_create` 在 Rust 侧保证；JS 层 `_nodes` Set 持有强引用）

```js
// 示例（node.ts 中的 wrapHandle）
function wrapHandle(handle) {
  const type = doc.nodeType(handle);
  const factory = nodeRegistry.get(type);
  return factory ? factory(handle) : new Node(handle);
}
```

---

## 与 Web DOM 的对齐目标

- 属性和方法签名与 MDN / W3C 规范一致，不引入自定义前缀
- 错误时抛出标准 `DOMException`（`HierarchyRequestError`、`NotFoundError` 等）
- 实现顺序：`Node → Element → HTMLElement → 具体标签类`，不跳过继承层级
