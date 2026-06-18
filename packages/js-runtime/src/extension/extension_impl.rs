use std::collections::HashMap;

use rquickjs::{ Ctx, Result};

use crate::event_loop::{ActiveHandles, EventLoopHandle};




pub trait Extension {
    fn name(&self) -> &'static str;
    fn native_module_name(&self) -> String {
        format!("webatom_ext_native:{}", self.name())
    }
    fn module_name(&self) -> String {
        format!("webatom_ext_js:{}", self.name())
    }
    /// 钩子 1: 初始化 (Init)
    /// 触发时机：JS 引擎刚创建，尚未加载任何业务代码。
    /// 作用：注入底层 Rust API (globals)、注册原生模块。
    /// 状态管理：在这里创建状态并注入到 RuntimeContext 或 JS 全局对象中。
    fn on_init(&self, _ctx: &Ctx<'_>) -> Result<() > {
        Ok(())
    }


    fn event_loop_handle<'js>(&self, ctx: &Ctx<'js>) -> EventLoopHandle {
        ctx.userdata::<EventLoopHandle>()
            .expect("EventLoopHandle userdata not registered")
            .clone()
    }

    fn active_handles<'js>(&self, ctx: &Ctx<'js>) -> ActiveHandles {
        self.event_loop_handle(ctx).active_handles
    }
    fn init<'js>(&self, _ctx: &Ctx<'js>) {}
    
    // fn native_module(&self) -> Option<fn(&Ctx<'_>) -> rquickjs::Result<()>> { None }
    fn native_module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()>{ Ok(()) }
    fn js_module_init<'js>(&self, _ctx: &Ctx<'js>) -> Result<()> {
        Ok(())
    }


}


#[allow(dead_code)]
struct ExtensionArgs {
    args: HashMap<String, String>,
}