use std::any::Any;
use rquickjs::{ Context, Ctx};
use tokio_util::sync::CancellationToken;

use crate::event_loop::event_loop_impl::{EventPortRegistrar, EventSender, TaskType};

// ── Extension 级环境 ───────────────────────────────────────────────────────────

/// `setup` 的调用环境，每个 Extension 安装时独立创建。
///
/// - `cancel`：感知运行时生命周期的取消令牌
/// - `register_event_port()`：注册事件端口
/// - `declare_native_module::<M>(specifier)`：注册 Rust 原生模块（只允许 `module_specifiers()` 中声明的 specifier）
///
/// 全局变量注入通过 `js_source()` 返回 ESM 代码实现，不在 Rust 侧直接操作 `Context`。
pub struct ExtensionEnv {
    context: Context,
    pub cancel: CancellationToken,
    ports: EventPortRegistrar,
    plugin_name: &'static str,
    allowed_specifiers: &'static [&'static str],
}

impl ExtensionEnv {
    pub fn new(
        context: Context,
        cancel: CancellationToken,
        ports: EventPortRegistrar,
        plugin_name: &'static str,
        allowed_specifiers: &'static [&'static str],
    ) -> Self {
        Self { context, cancel, ports, plugin_name, allowed_specifiers }
    }

    /// 注册原生 Rust 模块，specifier 必须在 `native_module_specifiers()` 中声明。
    pub fn declare_native_module<M: rquickjs::module::ModuleDef>(&self, specifier: &'static str) {
        assert!(
            self.allowed_specifiers.contains(&specifier),
            "[{}] native module `{specifier}` not declared in native_module_specifiers()",
            self.plugin_name
        );
        self.context.with(|ctx| {
            rquickjs::Module::declare_def::<M, _>(ctx.clone(), specifier)
                .expect("failed to declare native module");
        });
    }
    pub fn get_context(&self) -> &Context {
        &self.context
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
/// 每个 Extension 按职责实现以下方法：
///   - `setup(env)`：注册事件端口；调用 `env.declare_native_module` 注册 Rust 原生模块
///   - `native_module_specifiers()`：声明 Rust 原生模块的 specifier（供 assert 校验）
///   - `js_modules()`：用户可 `import` 的 JS 模块列表 `(specifier, ESM source)`
///   - `global_js()`：启动时绑定到 `globalThis` 的 JS 代码（以 ESM 执行，可 `import` 原生模块）
pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn depends_on(&self) -> &'static [&'static str] { &[] }

    fn native_setup(&self, _env: &mut ExtensionEnv) {}

    fn native_module_specifiers(&self) -> &'static [&'static str] { &[] }

    fn js_modules(&self) -> &[(&'static str, &'static str)] { &[] }

    fn global_js(&self) -> Option<&'static str> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
