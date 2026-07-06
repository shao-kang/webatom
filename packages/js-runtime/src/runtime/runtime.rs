use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use rquickjs::{AsyncContext, AsyncRuntime, Module};

use crate::storage::{GlobalRoomStorage, RoomMemoryCenter};
use crate::extension::{ExtensionContext, ExtensionSet};
use crate::event_loop::EventLoop;

use super::JsRuntimeBuilder;
// ──────────────────────────────────────────────────────────────
// 1. JsContext：绝对隔离的单房间沙箱
// ──────────────────────────────────────────────────────────────

pub struct JsContext {
    // 每一个房间，都死死咬住属于自己的 QuickJS 异步上下文底座
    inner_ctx: AsyncContext,
}

impl JsContext {
    pub async fn new(
        runtime: &AsyncRuntime,
        extensions: &Arc<ExtensionSet>,
    ) -> rquickjs::Result<Self> {
        let inner_ctx = AsyncContext::full(runtime).await?;

        inner_ctx.with(|ctx| {
            let room_cancel_token = CancellationToken::new();
            
            let memory_center = RoomMemoryCenter {
                global: std::sync::Mutex::new(GlobalRoomStorage::new(room_cancel_token.clone())),
                private_manifest: std::sync::Mutex::new(HashMap::new()),   
            };
            ctx.store_userdata(memory_center);

            for ext in extensions.iter() {
                let ext_ctx = ExtensionContext {
                    ctx: &ctx,
                    cancel_token: room_cancel_token.clone(),
                };
                ext.install(&ext_ctx)?;
            }

            for ext in extensions.iter() {
                if let Some(glue_code) = ext.js_glue(&ctx) {
                    ctx.eval::<(), _>(glue_code)?;
                }
            }
            Ok::<(), rquickjs::Error>(())
        }).await?;

        Ok(Self { inner_ctx })
    }

    pub fn inner(&self) -> &AsyncContext {
        &self.inner_ctx
    }


    // ──────────────────────────────────────────────────────────────
    // ⚔️ 终极唯一高层执行接口：一刀切的万能 eval 大闸
    // ──────────────────────────────────────────────────────────────

    // 🔥 就地异步执行一段 JS 代码，并全自动将其返回值转换为 Rust 强类型
    // 泛型 R 表示预期的 Rust 返回类型（如 String, i32, bool, 或自定义结构体）
    // pub async fn eval<R, S>(&self, source: S) -> rquickjs::Result<R>
    // where
    //     R: for<'js> From<'js> + 'static,
    //     S: AsRef<str> + Send + 'static,
    // {
    //     // 引擎在幕后全自动剥离 rquickjs 复杂的 with 异步闭包和借用生存期检查
    //     self.inner_ctx.with(|ctx| {
    //         ctx.eval::<R, _>(source)
    //     }).await
    // }


    // ──────────────────────────────────────────────────────────────
    // 📦 终极核心接口：ESM 模块执行大闸 (ES Module Evaluation)
    // ──────────────────────────────────────────────────────────────

    // 🔥 异步运行一个标准的 ES 模块，并全自动冲刷内部可能存在的 Top-level await 异步拓扑树
    // `module_name`: 给模块起个虚拟路径名（例如 "foo.js" 或 "app/index.js"）
    // `source`: 模块的货真价实的纯文本源代码（包含 import / export）
    // pub async fn eval_module<S>(&self, module_name: S, source: S) -> rquickjs::Result<()>
    // where
    //     S: AsRef<str> + Send + 'static,
    // {
    //     let name_str = module_name.as_ref().to_string();
    //     let source_str = source.as_ref().to_string();

    //     self.inner_ctx.with(|ctx| {
    //         // 阶段 1：在当前房间中宣告并注册该 ES 模块
    //         let (module, promise) = Module::declare(
    //             ctx.clone(),
    //             name_str,
    //             source_str,
    //         )?;

    //         // 阶段 2：对模块进行编译并就地求值（Evaluation）
    //         // 此时如果模块内部有语法错误，会直接在这里爆破抛出
    //         let _ = module.eval()?;

    //         // 阶段 3：终极杀招！冲刷并驱动由于 Top-level await 引发的内部 Promise 链条
    //         // 这是 Deno / Node.js 底层最核心的打通逻辑
    //         while ctx.process_pending_promises() {
    //             // 疯狂空转冲刷，直到所有挂起的模块异步任务全部落地
    //         }

    //         Ok::<(), rquickjs::Error>(())
    //     }).await?;

    //     Ok(())
    // }
}

// ──────────────────────────────────────────────────────────────
// 2. JsRuntime：全局唯一的发动机底座
// ──────────────────────────────────────────────────────────────

pub struct JsRuntime {
    runtime: AsyncRuntime,
    extensions: Arc<ExtensionSet>,
    cancel_token: CancellationToken,
}

impl JsRuntime {
    /// 初始化整个运行时的发动机底座，并锁死当前进程/线程的全局只读插件集合
    pub fn new(extensions: ExtensionSet,cancel_token: CancellationToken) -> Self {
        let runtime = AsyncRuntime::new().unwrap();

        Self {
            runtime,
            extensions: Arc::new(extensions),
            cancel_token,
        }
    }
    pub fn builder() -> JsRuntimeBuilder { JsRuntimeBuilder::new() }

    /// 面向多租户：从当前的发动机底座上，随时随地快速派生一个全新、干净、彼此隔离的房间
    pub async fn create_context(&self) -> rquickjs::Result<JsContext> {
        // 将全局只读的 Arc<ExtensionSet> 传下去，房间初始化时无任何所有权复制开销
        JsContext::new(&self.runtime, &self.extensions).await
    }

    /// 获取暴力的绝对安全拉闸开关
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// 🚀 启动绝对纯净的单线程事件循环大闸（冬眠与收网核心）
    pub async fn run(self) -> rquickjs::Result<()> {
        let mut event_loop = EventLoop::new(self.runtime, self.cancel_token);
        
        #[cfg(debug_assertions)]
        println!("🚀 [JsRuntime] 引擎大闸已闭合，开始接管全局异步 IO 和事件流调配...");
        
        // 彻底告别 ctx 传参，面向整个底层 Runtime 的生命周期进行绝对审判！
        event_loop.run().await
    }
}