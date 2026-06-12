# dom
web dom 实现

## 核心架构：Rust 持有数据，JS 持有 Handle

**这是整个代码库最核心的不变量。**

- 所有 DOM 节点数据（标签名、属性、文本内容、父子关系等）存储在 Rust 管理的内存中（如 Arena 或 `SlotMap`）。
- JS 对象是轻量 Handle，只包含一个数字 ID（如 `NodeId(u32)`），用于索引 Rust 侧的存储。
- DOM 数据不会被复制到 JS 堆中。JS 从不"拥有"节点，只通过 ID 借用引用。
- JS Handle 被 GC 回收时，对应的 Rust 节点**不会**自动释放——节点生命周期由 Rust 侧控制（通常绑定到 Document 的 Arena）。

### 为什么这样设计

- 避免 Rust 中自引用树结构带来的 unsafe 问题。
- Rust 侧可以直接做序列化、Diff、SSR，无需跨语言同步。
- QuickJS GC 压力小——JS 对象极轻量。
- Rust 可以自由重组树结构而不会使 JS Handle 失效（只要节点存活，ID 始终有效）。

## 分层实现策略：Rust 提供原语，JS 实现 DOM 接口

**Rust 只暴露一组最小化的底层原语，完整的 W3C DOM API 由 JS 层在这些原语之上实现。**

### Rust 层职责（薄 FFI 边界）

Rust 只实现以下类别的基础操作，不追求 W3C 完整性：

| 类别 | 示例方法 |
|------|---------|
| 节点创建 | `__web_atom_dom.createElement(tag)`, `__web_atom_dom.createTextNode(data)` |
| 树操作 | `__web_atom_dom.appendChild(parentId, childId)`, `__web_atom_dom.removeChild(parentId, childId)`, `__web_atom_dom.insertBefore(parentId, newId, refId)` |
| 节点查询 | `__web_atom_dom.parentNode(id)`, `__web_atom_dom.firstChild(id)`, `__web_atom_dom.nextSibling(id)`, `__web_atom_dom.nodeType(id)` |
| 属性读写 | `__web_atom_dom.getAttribute(id, name)`, `__web_atom_dom.setAttribute(id, name, value)`, `__web_atom_dom.removeAttribute(id, name)` |
| 文本内容 | `__web_atom_dom.nodeValue(id)`, `__web_atom_dom.setNodeValue(id, value)`, `__web_atom_dom.tagName(id)` |
| 文档操作 | `__web_atom_dom.createDocument()`, `__web_atom_dom.documentElement(docId)` |

这些方法以 `__web_atom_dom.` 前缀命名，表明它们是内部原语，不直接暴露给用户代码。

### JS 层职责（完整 DOM API）

所有 W3C DOM 接口（`Node`、`Element`、`HTMLElement`、`Document`、`NodeList`、`querySelector` 等）用 **TypeScript** 编写，经 Rolldown 编译为单文件 JS，在 `Context` 创建时注入：

```
js/
  src/
    index.ts        — 入口，汇总所有 DOM 类并挂载到全局
    node.ts         — Node 基类（封装 Rust 原语）
    element.ts      — Element 类（继承 Node）
    html_element.ts — HTMLElement 及各具体标签类
    document.ts     — Document 类（querySelector、createElement 等高级 API）
    event.ts        — EventTarget / Event（纯 TS 实现）
    node_list.ts    — NodeList / HTMLCollection
  dist/
    dom.js          — Rolldown 打包产物，Rust 侧 include_str! 嵌入此文件
  rolldown.config.js
  tsconfig.json
  package.json
```

### 为什么在 JS 侧实现 DOM 接口

- W3C DOM 继承链深（Node → Element → HTMLElement → HTMLDivElement……），用 Rust + rquickjs class 系统实现极为繁琐。
- JS 原型链天然适合表达 DOM 继承关系，修改和扩展成本极低。
- 新增标签类型或修复规范兼容问题只需改 JS，无需重新编译 Rust。
- Rust/JS 边界保持最小，降低 FFI bug 的概率。

## 关键概念

### NodeId / Handle 模式

```rust
// Rust 侧：不透明的数字 ID
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

// NodeHandle：rquickjs 注册的原生 JS class
// 持有 NodeId 和指向 Document 的共享引用
// JS GC 回收时，Drop 实现直接递减对应节点的 handle_num
pub struct NodeHandle {
    id: NodeId,
    doc: Rc<RefCell<Document>>,
}

// JS 调用 node.appendChild(child) 时，rquickjs 将两个 NodeId 传入 Rust
// 实际的树变更完全由 Rust 完成
```

### Document / Arena

```rust
struct Document {
    arena: SlotMap<NodeId, Node>,
    root: NodeId,
    detached_nodes: Vec<NodeId>,  // 仅存脱离树的子树根节点
}

struct Node {
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    data: NodeData,
    handle_num: u32,  // 当前存活的 JS NodeHandle 数量
}
```

`Document` 是所有 DOM 状态的根，通常存储在 rquickjs 的 `Context` userdata 中，以 `Rc<RefCell<Document>>` 共享给每个 `NodeHandle`。

