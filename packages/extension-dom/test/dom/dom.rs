use js_runtime::JsRuntime;
use rquickjs::Error;
use webatom_blitz_msg::create_channels;
use webatom_extension_dom::{DomExtension, DomExtensionState, html_entry::HtmlEntry};

const SIMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test/dom/fixtures/simple.html"
);

const WITH_CLASSIC_SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test/dom/fixtures/with_classic_script.html"
);

const WITH_IMPORTMAP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test/dom/fixtures/with_importmap.html"
);

// ── 工具函数 ──────────────────────────────────────────────────────────────────

fn init_tracing() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

fn build_headless() -> Result<JsRuntime, Error> {
    init_tracing();
    JsRuntime::builder()
        .with_extension(DomExtension::new())
        .build()
}

async fn build_with_html(path: &str) -> JsRuntime {
    init_tracing();
    let (js_side, _blitz) = create_channels();
    let entry = HtmlEntry::load(path).await.expect("fixture should load");
    let state = DomExtensionState::new(js_side).with_entry(entry);
    JsRuntime::builder()
        .with_extension(DomExtension::with_state(state))
        .build()
        .expect("runtime should build")
}

// ── 基础启动 ──────────────────────────────────────────────────────────────────

/// DomExtension 无 HTML 入口时能正常构建，事件循环无任务时立即退出。
#[tokio::test]
async fn setup_dom_succeeds() {
    build_headless()
        .unwrap()
        .run().await
        .unwrap();
}

// ── DocumentHandle 原语 ───────────────────────────────────────────────────────

/// createElement / createTextNode / appendChild / firstChild / nodeType / nodeValue
#[tokio::test]
async fn create_and_append_elements() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc  = new DocumentHandle();
        const div  = doc.createElement('div');
        const text = doc.createTextNode('hello');
        doc.appendChild(div, text);
        const root = doc.documentNode();
        doc.appendChild(root, div);

        const child = doc.firstChild(root);
        if (child !== div) throw new Error('firstChild of root should be div');
        const textChild = doc.firstChild(div);
        if (doc.nodeType(textChild) !== 3) throw new Error('expected TEXT_NODE(3)');
        if (doc.nodeValue(textChild) !== 'hello') throw new Error('wrong text content');
    "#).expect("eval_module should succeed");

    rt.run().await.expect("event loop");
}

/// setAttribute / getAttribute / hasAttribute / removeAttribute
#[tokio::test]
async fn attribute_operations() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc = new DocumentHandle();
        const el  = doc.createElement('div');

        doc.setAttribute(el, 'id', 'app');
        if (doc.getAttribute(el, 'id') !== 'app')
            throw new Error('getAttribute failed');
        if (!doc.hasAttribute(el, 'id'))
            throw new Error('hasAttribute should be true');

        doc.removeAttribute(el, 'id');
        if (doc.hasAttribute(el, 'id'))
            throw new Error('hasAttribute should be false after remove');
        if (doc.getAttribute(el, 'id') !== null)
            throw new Error('getAttribute should return null after remove');
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

/// insertBefore / removeChild / nextSibling / previousSibling / childNodes
#[tokio::test]
async fn tree_manipulation() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc    = new DocumentHandle();
        const parent = doc.createElement('ul');
        const a = doc.createElement('li');
        const b = doc.createElement('li');
        const c = doc.createElement('li');

        doc.appendChild(parent, a);
        doc.appendChild(parent, c);
        doc.insertBefore(parent, b, c);  // a → b → c

        const children = doc.childNodes(parent);
        if (children.length !== 3)
            throw new Error('expected 3 children, got ' + children.length);
        if (doc.nextSibling(a) !== b)
            throw new Error('a.nextSibling should be b');
        if (doc.previousSibling(c) !== b)
            throw new Error('c.previousSibling should be b');

        doc.removeChild(parent, b);
        if (doc.nextSibling(a) !== c)
            throw new Error('after remove, a.nextSibling should be c');
        if (doc.childNodes(parent).length !== 2)
            throw new Error('should have 2 children after remove');
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

