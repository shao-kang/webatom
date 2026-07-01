use rquickjs::{Ctx, Error, Module, Result};
use rquickjs::loader::{ImportAttributes, Loader};

pub struct FileLoader;

impl Loader for FileLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _attr: Option<ImportAttributes<'js>>) -> Result<Module<'js>> {
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