### 节点生命周期与内存释放

**引用计数规则**

- 每次创建 `NodeHandle`（`__web_atom_dom.createElement`、`__web_atom_dom.querySelector` 等返回节点时）：`node.handle_num += 1`。
- `NodeHandle` 被 JS GC 回收时，`Drop` 直接执行 `doc.borrow_mut().arena[id].handle_num -= 1`。单线程下 GC 不会打断正在执行的 Rust 代码，`borrow_mut()` 始终安全。

**脱离树的节点回收**

- `__web_atom_dom.removeChild(parent, child)` 执行后，若 `child` 尚无 detached 父节点（`node.parent.is_none()`），将 `child` 加入 `detached_nodes`（只存子树根，O(1)）。
- 事件循环每轮 tick 结束时（安全点），扫描 `detached_nodes`：
  ```
  for root in detached_nodes:
      if subtree_all_handles_zero(root):   // 递归检查整棵子树
          free_subtree(root)               // 从 arena 中移除
          remove from detached_nodes
      // 否则留到下一轮
  ```
- 子树内任意节点的 `handle_num > 0`，整棵子树保留，避免 JS 端悬空引用。

**Document 销毁**

`Document` 销毁时 Arena 整体释放，无需逐节点回收。树内节点（未进入 `detached_nodes`）的生命周期完全绑定到 `Document`。

### JS class 注册

Rust 侧只通过 rquickjs 的 `Class` trait 注册一个原生类：`NodeHandle`（持有 `NodeId` 和 `Rc<RefCell<Document>>`，不暴露任何 DOM 方法）。

TS 层的 `Node`、`Element`、`HTMLElement`、`Document` 等均是对 `NodeHandle` 的 JS 包装，通过 `__web_atom_dom.*` 原语函数实现完整 DOM API，**不是** Rust 注册的 class。

```
Rust Class trait → NodeHandle（NodeId + Rc<RefCell<Document>>）
JS 包装层       → Node / Element / HTMLElement / Document …
```

`NodeHandle` 仅有一个 JS 属性 `_id`（数字），所有 DOM 方法均在 TS 层通过调用全局 `__web_atom_dom.*` 函数完成。

```

TS 源码经 Rolldown 编译为 `crates/dom/js/dist/index.js`（`iife` 格式），Rust 侧通过 `include_str!` 嵌入二进制，Context 初始化时执行注入。`dist/index.js` 提交到仓库，使用方无需安装 Node 即可构建。

新增 Web 特性：在 `crates/web/src/` 添加新模块，并在 `lib.rs` 的 `init()` 中注册即可。


## 设计规则

1. **禁止在 JS 对象中存储 DOM 数据** — JS 对象只允许存 `NodeId` 和反向引用。
2. **所有树变更必须通过 `Document`** — 禁止直接操作指针。
3. **NodeId 稳定不变** — 节点一旦分配，其 ID 在显式释放前不得改变。
4. **无效 ID 抛出 JS 异常** — 若 JS Handle 引用了已释放的节点，抛出 `DOMException`，禁止 panic。各原语对应的异常类型如下：

   | 场景 | DOMException 类型 |
   |------|-----------------|
   | 使用已释放节点的 ID | `InvalidStateError` |
   | `__web_atom_dom.appendChild` 形成循环引用 | `HierarchyRequestError` |
   | `__web_atom_dom.insertBefore` 的 `refId` 不是 `parentId` 的子节点 | `NotFoundError` |
   | 向文本节点插入子节点 | `HierarchyRequestError` |
   | 其他 ID 不存在 | `NotFoundError` |
5. **unsafe 仅限 rquickjs FFI 边界** — 将 unsafe 集中在一处并加以注释说明。
6. **按 W3C 继承链增量实现** — 按 `Node → Element → HTMLElement → ...` 顺序实现，不跳过基础接口。
7. **Rust 只做原语，不做 API** — 凡是可以在 JS 层组合原语实现的功能，不在 Rust 侧重复实现（如 `querySelector`、`innerHTML`、`classList` 等均属 JS 层）。
8. **`__web_atom_dom.` 前缀方法为内部原语** — 以 `__web_atom_dom.` 开头的方法仅供 JS 层内部调用，不属于对外 DOM API，用户代码不应直接使用。

### 类似实现（可借鉴）

- [deno/deno_dom](https://github.com/nicolo-ribaudo/deno_dom) — Deno 的轻量 DOM 实现，纯 Rust，可参考节点结构设计
- [denoland/deno](https://github.com/denoland/deno) — Deno 核心运行时，包含 URL、URLSearchParams、TextEncoder/TextDecoder、console、fetch、setTimeout 等 JS 基础 API 的规范参照实现；实现 `web/` crate 中各类 Web API 时以此为主要参考
- [jsdom](https://github.com/jsdom/jsdom) — Node.js 下完整 DOM 实现，JS 层参考
- [servo/html5ever](https://github.com/servo/html5ever) — Rust HTML 解析器，后续集成 HTML 解析时参考