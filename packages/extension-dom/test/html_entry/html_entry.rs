use webatom_extension_dom::html_entry::HtmlEntry;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test/html_entry/fixtures/entry.html"
);

/// 裸路径加载：url 规范化为 file://，base_url 指向父目录
#[tokio::test]
async fn load_local_path() {
    let entry = HtmlEntry::load(FIXTURE).await.expect("should load local file");
    assert!(entry.url().starts_with("file://"), "url={}", entry.url());
    assert!(entry.base_url().ends_with('/'), "base_url must end with /: {}", entry.base_url());
    assert!(
        !entry.base_url().contains("entry.html"),
        "base_url should not contain filename: {}",
        entry.base_url()
    );
    assert!(entry.content().contains("Entry Test"), "content missing expected text");
}

/// base_url 应指向 fixtures/ 目录
#[tokio::test]
async fn base_url_points_to_parent_dir() {
    let entry = HtmlEntry::load(FIXTURE).await.unwrap();
    assert!(
        entry.base_url().contains("fixtures/"),
        "base_url should end with fixtures/: {}",
        entry.base_url()
    );
}

/// file:// 前缀路径：url 保持原样，内容相同
#[tokio::test]
async fn file_url_prefix_works() {
    let url = format!("file://{FIXTURE}");
    let entry = HtmlEntry::load(&url).await.unwrap();
    assert!(entry.content().contains("Entry Test"));
    assert_eq!(entry.url(), url);
}

/// HTTP URL 在 Phase 1 应返回错误
#[tokio::test]
async fn http_source_returns_error() {
    let result = HtmlEntry::load("https://example.com/index.html").await;
    assert!(result.is_err(), "HTTP load should fail in Phase 1");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("HTTP") || msg.contains("Phase 2"),
        "error message should mention HTTP/Phase 2: {msg}"
    );
}
