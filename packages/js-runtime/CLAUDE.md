# webAtom JS Runtime

基于 [rquickjs](https://github.com/DelSkayn/rquickjs) 的完整 JavaScript 运行时，实现 ESM 模块系统和事件循环，通过 Extension 机制接入 Web API（console、timer、fetch、DOM 等）。

---

## 模块结构

| 模块 | 路径 | 说明 |
|---|---|---|
| `event_loop` | `src/event_loop/` | 事件循环主循环、microtask checkpoint、rAF/idle 调度、RenderScheduler |
| `extension` | `src/extension/` | Extension trait、HostBridge 三层接口、各 Web API 实现 |
| `runtime` | `src/runtime/` | JsRuntime 顶层组装、RuntimeBuilder、生命周期管理 |

详细设计见各模块 CLAUDE.md：[event_loop](src/event_loop/CLAUDE.md) · [extension](src/extension/CLAUDE.md) · [runtime](src/runtime/CLAUDE.md)

---

## 整体架构

```
┌──────────────────────────────┐
│          JsRuntime           │
│  eval() / run() / handle()   │
│          runtime/            │
└────────────┬─────────────────┘
             │ 组装
    ┌─────────┴──────────┐
    ▼                    ▼
┌──────────┐       ┌───────────┐
│EventLoop │◄──────│Extensions │
│(drain /  │  Host │(push only)│
│ execute) │Bridge └───────────┘
└──────────┘
      ▼
QuickJS AsyncContext (rquickjs)
```

**单向依赖：**

```
runtime    → event_loop + extension
extension  → event_loop（仅通过 HostBridge push，不直接访问内部队列）
event_loop → QuickJS（不依赖 extension）
```

**Phase 0 核心 Web API：**

| Extension | 全局 API |
|---|---|
| `ConsoleExtension` | `console.log/warn/error/debug/info` |
| `PerformanceExtension` | `performance.now()` |
| `MicrotaskExtension` | `queueMicrotask()` |
| `TimerExtension` | `setTimeout/clearTimeout/setInterval/clearInterval` |
| `AnimationFrameExtension` | `requestAnimationFrame/cancelAnimationFrame` |
| `IdleCallbackExtension` | `requestIdleCallback/cancelIdleCallback` |

---

## 模块系统

### 目标

实现符合 [WHATWG HTML 规范](https://html.spec.whatwg.org/multipage/webappapis.html#module-specifier-resolution) 的 ESM 模块加载，使前端代码无需修改即可在 QuickJS 中运行。

### 模块解析规则

| Specifier 类型 | 示例 | 处理方式 |
|---|---|---|
| 相对路径 | `./foo.js` | 基于当前模块路径解析 |
| 绝对路径 | `/abs/path.js` | 直接映射到文件系统 |
| Bare import | `react` | 查找 `node_modules` 或 import map |
| URL | `https://cdn/foo.js` | 网络加载（可缓存） |

### Import Map 支持

```json
{
  "imports": {
    "react": "./vendor/react.js",
    "lodash-es": "https://cdn.skypack.dev/lodash-es"
  }
}
```

### 加载器管道

```
specifier
  → normalize（路径规范化）
  → resolve（查找实际资源位置）
  → fetch（读取源码：本地 FS / 网络 / 缓存）
  → transform（可选：TS/JSX 转译）
  → compile（QuickJS 编译为字节码）
  → evaluate（执行模块，缓存导出）
```

### 循环依赖处理

遵循 ES 规范 [Module Linking](https://tc39.es/ecma262/#sec-moduledeclarationlinking)：先构建完整模块图，再按拓扑顺序 evaluate，循环依赖模块的导出在 evaluate 前为 TDZ（临时死区）。

### 动态 import()

支持 `import()` 异步加载，返回 Promise，resolve 后进入 microtask 队列。

---

## 快速上手

```rust
let mut rt = JsRuntime::builder()
    .with_default_extensions()
    .build()
    .await?;

rt.eval_module("index.js", source).await?;
rt.run().await?;
```

---

## 参考规范

- [HTML Event Loop](https://html.spec.whatwg.org/multipage/webappapis.html#event-loops)
- [ES Module 规范](https://tc39.es/ecma262/#sec-modules)
- [WHATWG 模块解析算法](https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier)
- [Timer 规范](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
- [W3C DOM Living Standard](https://dom.spec.whatwg.org/)
- [W3C HTML Living Standard](https://html.spec.whatwg.org/multipage/dom.html)
- [MDN DOM API 文档](https://developer.mozilla.org/zh-CN/docs/Web/API/Document_Object_Model)
- [QuickJS / rquickjs 文档](https://bellard.org/quickjs/quickjs.html)

## 参考实现

- [LLRT](https://github.com/awslabs/llrt) — AWS 基于 QuickJS 的轻量 JS 运行时，重点参考其 ESM 加载器、timer 实现及 Rust 侧 event loop 驱动方式
- [Deno](https://github.com/denoland/deno) — 基于 V8 + Tokio 的现代 JS 运行时，重点参考其 Web API 兼容层设计、模块图构建及跨线程通信模式
