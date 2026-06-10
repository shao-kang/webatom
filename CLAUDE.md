# webAtom



## 项目目标
构建一个结构化，轻量化的web 跨端（移动端，pc端）运行时， 兼容常用web api, 如dom, fetch, timer, esm import 等。 使用 quickjs 作为js 运行时， blitz 作为 页面渲染部分。 适合使用现代前端架构搭建跨端应用



## 模块结构

| 包 | 路径 | 说明 |
|---|---|---|
| js-runtime | `packages/js-runtime` | 基于 rquickjs 的 JS 运行时，提供 ESM 模块系统、事件循环和插件接入机制 |

## 参考资料

### 规范

- [W3C DOM Living Standard](https://dom.spec.whatwg.org/) — DOM 接口的权威规范，JS 层实现以此为准
- [W3C HTML Living Standard](https://html.spec.whatwg.org/multipage/dom.html) — HTMLElement 及各具体标签的接口定义
- [MDN DOM API 文档](https://developer.mozilla.org/zh-CN/docs/Web/API/Document_Object_Model) — 规范的可读版本，适合快速查阅接口细节


### 代码规范

所有 Rust 代码使用 **Edition 2024** 规范，参考：https://doc.rust-lang.org/edition-guide/rust-2024/index.html

#### 关键规则

**unsafe 使用**
- `unsafe fn` 内部的操作必须显式加 `unsafe { }` 块，不得依赖函数级隐式 unsafe（`unsafe_op_in_unsafe_fn` 默认 deny）
- `extern` 块声明必须写为 `unsafe extern "C" { ... }`

**生命周期捕获（RPIT）**
- `-> impl Trait` 默认捕获所有输入生命周期，需要精确控制时使用 `use<>` 语法：
  ```rust
  fn foo<'a, 'b>(x: &'a str) -> impl Sized + use<'a> { ... }
  ```

**临时变量作用域**
- `if let` 和尾表达式中临时值的 drop 顺序与 Edition 2021 有差异，涉及锁/资源释放时需注意

**保留关键字**
- `gen` 是保留关键字，不得用作标识符

**模式匹配**
- `match` 中裸 `&` 模式的行为有调整，优先使用 `ref` 或显式解引用

**模块文件命名**
- 使用目录同名入口文件，不使用 `mod.rs`：
  ```
  src/
  ├── module.rs        ✓  (代替 src/module/mod.rs)
  └── module/
      ├── resolver.rs
      └── loader.rs
  ```
- `mod.rs` 仅在无子模块的单文件场景保留，一般应避免

#### 工具链要求
```toml
# Cargo.toml
edition = "2024"
```


