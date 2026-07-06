use std::any::Any;
use tokio_util::sync::CancellationToken;
use rquickjs::Ctx;

use crate::storage::RoomMemoryCenter;

pub struct ExtensionContext<'a, 'js> {
    pub ctx: &'a Ctx<'js>,
    pub cancel_token: CancellationToken,
}

impl<'a, 'js> ExtensionContext<'a, 'js> {
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

    pub fn set_private_storage(&self, plugin_name: &str, payload: impl Any + Send + Sync) {
        let center = self.ctx.userdata::<RoomMemoryCenter>().expect("❌ 引擎未初始化 RoomMemoryCenter");
        let mut private_manifest = center.private_manifest.lock().unwrap();
        private_manifest.insert(
            plugin_name.to_string(),
            crate::storage::PluginPrivateStorage { payload: Some(Box::new(payload)) }
        );
    }

    pub fn get_private_storage<T: 'static, R, F>(&self, plugin_name: &str, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        let center = self.ctx.userdata::<RoomMemoryCenter>().unwrap();
        let private_manifest = center.private_manifest.lock().unwrap();
        let private_box = private_manifest.get(plugin_name)?;
        let any_payload = private_box.payload.as_ref()?;
        let concrete_data = any_payload.downcast_ref::<T>()?;
        Some(f(concrete_data))
    }
}

pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn native_module_specifiers(&self) -> &'static [&'static str] { &[] }
    fn install(&self, ext_ctx: &ExtensionContext<'_, '_>) -> rquickjs::Result<()>;
    fn js_glue(&self, _ctx: &Ctx<'_>) -> Option<String> { None }
}

pub type ExtensionSet = Vec<Box<dyn Extension>>;