use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use rquickjs::{AsyncContext, AsyncRuntime, Module, FromJs};

use crate::storage::{GlobalRoomStorage, RoomMemoryCenter};
use crate::extension::{ExtensionContext, ExtensionSet, Extension};
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
                let ext_ctx = ExtensionContext::new(
                    &ctx,
                    &inner_ctx,
                    room_cancel_token.clone(),
                    ext.name().to_string(),
                );
                ext.install(&ext_ctx)?;
            }

            for ext in extensions.iter() {
                if let Some(glue_code) = ext.get_js_source(&ctx) {
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

    /// 🔥 就地异步执行一段 JS 代码，并全自动将其返回值转换为 Rust 强类型
    /// 泛型 R 表示预期的 Rust 返回类型（如 String, i32, bool, 或自定义结构体）
    pub async fn eval<R, S>(&self, source: S) -> rquickjs::Result<R>
    where
        R: for<'js> FromJs<'js> + 'static,
        S: Into<Vec<u8>> + Send + 'static,
    {
        // 引擎在幕后全自动剥离 rquickjs 复杂的 with 异步闭包和借用生存期检查
        self.inner_ctx.with(|ctx| {
            ctx.eval::<R, _>(source)
        }).await
    }


    // ──────────────────────────────────────────────────────────────
    // 📦 终极核心接口：ESM 模块执行大闸 (ES Module Evaluation)
    // ──────────────────────────────────────────────────────────────

    // 🔥 异步运行一个标准的 ES 模块，并全自动冲刷内部可能存在的 Top-level await 异步拓扑树
    // `module_name`: 给模块起个虚拟路径名（例如 "foo.js" 或 "app/index.js"）
    // `source`: 模块的货真价实的纯文本源代码（包含 import / export）
    pub async fn eval_module<S>(&self, module_name: S, source: S) -> rquickjs::Result<()>
    where
        S: AsRef<str> + Send + 'static,
    {
        let name_str = module_name.as_ref().to_string();
        let source_str = source.as_ref().to_string();

        self.inner_ctx.with(|ctx| {
            // 🎯 修复核心 1：Module::declare 吐出的是纯粹的 Module 实体，不是元组！
            // 这里我们传入 ctx 的引用即可，不需要显式 ctx.clone() 从而避免引用计数无谓开销
            let module = rquickjs::Module::declare(
                ctx,
                name_str,
                source_str,
            )?;

            // 🎯 修复核心 2：在最新异步架构中，eval() 或者是 evaluate() 才会返回一个
            // 标志着该模块是否加载、执行完毕的 Promise 或者是特定状态标识。
            // 我们直接将其就地求值：
            let _eval_result = module.eval()?;


            Ok::<(), rquickjs::Error>(())
        }).await?;

        Ok(())
    }
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



    // 🚀 终极合闸：无需外来传参，直接从 self.extensions 的 Arc 宇宙中汲取插件
    // pub async fn bootstrap_extensions(&self) -> rquickjs::Result<()> {
        
    //     // -----------------------------------------------------------------
    //     // 🔒 第一道防线：全宇宙模块名主权审查（防止多个插件申请相同模块）
    //     // -----------------------------------------------------------------
    //     let mut global_module_registry: HashMap<String, &'static str> = HashMap::new();
        
    //     // Directly look into self.extensions via Arc borrowing
    //     for ext in self.extensions.iter() {
    //         let ext_name = ext.name();
    //         for specifier in ext.module_specifiers() {
    //             let specifier_str = specifier.to_string();
                
    //             if let Some(existing_owner) = global_module_registry.get(&specifier_str) {
    //                 panic!(
    //                     "\n❌ [Runtime Fatal Error]: 发生模块说明符命名撞车冲突！\n\
    //                     👉 冲突模块: `{}`\n\
    //                     🚨 冲突详情: 插件 `{}` 企图注册该模块，但该模块的主权早已被插件 `{}` 占领！\n",
    //                     specifier_str, ext_name, existing_owner
    //                 );
    //             }
    //             global_module_registry.insert(specifier_str, ext_name);
    //         }
    //     }

    //     // -----------------------------------------------------------------
    //     // 🔗 第二道防线：基于引用的拓扑排序（解决 Arc 无法 Move 所有权的问题）
    //     // -----------------------------------------------------------------
    //     let sorted_extensions: Vec<&dyn Extension> = self.topological_sort_extensions_ref();

    //     // -----------------------------------------------------------------
    //     // ⚡ 核心阶段：按照拓扑安全顺序，依次执行插件的 install()
    //     // -----------------------------------------------------------------
    //     // 注意：rquickjs 异步上下文需要通过 runtime 来获取当前线程的 context
    //     // 假设这里可以通过 self.runtime.with 或类似的机制拿到 ctx
    //     self.runtime.with(|ctx| {
    //         for ext in &sorted_extensions {
    //             let ext_ctx = ExtensionContext {
    //                 ctx: &ctx,
    //                 async_context: &self.runtime, // 对应你结构体里的 AsyncRuntime/AsyncContext
    //                 cancel_token: self.cancel_token.clone(), // 🎯 完美利用 self 的熔断 Token
    //                 current_plugin_name: ext.name(),
    //             };
                
    //             ext.install(&ext_ctx)?;
    //         }
    //         Ok::<(), rquickjs::Error>(())
    //     }).await?;

    //     // -----------------------------------------------------------------
    //     // 🕵️‍♂️ 第三道防线：孤儿模块检查（货不对板大清查）
    //     // -----------------------------------------------------------------
    //     self.runtime.with(|ctx| {
    //         for ext in &sorted_extensions {
    //             for specifier in ext.module_specifiers() {
    //                 let has_js_source = ext.get_js_source(specifier).is_some();
    //                 let is_rust_declared = rquickjs::Module::get(ctx, *specifier).is_ok();

    //                 if !has_js_source && !is_rust_declared {
    //                     panic!(
    //                         "\n❌ [Runtime Fatal Error]: 插件 `{}` 的模块 `{}` 发生了构建方式丢失！\n",
    //                         ext.name(), specifier
    //                     );
    //                 }

    //                 if has_js_source && is_rust_declared {
    //                     panic!(
    //                         "\n❌ [Runtime Fatal Error]: 插件 `{}` 的模块 `{}` 发生了构建冲突！\n",
    //                         ext.name(), specifier
    //                     );
    //                 }
    //             }
    //         }
    //         Ok::<(), rquickjs::Error>(())
    //     }).await?;

    //     println!("🎉 [Runtime System] 宿主从内部顺利提取 `Arc<ExtensionSet>` 并通过三道防线安全起飞！");
    //     Ok(())
    // }

    /// 🔗 升级版拓扑排序：只对借用指针 `&dyn Extension` 进行排序，完美契合 Arc 
    fn topological_sort_extensions_ref(&self) -> Vec<&dyn Extension> {
        // 建立名字到引用的映射
        let mut ext_map: HashMap<&'static str, &dyn Extension> = self.extensions
            .iter()
            .map(|e| (e.name(), e.as_ref()))
            .collect();
            
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        fn dfs<'a>(
            name: &'static str,
            ext_map: &HashMap<&'static str, &'a dyn Extension>,
            visited: &mut HashSet<&'static str>,
            visiting: &mut HashSet<&'static str>,
            sorted: &mut Vec<&'a dyn Extension>,
        ) {
            if visited.contains(name) { return; }
            if visiting.contains(name) {
                panic!("\n🚨 [Runtime Fatal Error]: 发现插件循环依赖循环圈！涉及插件: `{}`\n", name);
            }
            visiting.insert(name);

            if let Some(ext) = ext_map.get(name) {
                for dep in ext.dependencies() {
                    dfs(dep, ext_map, visited, visiting, sorted);
                }
                
                visiting.remove(name);
                visited.insert(name);
                sorted.push(*ext);
            } else {
                visiting.remove(name);
                visited.insert(name);
            }
        }

        let keys: Vec<&'static str> = ext_map.keys().cloned().collect();
        for key in keys {
            dfs(key, &ext_map, &mut visited, &mut visiting, &mut sorted);
        }

        sorted
    }
}