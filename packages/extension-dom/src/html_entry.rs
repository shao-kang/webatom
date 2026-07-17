use std::sync::Arc;

const ERROR_PAGE_URL: &str = "webatom://error";
const ERROR_PAGE_HTML: &str = include_str!("error.html");

#[derive(Debug)]
pub enum HtmlEntryError {
    Io(std::io::Error),
    Http(reqwest::Error),
}

impl std::fmt::Display for HtmlEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e)   => write!(f, "IO error: {e}"),
            Self::Http(e) => write!(f, "HTTP error: {e}"),
        }
    }
}

impl std::error::Error for HtmlEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e)   => Some(e),
            Self::Http(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub struct HtmlEntry {
    url: String,
    content: String,
}

impl HtmlEntry {
    /// 直接从 url 和已有内容构造（用于 include_str! 内嵌场景）。
    pub fn new(url: impl Into<String>, content: impl Into<String>) -> Self {
        Self { url: url.into(), content: content.into() }
    }

    /// 加载失败时使用的内置错误页面。
    pub fn error_page() -> Arc<Self> {
        Arc::new(Self {
            url:     ERROR_PAGE_URL.to_owned(),
            content: ERROR_PAGE_HTML.to_owned(),
        })
    }

    /// 自动检测来源并异步加载；失败时回退到 `fallback`，不传则使用内置错误页面。
    pub async fn load_or_error(
        source: impl Into<String>,
        fallback: impl Into<Option<Arc<Self>>>,
    ) -> Arc<Self> {
        let source = source.into();
        match Self::load(&source).await {
            Ok(entry) => entry,
            Err(e) => {
                tracing::warn!(source, error = %e, "HTML 入口加载失败，使用错误页面");
                fallback.into().unwrap_or_else(Self::error_page)
            }
        }
    }

    /// 自动检测来源并异步加载，失败返回 `Err`。
    pub async fn load(source: impl Into<String>) -> Result<Arc<Self>, HtmlEntryError> {
        let source = source.into();
        if source.starts_with("http://") || source.starts_with("https://") {
            Self::from_http(source).await
        } else {
            Self::from_file(source).await
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    /// 相对路径解析基准（同目录 URL，末尾含 /），从 url 实时计算。
    pub fn base_url(&self) -> &str {
        match self.url.rfind('/') {
            Some(pos) => &self.url[..=pos],
            None => &self.url,
        }
    }

    async fn from_file(path: String) -> Result<Arc<Self>, HtmlEntryError> {
        let url = if path.starts_with("file://") {
            path
        } else {
            format!("file://{path}")
        };
        let fs_path = url.strip_prefix("file://").unwrap_or(&url).to_owned();
        let content = std::fs::read_to_string(&fs_path).map_err(HtmlEntryError::Io)?;
        Ok(Arc::new(Self { url, content }))
    }

    async fn from_http(url: String) -> Result<Arc<Self>, HtmlEntryError> {
        let content = reqwest::get(&url)
            .await
            .map_err(HtmlEntryError::Http)?
            .text()
            .await
            .map_err(HtmlEntryError::Http)?;
        Ok(Arc::new(Self { url, content }))
    }
}
