//! 纯 Rust 测试：验证 `Document::parse_html` 解析后的 DOM 树结构正确性。
//! 不依赖 JS 运行时，直接操作 Rust 侧的 Document API。

use webatom_extension_dom::Document;

// W3C DOM Level 1 节点类型常量
const DOCUMENT_NODE: u16 = 9;
const ELEMENT_NODE:  u16 = 1;
const TEXT_NODE:     u16 = 3;
const COMMENT_NODE:  u16 = 8;

/// 测试用 HTML fixture：包含元素、属性、文本、注释，嵌套层次清晰
const FIXTURE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>webAtom Test</title>
</head>
<body>
  <div id="app">
    <h1 class="title">Hello</h1>
    <!-- 注释节点 -->
    <p>世界</p>
  </div>
</body>
</html>"#;

// ── 工具函数 ──────────────────────────────────────────────────────────────────

/// 按文档顺序收集 `parent` 的直接子节点 id。
fn children(doc: &Document, parent: usize) -> Vec<usize> {
    let mut ids = Vec::new();
    let mut cur = doc.first_child(parent);
    while let Some(id) = cur {
        ids.push(id);
        cur = doc.next_sibling(id);
    }
    ids
}

/// 在 `parent` 的直接子节点中找到第一个指定 tag（大写）的元素。
fn find_child_element(doc: &Document, parent: usize, tag: &str) -> Option<usize> {
    let upper = tag.to_uppercase();
    children(doc, parent)
        .into_iter()
        .find(|&id| doc.tag_name(id).as_deref() == Some(upper.as_str()))
}

// ── 文档根节点 ────────────────────────────────────────────────────────────────

#[test]
fn root_node_type_is_document() {
    let doc = Document::parse_html(FIXTURE);
    assert_eq!(doc.node_type(doc.root()), Some(DOCUMENT_NODE));
}

#[test]
fn root_has_no_parent() {
    let doc = Document::parse_html(FIXTURE);
    assert_eq!(doc.parent_node(doc.root()), None);
}

// ── <html> 元素 ───────────────────────────────────────────────────────────────

#[test]
fn html_element_exists() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("document_element 应指向 <html>");
    assert_eq!(doc.node_type(html_id), Some(ELEMENT_NODE));
    assert_eq!(doc.tag_name(html_id).as_deref(), Some("HTML"));
}

#[test]
fn html_lang_attribute() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("document_element 应指向 <html>");
    assert_eq!(
        doc.get_attribute(html_id, "lang").as_deref(),
        Some("en"),
        "<html> 的 lang 属性应为 en"
    );
}

#[test]
fn html_parent_is_root() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("document_element 应指向 <html>");
    assert_eq!(doc.parent_node(html_id), Some(doc.root()));
}

// ── <head> 与 <body> ──────────────────────────────────────────────────────────

#[test]
fn head_exists_as_child_of_html() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("<html>");
    let head_id = doc.head.expect("<head> 应存在");
    assert_eq!(doc.tag_name(head_id).as_deref(), Some("HEAD"));
    assert_eq!(doc.parent_node(head_id), Some(html_id));
}

#[test]
fn body_exists_as_child_of_html() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("<html>");
    let body_id = doc.body.expect("<body> 应存在");
    assert_eq!(doc.tag_name(body_id).as_deref(), Some("BODY"));
    assert_eq!(doc.parent_node(body_id), Some(html_id));
}

#[test]
fn head_comes_before_body_in_html_children() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("<html>");
    let element_children: Vec<String> = children(&doc, html_id)
        .into_iter()
        .filter_map(|id| doc.tag_name(id))
        .collect();
    let head_pos = element_children.iter().position(|t| t == "HEAD").expect("HEAD");
    let body_pos = element_children.iter().position(|t| t == "BODY").expect("BODY");
    assert!(head_pos < body_pos, "HEAD 应排在 BODY 之前");
}

// ── <head> 内容 ───────────────────────────────────────────────────────────────

#[test]
fn meta_charset_attribute() {
    let doc = Document::parse_html(FIXTURE);
    let head_id = doc.head.expect("<head>");
    let meta_id = find_child_element(&doc, head_id, "meta").expect("<meta> 应在 <head> 内");
    assert_eq!(
        doc.get_attribute(meta_id, "charset").as_deref(),
        Some("UTF-8"),
        "<meta> 的 charset 应为 UTF-8"
    );
}

