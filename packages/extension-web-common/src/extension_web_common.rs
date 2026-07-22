use js_runtime::extension::Extension;


pub struct WebCommonExtension;
impl WebCommonExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for WebCommonExtension {
    fn name(&self) -> &'static str {
        "extension-web-common"
    }

    fn global_js(&self) -> Option<&'static str> {
        Some(include_str!("../js/dist/index.js"))
    }
}
