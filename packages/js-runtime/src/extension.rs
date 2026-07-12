use std::any::Any;
use rquickjs::{ Context, Ctx, JsLifetime, runtime::UserDataGuard};
use tokio_util::sync::CancellationToken;

use crate::{event_loop::event_loop_impl::{EventPortRegistrar, EventSender, TaskType}, storage::RoomMemoryCenter};

// ── Extension 级环境 ───────────────────────────────────────────────────────────

/// `setup` 的调用环境，每个 Extension 安装时独立创建。
///
/// - `cancel`：感知运行时生命周期的取消令牌
/// - `register_event_port()`：注册事件端口
/// - `declare_native_module::<M>(specifier)`：注册 Rust 原生模块（只允许 `module_specifiers()` 中声明的 specifier）
///
/// 全局变量注入通过 `js_source()` 返回 ESM 代码实现，不在 Rust 侧直接操作 `Context`。
pub struct ExtensionEnv<'a> {
    context: &'a Context,
    pub cancel: CancellationToken,
    ports: EventPortRegistrar,
    plugin_name: &'static str,
    allowed_specifiers: &'static [&'static str],
}

impl<'a> ExtensionEnv<'a> {
    pub fn new(
        context: &'a Context,
        cancel: CancellationToken,
        ports: EventPortRegistrar,
        plugin_name: &'static str,
        allowed_specifiers: &'static [&'static str],
    ) -> Self {
        Self { context, cancel, ports, plugin_name, allowed_specifiers }
    }

   pub fn set_state< T>(&self, data: T)
where
    T: 'static + for<'js>JsLifetime<'js>,
{
    self.context.with(|ctx| {
        ctx.store_userdata(data).unwrap();
    });
}
    pub fn get_state<'js, T>(ctx: &'js Ctx<'js>) -> Option<UserDataGuard<'js, T>>
where
    T: 'static + JsLifetime<'js>,
{
    ctx.userdata::<T>()
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

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventSender`]。
    pub fn register_event_port<F>(&mut self, task_type: TaskType, handler: F) -> EventSender
    where
        F: FnMut(&dyn Any) + 'static,
    {
        self.ports.register_event_port(task_type, handler)
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

    fn native_setup(&self, _env: &mut ExtensionEnv<'_>) {}

    fn native_module_specifiers(&self) -> &'static [&'static str] { &[] }

    fn js_modules(&self) -> &[(&'static str, &'static str)] { &[] }

    fn global_js(&self) -> Option<&'static str> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
