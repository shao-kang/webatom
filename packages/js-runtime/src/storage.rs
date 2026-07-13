use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tokio_util::sync::CancellationToken;

pub struct GlobalRoomStorage {
    pub cancel_token: CancellationToken,
    #[allow(dead_code)]
    pub share_map: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl GlobalRoomStorage {
    pub fn new(token: CancellationToken) -> Self {
        Self {
            cancel_token: token,
            share_map: HashMap::new(),
        }
    }
}

pub struct PluginPrivateStorage {
    pub payload: Option<Box<dyn Any + Send + Sync>>,
}

use std::sync::Mutex;

use crate::anymap;
pub struct RoomMemoryCenter {
    pub global: Mutex<GlobalRoomStorage>,
    pub private_manifest: Rc<RefCell<anymap::AnyMap>>,
}
impl Drop for RoomMemoryCenter {
    fn drop(&mut self) {
        println!("💥 [内核管家] 检测到当前房间正在覆灭！正在全网拉响资源自毁炸弹...");
        // self.global.cancel_token.cancel(); 
        // self.private_manifest.clear();
        // self.global.share_map.clear();
        if let Ok(global) = self.global.get_mut() {
            // 顺藤摸瓜拉响炸弹，震碎所有长驻异步泵和后台多线程
            global.cancel_token.cancel();
        }
        
        // 顺便把私有仓也安全清空
        let mut manifest = self.private_manifest.borrow_mut();
        manifest.clear();
        
    }
}

/// 🎯 安全背书：由于管家内部的所有动态载荷都是 'static 的，
/// 我们百分之百担保它不会发生由于 JS 生命周期错配导致的悬空指针（Dangling Pointer）。
unsafe impl<'js> rquickjs::JsLifetime<'js> for RoomMemoryCenter {
    // 明确声明在生命周期流转中，管家类型保持磐石般不变
    type Changed<'a> = RoomMemoryCenter;
}
