use std::sync::Arc;

#[derive(Debug)]
pub enum HtmlEntryError {
    Io(std::io::Error),
    HttpNotSupported,
}

impl std::fmt::Display for HtmlEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::HttpNotSupported => {
                write!(f, "HTTP entry loading not yet supported (Phase 2)")
            }
        }
    }
}

impl std::error::Error for HtmlEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::HttpNotSupported => None,
        }
    }
}

#[derive(Debug)]
pub struct HtmlEntry {
    /// 原始入口地址（file:// URL 或 http(s):// URL）
    pub url: String,
    /// 相对路径解析基准（同目录 URL，末尾含 /）
    pub base_url: String,
    /// 原始 HTML 文本
    pub content: String,
}

impl HtmlEntry {
    /// 自动检测来源并异步加载。`source` 可以是裸文件路径、`file://` URL 或 `http(s)://` URL。
    pub async fn load(source: impl Into<String>) -> Result<Arc<Self>, HtmlEntryError> {
        let source = source.into();
        if source.starts_with("http://") || source.starts_with("https://") {
            Self::from_http(source).await
        } else {
            Self::from_file(source).await
        }
    }

    async fn from_file(path: String) -> Result<Arc<Self>, HtmlEntryError> {
        let url = if path.starts_with("file://") {
            path
        } else {
            format!("file://{path}")
        };

        // base_url = 最后一个 '/' 之前的部分 + '/'
        let base_url = match url.rfind('/') {
            Some(pos) => format!("{}/", &url[..pos]),
            None => format!("{url}/"),
        };

        let fs_path = url.strip_prefix("file://").unwrap_or(&url).to_owned();
        let content = std::fs::read_to_string(&fs_path).map_err(HtmlEntryError::Io)?;

        Ok(Arc::new(Self { url, base_url, content }))
    }

    /// Phase 2：从 HTTP/HTTPS URL 加载入口文件。
    async fn from_http(_url: String) -> Result<Arc<Self>, HtmlEntryError> {
        Err(HtmlEntryError::HttpNotSupported)
    }
}
