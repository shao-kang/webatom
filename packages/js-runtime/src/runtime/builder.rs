use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use rquickjs::AsyncRuntime;

// 引入你之前定义好的核心组件
pub use crate::extension::{Extension, ExtensionSet};
pub use crate::runtime::JsRuntime;

/// 🏎️ 现代运行时的高效装配流水线
pub struct JsRuntimeBuilder {
    extensions: ExtensionSet,
    cancel_token: Option<CancellationToken>,
}

impl Default for JsRuntimeBuilder {
    /// 默认状态：没有外挂插件，但全自动准备好退出拉闸安全大闸
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            cancel_token: None,
        }
    }
}

impl JsRuntimeBuilder {
    /// 1. 开启建造者流水线
    pub fn new() -> Self {
        Self::default()
    }

    /// 2. 链式注入单个原生扩展插件（最常用的爽点方法）
    pub fn with_extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    /// 3. 一次性批量打包注入一组插件
    pub fn with_extensions(mut self, mut exts: ExtensionSet) -> Self {
        self.extensions.append(&mut exts);
        self
    }

    /// 4. 自定义取消令牌（如果不传，build 时会自动生成一个默认的）
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// 🚀 最终一键点火，重组并孵化出完全体的 JsRuntime
    pub fn build(self) -> rquickjs::Result<JsRuntime> {
        
        // 如果用户没传令牌，全自动现场生成一个，绝不给用户添堵
        let cancel_token = self.cancel_token.unwrap_or_else(CancellationToken::new);

        Ok(JsRuntime::new( self.extensions, cancel_token))
    }
}