/// replaceChild
#[tokio::test]
async fn replace_child() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc    = new DocumentHandle();
        const parent = doc.createElement('div');
        const old    = doc.createElement('span');
        const newEl  = doc.createElement('p');

        doc.appendChild(parent, old);
        doc.replaceChild(parent, newEl, old);

        const children = doc.childNodes(parent);
        if (children.length !== 1)
            throw new Error('should have 1 child after replace');
        if (doc.tagName(children[0]) !== 'P')
            throw new Error('replaced child should be P, got ' + doc.tagName(children[0]));
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

/// setNodeValue 更新文本节点内容
#[tokio::test]
async fn set_node_value() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc  = new DocumentHandle();
        const text = doc.createTextNode('initial');
        doc.setNodeValue(text, 'updated');
        if (doc.nodeValue(text) !== 'updated')
            throw new Error('nodeValue should be updated');
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

/// createComment / nodeType 验证注释节点
#[tokio::test]
async fn create_comment_node() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc     = new DocumentHandle();
        const comment = doc.createComment('test comment');
        if (doc.nodeType(comment) !== 8)
            throw new Error('expected COMMENT_NODE(8)');
        if (doc.nodeValue(comment) !== 'test comment')
            throw new Error('wrong comment content');
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

/// querySelectorAll 返回匹配节点列表
#[tokio::test]
async fn query_selector_all() {
    let rt = build_headless().expect("build");

    rt.eval_module("test", r#"
        import { DocumentHandle } from 'webatom_ext_native:dom';
        const doc  = new DocumentHandle();
        const root = doc.documentNode();
        const div1 = doc.createElement('div');
        const div2 = doc.createElement('div');
        const span = doc.createElement('span');
        doc.appendChild(root, div1);
        doc.appendChild(root, div2);
        doc.appendChild(root, span);

        const divs = doc.querySelectorAll(root, 'div');
        if (divs.length !== 2)
            throw new Error('expected 2 divs, got ' + divs.length);
    "#).expect("eval_module");

    rt.run().await.expect("event loop");
}

// ── HTML 入口解析 ─────────────────────────────────────────────────────────────

/// 带 HTML 入口时 document.head / document.body 可正常访问
#[tokio::test]
async fn html_entry_head_and_body() {
    let rt = build_with_html(SIMPLE).await;

    rt.eval::<(), _>(
        "if (document.head === null) throw new Error('document.head should not be null'); \
         if (document.body === null) throw new Error('document.body should not be null');"
    ).expect("eval");

    rt.eval::<(), _>("globalThis.document = undefined").expect("cleanup");
    rt.run().await.expect("event loop");
}

/// simple.html 中 <h1 id="title"> 可被 querySelector 找到
#[tokio::test]
async fn html_entry_query_selector() {
    let rt = build_with_html(SIMPLE).await;

    rt.eval::<(), _>(
        r#"const h1 = document.querySelector('#title');
           if (h1 === null) throw new Error('should find #title via querySelector');
           if (h1.tagName !== 'H1') throw new Error('expected H1, got ' + h1.tagName);"#
    ).expect("eval");

    rt.eval::<(), _>("globalThis.document = undefined").expect("cleanup");
    rt.run().await.expect("event loop");
}

// ── classic script 执行 ───────────────────────────────────────────────────────

/// HTML 入口中的 classic script 被执行，全局变量可读
#[tokio::test]
async fn classic_script_runs_from_html_entry() {
    let rt = build_with_html(WITH_CLASSIC_SCRIPT).await;

    rt.eval::<(), _>(
        "if (globalThis.__value !== 42) \
             throw new Error('__value should be 42, got ' + globalThis.__value); \
         if (globalThis.__script_ran !== true) \
             throw new Error('__script_ran should be true');"
    ).expect("eval");

    rt.eval::<(), _>("globalThis.document = undefined").expect("cleanup");
    rt.run().await.expect("event loop");
}

// ── importmap ─────────────────────────────────────────────────────────────────

/// HTML 中的 importmap 解析后不崩溃，事件循环正常退出
#[tokio::test]
async fn importmap_entries_parsed_without_crash() {
    let rt = build_with_html(WITH_IMPORTMAP).await;

    rt.eval::<(), _>("globalThis.document = undefined").expect("cleanup");
    rt.run().await.expect("event loop should exit after importmap html");
}
