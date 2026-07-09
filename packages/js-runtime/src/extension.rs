use std::any::Any;
use rquickjs::{Runtime, Context, Ctx};
use tokio_util::sync::CancellationToken;

use crate::event_loop::event_loop_impl::{EventPortRegistrar, EventSender, TaskType};

// ── Extension 级环境 ───────────────────────────────────────────────────────────

/// `setup` 的调用环境，每个 Extension 安装时独立创建。
///
/// - `runtime`：引擎级配置（内存上限、GC 阈值、栈大小等）
/// - `ctx`：通过 `ctx.with(|ctx| { ... })` 注入 JS 全局对象
/// - `cancel`：感知运行时生命周期的取消令牌
pub struct ExtensionEnv<'a> {
    runtime: &'a Runtime,
    ctx: &'a Context,
    pub cancel: CancellationToken,
    ports: EventPortRegistrar<'a>,
    plugin_name: &'static str,
}

impl<'a> ExtensionEnv<'a> {
    pub fn new(
        runtime: &'a Runtime,
        ctx: &'a Context,
        cancel: CancellationToken,
        ports: EventPortRegistrar<'a>,
        plugin_name: &'static str,
    ) -> Self {
        Self { runtime, ctx, cancel, ports, plugin_name }
    }

    pub fn get_context(&self) -> &'a Context {
        self.ctx
    }

    pub fn get_runtime(&self) -> &'a Runtime {
        self.runtime
    }

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventSender`]。
    pub fn register_event_port<F>(&mut self, task_type: TaskType, handler: F) -> EventSender
    where
        F: FnMut(&dyn Any) + 'static,
    {
        self.ports.register_event_port(task_type, handler)
    }

    /// 初始化本插件在当前 Context 的私有存储（`setup` 中调用一次）。
    pub fn storage_init<T: Any + Send + Sync>(&self, ctx: &Ctx<'_>, data: T) {
        let center = ctx.userdata::<crate::storage::RoomMemoryCenter>()
            .expect("RoomMemoryCenter not initialized");
        let mut manifest = center.private_manifest.lock().unwrap();
        manifest.insert(
            self.plugin_name.to_string(),
            crate::storage::PluginPrivateStorage { payload: Some(Box::new(data)) },
        );
    }

    /// 只读访问本插件的私有存储。
    pub fn storage_get<T: 'static, R>(
        &self,
        ctx: &Ctx<'_>,
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
    pub fn storage_get_mut<T: 'static, R>(
        &self,
        ctx: &Ctx<'_>,
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
///   - 引擎级配置（通过 `env.get_runtime()`）
///   - JS 全局函数注入（通过 `env.get_context().with(|ctx| { ... })`）
///   - 事件端口注册（通过 `env.register_event_port(...)`）
pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn depends_on(&self) -> &'static [&'static str] { &[] }

    fn module_specifiers(&self) -> &'static [&'static str] { &[] }

    fn setup(&self, _env: &mut ExtensionEnv<'_>) {}

    fn js_source(&self, _specifier: &str) -> Option<String> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
