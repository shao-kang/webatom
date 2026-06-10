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
| 节点创建 | `__createElement(tag)`, `__createTextNode(data)` |
| 树操作 | `__appendChild(parentId, childId)`, `__removeChild(parentId, childId)`, `__insertBefore(parentId, newId, refId)` |
| 节点查询 | `__parentNode(id)`, `__firstChild(id)`, `__nextSibling(id)`, `__nodeType(id)` |
| 属性读写 | `__getAttribute(id, name)`, `__setAttribute(id, name, value)`, `__removeAttribute(id, name)` |
| 文本内容 | `__nodeValue(id)`, `__setNodeValue(id, value)`, `__tagName(id)` |
| 文档操作 | `__createDocument()`, `__documentElement(docId)` |

这些方法以 `__` 前缀命名，表明它们是内部原语，不直接暴露给用户代码。

### JS 层职责（完整 DOM API）

所有 W3C DOM 接口（`Node`、`Element`、`HTMLElement`、`Document`、`NodeList`、`querySelector` 等）用 **TypeScript** 编写，经 Rolldown 编译为单文件 JS，在 `Context` 创建时注入：

```
src/js/
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

// JS 侧：一个 JS class，唯一字段是 NodeId
// JS 调用 node.appendChild(child) 时，rquickjs 将两个 NodeId 传入 Rust
// 实际的树变更完全由 Rust 完成
```

### Document / Arena

`Document` 是所有 DOM 状态的根。它持有节点 Arena，生命周期必须长于所有指向其节点的 JS Handle。通常存储在 rquickjs 的 `Runtime` 或 `Context` 的 userdata 中。

### JS class 注册

DOM 类（Node、Element、Document 等）通过 rquickjs 的 `Class` trait 注册。每个 class：
1. 只持有一个 `NodeId`（以及必要时指向 `Document` 的引用/指针）。
2. 实现的方法通过存储的 ID 委托给 `Document` 执行。

## Cargo Workspace 结构

项目采用 Cargo workspace，按**依赖边界**拆分为四个 crate：

```
rquickjs-dom/
  Cargo.toml               — workspace 根，列出所有成员
  crates/
    dom-core/              — 纯 Rust DOM 数据层，不依赖 rquickjs
      Cargo.toml           — name = "rquickjs-dom-core"
      src/
        lib.rs             — mod 声明 + pub use 汇总（原 src/document.rs）
        document.rs        — Document：持有 Slab<Node>，实现所有树操作
        node.rs            — Node { id, parent, children, data }、NodeData 枚举
        element.rs         — ElementData { name: QualName, attrs: Attributes }、TextNodeData
        attributes.rs      — Attribute { name: QualName, value: String }、Attributes
        html_parser.rs     — html5ever TreeSink，parse_html() → Document
    dom/                   — rquickjs 绑定层 + TS/JS 层（对外公开名称 rquickjs-dom）
      Cargo.toml           — name = "rquickjs-dom"，依赖 rquickjs-dom-core
      src/
        lib.rs             — pub init(ctx)、init_with_html(ctx, html)
        bindings.rs        — bindings 模块声明（Rust 2018 风格）
        bindings/
          console.rs       — console.log / info / warn / error（tracing）
          node_handle.rs   — NodeHandle JS class（仅持有 usize node id）
          primitives.rs    — 将 Rust 底层原语（__createXxx 等）注册为全局函数
        js.rs              — include_str! 嵌入 js/dist/index.js，Context 初始化注入
      js/                  — TS 源码层：在 Rust 原语之上实现完整 W3C DOM API
        src/
          primitives.d.ts  — Rust 原语的 TypeScript 类型声明（全局函数）
          event_target.ts  — EventTarget / Event / CustomEvent / MouseEvent / KeyboardEvent
          node.ts          — Node 基类（封装 Rust 原语）
          element.ts       — Element 类（继承 Node）、DOMTokenList、CSSStyleDeclaration
          document.ts      — document 对象（createElement、querySelector、body、head 等）
          window.ts        — window 对象（location、navigator、setTimeout 等）
          index.ts         — 入口，汇总所有类并挂载到 globalThis
        dist/
          index.js         — Rolldown 打包产物（iife 格式，提交到仓库）
        rolldown.config.js
        tsconfig.json
        package.json
      tests/
        dom.rs             — 通过 __* 原语直接测试 Document 层
        wpt.rs             — WPT 风格测试，测试高层 JS DOM API
        wpt/               — WPT 测试用例 + testharness.js
        example/           — 示例 HTML 文件
    web/                   — Web 平台小特性（fetch、event-loop、定时器等）
      Cargo.toml           — name = "rquickjs-web"（避免与 rquickjs 内部 rquickjs-core 冲突）
      src/
        lib.rs             — pub fn init(ctx) → Result<Rc<RefCell<EventLoop>>>，注册所有 web 特性并返回事件循环句柄
        fetch.rs           — js_fetch 函数（ureq 阻塞 HTTP，返回已解决 Promise）
        response.rs        — FetchResponse rquickjs class（status/ok/text()/json()）
        event_loop.rs      — EventLoop 定时器状态 + pub fn run(rt, ctx, el) 驱动微任务队列和定时器
    repl/                  — 交互式 REPL 二进制
      Cargo.toml           — name = "rquickjs-dom-repl"
      src/
        main.rs            — REPL 入口，支持加载 HTML 文件
```

TS 源码经 Rolldown 编译为 `crates/dom/js/dist/index.js`（`iife` 格式），Rust 侧通过 `include_str!` 嵌入二进制，Context 初始化时执行注入。`dist/index.js` 提交到仓库，使用方无需安装 Node 即可构建。

新增 Web 特性：在 `crates/web/src/` 添加新模块，并在 `lib.rs` 的 `init()` 中注册即可。


## 设计规则

1. **禁止在 JS 对象中存储 DOM 数据** — JS 对象只允许存 `NodeId` 和反向引用。
2. **所有树变更必须通过 `Document`** — 禁止直接操作指针。
3. **NodeId 稳定不变** — 节点一旦分配，其 ID 在显式释放前不得改变。
4. **无效 ID 抛出 JS 异常** — 若 JS Handle 引用了已释放的节点，抛出 `DOMException`，禁止 panic。
5. **unsafe 仅限 rquickjs FFI 边界** — 将 unsafe 集中在一处并加以注释说明。
6. **按 W3C 继承链增量实现** — 按 `Node → Element → HTMLElement → ...` 顺序实现，不跳过基础接口。
7. **Rust 只做原语，不做 API** — 凡是可以在 JS 层组合原语实现的功能，不在 Rust 侧重复实现（如 `querySelector`、`innerHTML`、`classList` 等均属 JS 层）。
8. **`__` 前缀方法为内部原语** — 以 `__` 开头的方法仅供 JS 层内部调用，不属于对外 DOM API，用户代码不应直接使用。

### 类似实现（可借鉴）

- [deno/deno_dom](https://github.com/nicolo-ribaudo/deno_dom) — Deno 的轻量 DOM 实现，纯 Rust，可参考节点结构设计
- [denoland/deno](https://github.com/denoland/deno) — Deno 核心运行时，包含 URL、URLSearchParams、TextEncoder/TextDecoder、console、fetch、setTimeout 等 JS 基础 API 的规范参照实现；实现 `web/` crate 中各类 Web API 时以此为主要参考
- [jsdom](https://github.com/jsdom/jsdom) — Node.js 下完整 DOM 实现，JS 层参考
- [servo/html5ever](https://github.com/servo/html5ever) — Rust HTML 解析器，后续集成 HTML 解析时参考