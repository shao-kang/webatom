use std::{any::Any, time::Duration};
use tokio_util::sync::CancellationToken;
use rquickjs::Ctx;

use crate::{event_loop::event_loop_impl::{EventSender, TaskType, spawn_event_port}, storage::RoomMemoryCenter};

pub struct ExtensionContext<'a, 'js> {
    pub ctx: &'a Ctx<'js>,
    pub async_context: &'a rquickjs::AsyncContext,
    pub cancel_token: CancellationToken,
    name: String,
}

impl<'a, 'js> ExtensionContext<'a, 'js> {
    pub fn new(
        ctx: &'a Ctx<'js>,
        async_context: &'a rquickjs::AsyncContext,
        cancel_token: CancellationToken,
        name: String,
    ) -> Self {
        Self {
            ctx,
            async_context,
            cancel_token,
            name,
        }
    }

    pub fn spawn_event_port<F>(
        &self,
        task_type: TaskType,
        starvation_threshold: Duration,
        rust_handler: F,
    ) -> EventSender
    where
        F: FnMut( &dyn Any) + Send + 'static,
    {
        spawn_event_port(self.async_context, task_type, starvation_threshold, rust_handler)
    }
    pub fn global_storage<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut crate::storage::GlobalRoomStorage) -> R,
    {
        let mut center = self.ctx.userdata::<RoomMemoryCenter>()
            .expect("❌ 引擎未初始化 RoomMemoryCenter");
        // 🎯 核心：通过 lock() 拿到内部可变引用
        let mut global_guard = center.global.lock().unwrap();
        f(&mut global_guard)
    }

    pub fn set_private_storage(&self, payload: impl Any + Send + Sync) {
        let center = self.ctx.userdata::<RoomMemoryCenter>().expect("❌ 引擎未初始化 RoomMemoryCenter");
        let mut private_manifest = center.private_manifest.lock().unwrap();
        private_manifest.insert(
            self.name.clone(),
            crate::storage::PluginPrivateStorage { payload: Some(Box::new(payload)) }
        );
    }

    pub fn get_private_storage<T: 'static, R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        let center = self.ctx.userdata::<RoomMemoryCenter>().unwrap();
        let private_manifest = center.private_manifest.lock().unwrap();
        let private_box = private_manifest.get(&self.name)?;
        let any_payload = private_box.payload.as_ref()?;
        let concrete_data = any_payload.downcast_ref::<T>()?;
        Some(f(concrete_data))
    }
}

pub trait Extension: Send + Sync + 'static {
    /// 插件的唯一标识名（如 "fs", "crypto"）
    fn name(&self) -> &'static str;
    
    /// 宣告依赖的其他插件名（用于拓扑排序）
    fn dependencies(&self) -> &'static [&'static str] { &[] }

    /// 🌐 大一统：上报该插件治下的【所有】ES 模块标识符（Rust 核心模块 + JS 胶水模块）
    fn module_specifiers(&self) -> &'static [&'static str] { &[] }

    /// ⚡ 插件安装核心（专职处理 Rust 侧的底座绑定）
    fn install(&self, ext_ctx: &ExtensionContext<'_, '_>) -> rquickjs::Result<()>;

    /// 🎨 按需索取：根据模块名，吐出对应的纯 JS 模块源码
    /// 如果该说明符在 install 里作为纯 Rust 模块实现，这里直接返回 None
    fn get_js_source(&self, _specifier: &str) -> Option<String> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;