#[test]
fn title_text_content() {
    let doc = Document::parse_html(FIXTURE);
    let head_id = doc.head.expect("<head>");
    let title_id = find_child_element(&doc, head_id, "title").expect("<title> 应在 <head> 内");
    let text_id  = doc.first_child(title_id).expect("<title> 应有文本子节点");
    assert_eq!(doc.node_type(text_id), Some(TEXT_NODE));
    assert_eq!(doc.node_value(text_id).as_deref(), Some("webAtom Test"));
}

// ── <body> 内容 ───────────────────────────────────────────────────────────────

#[test]
fn div_app_exists_in_body() {
    let doc = Document::parse_html(FIXTURE);
    let body_id = doc.body.expect("<body>");
    let div_id = find_child_element(&doc, body_id, "div").expect("<div> 应在 <body> 内");
    assert_eq!(doc.get_attribute(div_id, "id").as_deref(), Some("app"));
}

#[test]
fn h1_class_and_text_in_div() {
    let doc = Document::parse_html(FIXTURE);
    let body_id = doc.body.expect("<body>");
    let div_id  = find_child_element(&doc, body_id, "div").expect("<div#app>");
    let h1_id   = find_child_element(&doc, div_id, "h1").expect("<h1> 应在 <div> 内");

    assert_eq!(doc.get_attribute(h1_id, "class").as_deref(), Some("title"));

    let text_id = doc.first_child(h1_id).expect("<h1> 应有文本子节点");
    assert_eq!(doc.node_type(text_id), Some(TEXT_NODE));
    assert_eq!(doc.node_value(text_id).as_deref(), Some("Hello"));
}

#[test]
fn p_text_content_in_div() {
    let doc = Document::parse_html(FIXTURE);
    let body_id = doc.body.expect("<body>");
    let div_id  = find_child_element(&doc, body_id, "div").expect("<div#app>");
    let p_id    = find_child_element(&doc, div_id, "p").expect("<p> 应在 <div> 内");
    let text_id = doc.first_child(p_id).expect("<p> 应有文本子节点");
    assert_eq!(doc.node_value(text_id).as_deref(), Some("世界"));
}

#[test]
fn comment_node_exists_in_div() {
    let doc = Document::parse_html(FIXTURE);
    let body_id = doc.body.expect("<body>");
    let div_id  = find_child_element(&doc, body_id, "div").expect("<div#app>");
    let has_comment = children(&doc, div_id)
        .into_iter()
        .any(|id| doc.node_type(id) == Some(COMMENT_NODE));
    assert!(has_comment, "<div> 内应含有注释节点");
}

// ── attributes_list ───────────────────────────────────────────────────────────

#[test]
fn attributes_list_contains_all_attrs() {
    let doc = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("<html>");
    let attrs = doc.attributes_list(html_id);
    assert!(
        attrs.iter().any(|(k, v)| k == "lang" && v == "en"),
        "attributes_list 应包含 lang=en，实际：{attrs:?}"
    );
}

// ── 父子关系完整性 ────────────────────────────────────────────────────────────

#[test]
fn parent_child_roundtrip() {
    let doc     = Document::parse_html(FIXTURE);
    let html_id = doc.document_element.expect("<html>");
    let head_id = doc.head.expect("<head>");
    let body_id = doc.body.expect("<body>");
    let div_id  = find_child_element(&doc, body_id, "div").expect("<div>");
    let h1_id   = find_child_element(&doc, div_id, "h1").expect("<h1>");

    // 每层的 parent_node 必须指向上层
    assert_eq!(doc.parent_node(html_id), Some(doc.root()), "html.parent = root");
    assert_eq!(doc.parent_node(head_id), Some(html_id),   "head.parent = html");
    assert_eq!(doc.parent_node(body_id), Some(html_id),   "body.parent = html");
    assert_eq!(doc.parent_node(div_id),  Some(body_id),   "div.parent  = body");
    assert_eq!(doc.parent_node(h1_id),   Some(div_id),    "h1.parent   = div");
}
