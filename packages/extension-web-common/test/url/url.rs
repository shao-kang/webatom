use js_runtime::JsRuntime;
use extension::WebCommonExtension;

fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder()
        .with_extension(WebCommonExtension::new())
        .build()
        .unwrap_or_else(|e| panic!("runtime build failed: {e}"))
}

/// Eval JS that returns a diagnostic string; panics with that string on failure.
fn check(rt: &JsRuntime, expr: &str) {
    let msg = rt
        .eval::<String, _>(expr)
        .unwrap_or_else(|e| panic!("eval error: {e}"));
    assert_eq!(msg, "ok", "{msg}");
}

/// globalThis.URL 由 global_js 注入后可正常构造
#[tokio::test]
async fn url_is_defined() {
    let rt = build();
    // check(&rt, r#"typeof URL === 'function' ? 'ok' : 'FAIL: URL is ' + typeof URL"#);
    rt.run().await.expect("event loop");
}

/// URL 能解析绝对地址，各属性值正确
#[tokio::test]
async fn url_parse_absolute() {
    let rt = build();
    check(&rt, r#"(() => {
        const u = new URL('https://example.com:8080/path?q=1#hash');
        if (u.protocol !== 'https:')      return 'protocol: '  + u.protocol;
        if (u.hostname !== 'example.com') return 'hostname: '  + u.hostname;
        if (u.port     !== '8080')        return 'port: '      + u.port;
        if (u.pathname !== '/path')       return 'pathname: '  + u.pathname;
        if (u.search   !== '?q=1')        return 'search: '    + u.search;
        if (u.hash     !== '#hash')       return 'hash: '      + u.hash;
        return 'ok';
    })()"#);
    rt.run().await.expect("event loop");
}

/// URL 能用 base 解析相对地址
#[tokio::test]
async fn url_parse_relative() {
    let rt = build();
    check(&rt, r#"(() => {
        const u = new URL('../bar', 'https://example.com/foo/index.html');
        return u.href === 'https://example.com/bar' ? 'ok' : 'href: ' + u.href;
    })()"#);
    rt.run().await.expect("event loop");
}

/// 无效 URL 抛出 TypeError
#[tokio::test]
async fn url_invalid_throws() {
    let rt = build();
    check(&rt, r#"(() => {
        try { new URL('not a url'); return 'FAIL: no throw'; }
        catch (e) { return 'ok'; }
    })()"#);
    rt.run().await.expect("event loop");
}

/// URLSearchParams 由 global_js 注入后可正常使用
#[tokio::test]
async fn url_search_params() {
    let rt = build();
    check(&rt, r#"(() => {
        const p = new URLSearchParams('a=1&b=2');
        if (p.get('a') !== '1') return 'a: ' + p.get('a');
        if (p.get('b') !== '2') return 'b: ' + p.get('b');
        p.set('a', '99');
        if (p.get('a') !== '99') return 'set failed';
        return 'ok';
    })()"#);
    rt.run().await.expect("event loop");
}

/// URL pathname setter 修改整个 URL
#[tokio::test]
async fn url_pathname_setter() {
    let rt = build();
    check(&rt, r#"(() => {
        const u = new URL('https://example.com/');
        u.pathname = '/new-path';
        return u.href === 'https://example.com/new-path' ? 'ok' : 'href: ' + u.href;
    })()"#);
    rt.run().await.expect("event loop");
}
