# webAtom

基于 [rquickjs](https://github.com/DelSkayn/rquickjs) 的完整 JavaScript 运行时，ESM 模块系统和事件循环，同时提供插件系统，DOM、Web API、 等通过插件机制接入运行时






## 模块系统

### 目标
实现符合 [WHATWG HTML 规范](https://html.spec.whatwg.org/multipage/webappapis.html#module-specifier-resolution) 的 ESM 模块加载，使前端代码无需修改即可在 QuickJS 中运行。

### 模块解析规则（Module Resolution）

| Specifier 类型 | 示例 | 处理方式 |
|---|---|---|
| 相对路径 | `./foo.js` | 基于当前模块路径解析 |
| 绝对路径 | `/abs/path.js` | 直接映射到文件系统 |
| Bare import | `react` | 查找 `node_modules` 或 import map |
| URL | `https://cdn/foo.js` | 网络加载（可缓存） |

### Import Map 支持
支持通过 `importmap` JSON 配置重映射 bare import：
```json
{
  "imports": {
    "react": "./vendor/react.js",
    "lodash-es": "https://cdn.skypack.dev/lodash-es"
  }
}
```

### 加载器管道（Loader Pipeline）
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

## 事件循环

### 设计目标
QuickJS 本身不内建 event loop，需要宿主（webAtom）驱动。目标是模拟浏览器的 [HTML Event Loop 规范](https://html.spec.whatwg.org/multipage/webappapis.html#event-loops)，保证 Promise / timer / I/O 的执行顺序与浏览器一致。

### 任务队列优先级

```
1. Microtask Queue  — Promise.then / queueMicrotask / async/await
2. MacroTask Queue  — setTimeout / setInterval / I/O 回调
3. Animation Frame  — requestAnimationFrame（渲染帧同步）
4. Idle Callback    — requestIdleCallback（低优先级后台任务）
```

每轮 tick 执行顺序：
```
取出一个 MacroTask 执行
  → 清空所有 Microtask（含 microtask 中新产生的）
  → 执行 rAF 回调（当前帧需要渲染时）
  → 等待下一个 MacroTask
```

### 驱动机制
- 使用平台原生线程（tokio / iOS RunLoop）驱动 loop
- QuickJS 通过 `JS_ExecutePendingJob` 清空 microtask
- timer 用最小堆管理到期时间，loop 每轮检查是否有到期项触发

### Timer 实现
```
setTimeout(fn, delay)    → 插入最小堆，key = now + delay
setInterval(fn, delay)   → 到期后重新入堆（self-rescheduling）
clearTimeout/Interval    → 标记取消，出堆时丢弃
```
精度对齐浏览器 [最小 4ms 限制](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)，嵌套超过 5 层时自动限流。

### 跨线程通信
JS 执行在单一 JS 线程，I/O（网络、文件）在 Rust/native 线程异步执行，完成后通过线程安全队列将回调 post 到 JS 线程，由 loop 下一轮取出执行。

---



## 参考规范

- [HTML Event Loop](https://html.spec.whatwg.org/multipage/webappapis.html#event-loops)
- [ES Module 规范](https://tc39.es/ecma262/#sec-modules)
- [WHATWG 模块解析算法](https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier)
- [QuickJS / rquickjs 文档](https://bellard.org/quickjs/quickjs.html)
- [Timer 规范](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
- [W3C DOM Living Standard](https://dom.spec.whatwg.org/) — DOM 接口的权威规范，JS 层实现以此为准
- [W3C HTML Living Standard](https://html.spec.whatwg.org/multipage/dom.html) — HTMLElement 及各具体标签的接口定义
- [MDN DOM API 文档](https://developer.mozilla.org/zh-CN/docs/Web/API/Document_Object_Model) — 规范的可读版本，适合快速查阅接口细节



## 参考实现

- [LLRT](https://github.com/awslabs/llrt) — AWS 基于 QuickJS 的轻量 JS 运行时，重点参考其 ESM 加载器、timer 实现及 Rust 侧 event loop 驱动方式
- [Deno](https://github.com/denoland/deno) — 基于 V8 + Tokio 的现代 JS 运行时，重点参考其 Web API 兼容层设计、模块图构建、以及 I/O 与 JS 线程的跨线程通信模式


