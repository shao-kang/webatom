use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;
use rquickjs::{AsyncRuntime, AsyncContext, Ctx};

use crate::event_loop::event_loop_impl::{EventPortRegistrar, EventSender, TaskType};

// ── Extension 级环境 ───────────────────────────────────────────────────────────

/// `setup` 的调用环境，每个 Extension 安装时独立创建。
///
/// - `runtime`：引擎级配置（内存上限、GC 阈值、栈大小等）
/// - `async_ctx`：通过 `async_ctx.with(|ctx| { ... })` 注入 JS 全局对象
/// - `cancel`：感知运行时生命周期的取消令牌
pub struct ExtensionEnv<'a> {
    runtime: &'a AsyncRuntime,
    async_ctx: &'a AsyncContext,
    pub cancel: CancellationToken,
    ports: EventPortRegistrar<'a>,
    plugin_name: &'static str,
}

impl<'a> ExtensionEnv<'a> {
    pub fn new(
        runtime: &'a AsyncRuntime,
        async_ctx: &'a AsyncContext,
        cancel: CancellationToken,
        ports: EventPortRegistrar<'a>,
        plugin_name: &'static str,
    ) -> Self {
        Self { runtime, async_ctx, cancel, ports, plugin_name }
    }

    pub fn get_context(&self) -> &'a AsyncContext {
        self.async_ctx
    }
    pub fn get_runtime(&self) -> &'a AsyncRuntime {
        self.runtime
    }


    /// 初始化本插件在当前 Context 的私有存储（`setup` 中调用一次）。
    pub fn storage_init<'js, T: Any + Send + Sync>(&self, ctx: &Ctx<'js>, data: T) {
        let center = ctx.userdata::<crate::storage::RoomMemoryCenter>()
            .expect("RoomMemoryCenter not initialized");
        let mut manifest = center.private_manifest.lock().unwrap();
        manifest.insert(
            self.plugin_name.to_string(),
            crate::storage::PluginPrivateStorage { payload: Some(Box::new(data)) },
        );
    }

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventSender`]。
    pub fn register_event_port<F>(&mut self, task_type: TaskType, handler: F) -> EventSender
    where
        F: FnMut(&dyn std::any::Any) + 'static,
    {
        self.ports.register_event_port(task_type, handler)
    }

    /// 只读访问本插件的私有存储。
    pub fn storage_get<'js, T: 'static, R>(
        &self,
        ctx: &Ctx<'js>,
        f: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        let center = ctx.userdata::<crate::storage::RoomMemoryCenter>()?;
        let manifest = center.private_manifest.lock().unwrap();
        let concrete = manifest
            .get(self.plugin_name)?
            .payload.as_ref()?
            .downcast_ref::<T>()?;
        Some(f(concrete))
    }

    /// 可变访问本插件的私有存储。
    pub fn storage_get_mut<'js, T: 'static, R>(
        &self,
        ctx: &Ctx<'js>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let center = ctx.userdata::<crate::storage::RoomMemoryCenter>()?;
        let mut manifest = center.private_manifest.lock().unwrap();
        let concrete = manifest
            .get_mut(self.plugin_name)?
            .payload.as_mut()?
            .downcast_mut::<T>()?;
        Some(f(concrete))
    }
}

// ── Extension trait ────────────────────────────────────────────────────────────

/// 向运行时注入原生能力的插件接口。
///
/// 每个 Extension 只需实现 [`Extension::setup`]，在其中完成：
///   - 引擎级配置（通过 `env.runtime`）
///   - JS 全局函数注入（通过 `env.async_ctx.with(|ctx| { ... })`）
///   - 事件端口注册（通过 `env.register_event_port(...)`）
pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn depends_on(&self) -> &'static [&'static str] { &[] }

    fn module_specifiers(&self) -> &'static [&'static str] { &[] }

    fn setup<'a>(
        &'a self,
        _env: &'a mut ExtensionEnv<'a>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async {})
    }

    fn js_source(&self, _specifier: &str) -> Option<String> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
