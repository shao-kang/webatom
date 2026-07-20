use rquickjs::{Context, Ctx, JsLifetime, runtime::UserDataGuard};
use std::any::Any;
use tokio_util::sync::CancellationToken;

use crate::{
    event_loop::event_loop_impl::{EventPortRegistrar, EventPort, QueueKind},
    module::ImportMap,
};

// ── Extension 级环境 ───────────────────────────────────────────────────────────

/// `setup` 的调用环境，每个 Extension 安装时独立创建。
#[derive()]
pub struct ExtensionEnv<'a> {
    context: &'a Context,
    pub cancel: CancellationToken,
    ports: EventPortRegistrar,
    plugin_name: &'static str,
    allowed_specifiers: &'static [&'static str],
    /// 与 EsmResolver 共享的 import map，Extension 可在 native_setup 中动态写入。
    pub import_map: ImportMap,
}

impl<'a> ExtensionEnv<'a> {
    pub fn new(
        context: &'a Context,
        cancel: CancellationToken,
        ports: EventPortRegistrar,
        plugin_name: &'static str,
        allowed_specifiers: &'static [&'static str],
        import_map: ImportMap,
    ) -> Self {
        Self {
            context,
            cancel,
            ports,
            plugin_name,
            allowed_specifiers,
            import_map,
        }
    }

    /// 返回当前 `Context` 的克隆。
    pub fn context(&self) -> Context {
        self.context.clone()
    }

    pub fn set_state<T>(&self, data: T)
    where
        T: 'static + for<'js> JsLifetime<'js>,
    {
        self.context.with(|ctx| {
            ctx.store_userdata(data).unwrap();
        });
    }

    pub fn get_state<'c, 'js, T>(ctx: &'c Ctx<'js>) -> Option<UserDataGuard<'c, T>>
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

    /// 注册一个事件端口，返回可跨线程 Clone 的 [`EventPort`]。
    pub fn register_event_port<F>(&self, queue: QueueKind, handler: F) -> EventPort
    where
        F: FnMut(&dyn Any) + 'static,
    {
        self.ports.register_event_port(queue, handler)
    }

    pub fn register_js_event_port<F>(&self, queue: QueueKind, handler: F) -> EventPort
    where
        F: FnMut(Ctx<'_>, &dyn Any) -> rquickjs::Result<()> + 'static,
    {
        self.ports.register_js_event_port(queue, handler)
    }
}

// ── Extension trait ────────────────────────────────────────────────────────────

pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }

    fn native_setup(&self, _env: &mut ExtensionEnv) {}

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &[]
    }

    fn js_modules(&self) -> &[(&'static str, &'static str)] {
        &[]
    }

    fn global_js(&self) -> Option<&'static str> {
        None
    }

    /// 额外的模块路径解析函数。`(base, name) -> Option<resolved>`，返回 `Some` 则命中。
    /// 在默认 `EsmResolver` 逻辑之前尝试，按注册顺序依次调用。
    fn extra_resolvers(&self) -> Vec<Box<dyn FnMut(&str, &str) -> Option<String> + Send>> {
        vec![]
    }

    /// 额外的模块源码加载函数。`name -> Option<source>`，返回 `Some` 则命中。
    /// 在默认文件系统加载之前尝试，按注册顺序依次调用。
    fn extra_loaders(&self) -> Vec<Box<dyn FnMut(&str) -> Option<String> + Send>> {
        vec![]
    }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;
