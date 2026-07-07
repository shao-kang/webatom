use std::any::Any;
use tokio_util::sync::CancellationToken;
use rquickjs::{AsyncRuntime, AsyncContext, Ctx};

use crate::event_loop::event_loop_impl::EventPortRegistrar;
use crate::storage::RoomMemoryCenter;

// ── Runtime 级伴生对象 ───────────────────────────────────────────────────────

/// [`Extension::setup_runtime`] 的上下文，跟随 [`AsyncRuntime`] 生命周期。
///
/// 在此配置 QuickJS 引擎本身：内存上限、栈大小、GC 阈值。
pub struct RuntimeEnv<'a> {
    pub runtime: &'a AsyncRuntime,
}

impl<'a> RuntimeEnv<'a> {
    pub fn new(runtime: &'a AsyncRuntime) -> Self {
        Self { runtime }
    }
}

// ── Context 级伴生对象 ───────────────────────────────────────────────────────

/// [`Extension::setup_context`] 的上下文，仅存在于 `async_ctx.with` 闭包内。
///
/// 提供本插件在当前 Context 的私有存储访问（per-context 隔离）。
/// 跨 Context 的共享状态直接用 Extension 结构体自身的 `Arc<Mutex<T>>` 字段。
pub struct ContextEnv<'a, 'js> {
    pub ctx: &'a Ctx<'js>,
    pub async_ctx: &'a AsyncContext,
    pub cancel: CancellationToken,
    plugin_name: &'static str,
}

impl<'a, 'js> ContextEnv<'a, 'js> {
    pub fn new(
        ctx: &'a Ctx<'js>,
        async_ctx: &'a AsyncContext,
        cancel: CancellationToken,
        plugin_name: &'static str,
    ) -> Self {
        Self { ctx, async_ctx, cancel, plugin_name }
    }

    /// 初始化本插件在当前 Context 的私有存储（`setup_context` 中调用一次）。
    pub fn storage_init<T: Any + Send + Sync>(&self, data: T) {
        let center = self.ctx.userdata::<RoomMemoryCenter>()
            .expect("RoomMemoryCenter not initialized");
        let mut manifest = center.private_manifest.lock().unwrap();
        manifest.insert(
            self.plugin_name.to_string(),
            crate::storage::PluginPrivateStorage { payload: Some(Box::new(data)) },
        );
    }

    /// 只读访问本插件在当前 Context 的私有存储。
    pub fn storage_get<T: 'static, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let center = self.ctx.userdata::<RoomMemoryCenter>()?;
        let manifest = center.private_manifest.lock().unwrap();
        let concrete = manifest.get(self.plugin_name)?.payload.as_ref()?.downcast_ref::<T>()?;
        Some(f(concrete))
    }

    /// 可变访问本插件在当前 Context 的私有存储。
    pub fn storage_get_mut<T: 'static, R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let center = self.ctx.userdata::<RoomMemoryCenter>()?;
        let mut manifest = center.private_manifest.lock().unwrap();
        let concrete = manifest.get_mut(self.plugin_name)?.payload.as_mut()?.downcast_mut::<T>()?;
        Some(f(concrete))
    }
}

// ── Extension trait ──────────────────────────────────────────────────────────

/// 向 QuickJS 引擎注入原生能力的插件接口。
///
/// 安装顺序：
///   1. `setup_runtime`  — `AsyncRuntime` 创建后，全局调用一次
///   2. `setup_context`  — 每个 `AsyncContext` 创建后调用一次
///   3. `js_source`      — 模块加载时按需调用（惰性）
pub trait Extension: Send + Sync + 'static {
    /// 插件唯一标识，如 "timer" / "fetch" / "dom"。
    fn name(&self) -> &'static str;

    /// 必须在本插件之前完成安装的插件（拓扑排序依据）。
    fn depends_on(&self) -> &'static [&'static str] { &[] }

    /// 本插件声明的所有 ES 模块说明符。
    fn module_specifiers(&self) -> &'static [&'static str] { &[] }

    // ── 第一层：引擎级 ──────────────────────────────────────────────────────
    //
    //   `AsyncRuntime` 创建后调用一次。
    //   适合：内存上限、栈大小、GC 阈值、运行时级 userdata。

    fn setup_runtime(&self, _env: &RuntimeEnv<'_>) -> rquickjs::Result<()> {
        Ok(())
    }

    // ── 第二层：JS 环境级 ────────────────────────────────────────────────────
    //
    //   每个 `AsyncContext` 创建后调用（在 `async_ctx.with` 闭包内）。
    //   适合：全局函数注入、原生对象绑定、事件端口注册、context userdata。
    //
    //   存储约定：
    //     per-context 私有状态 → `env.storage_init / storage_get / storage_get_mut`
    //     跨 context 共享状态  → Extension 结构体的 `Arc<Mutex<T>>` 字段

    fn setup_context(
        &self,
        _env: &ContextEnv<'_, '_>,
        _ports: &mut EventPortRegistrar<'_>,
    ) -> rquickjs::Result<()> {
        Ok(())
    }

    // ── 第三层：模块源码（惰性）─────────────────────────────────────────────
    //
    //   模块加载器请求 `owned_specifiers` 中的路径时调用。
    //   纯 Rust 原生模块返回 `None`；JS 胶水层返回源码字符串。

    fn js_source(&self, _specifier: &str) -> Option<String> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
