use rquickjs::{Ctx, Error, Module, Result};
use rquickjs::loader::{ImportAttributes, Loader};

/// `name -> Option<source_code>`
/// 返回 `Some` 表示命中，`None` 交给默认文件系统逻辑。
type LoaderFn = Box<dyn FnMut(&str) -> Option<String> + Send>;

pub struct FileLoader {
    extras: Vec<LoaderFn>,
}

impl Default for FileLoader {
    fn default() -> Self {
        Self { extras: Vec::new() }
    }
}

impl FileLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册扩展加载函数，在文件系统加载之前尝试。
    /// 返回 `Some(source)` 命中；`None` 继续走文件系统。
    pub fn add_loader<F>(&mut self, f: F)
    where
        F: FnMut(&str) -> Option<String> + Send + 'static,
    {
        self.extras.push(Box::new(f));
    }
}

impl Loader for FileLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _attr: Option<ImportAttributes<'js>>) -> Result<Module<'js>> {
        // 先试扩展加载器
        for extra in &mut self.extras {
            if let Some(source) = extra(name) {
                return Module::declare(ctx.clone(), name, source);
            }
        }

        if name.starts_with("data:") {
            let source = decode_data_url(name)
                .ok_or_else(|| Error::new_loading_message(name, "invalid data: URL"))?;
            return Module::declare(ctx.clone(), name, source);
        }
        if let Some(path) = name.strip_prefix("file://") {
            let source = std::fs::read_to_string(path)
                .map_err(|e| Error::new_loading_message(name, e.to_string()))?;
            return Module::declare(ctx.clone(), name, source);
        }
        let source = std::fs::read_to_string(name)
            .map_err(|e| Error::new_loading_message(name, e.to_string()))?;
        Module::declare(ctx.clone(), name, source)
    }
}

fn decode_data_url(url: &str) -> Option<String> {
    // Format: data:text/javascript[;...],<percent-encoded-source>
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let encoded = &rest[comma + 1..];
    Some(percent_decode(encoded))
}

fn percent_decode(s: &str) -> String {
    let input = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let Ok(hex) = std::str::from_utf8(&input[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(input[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
