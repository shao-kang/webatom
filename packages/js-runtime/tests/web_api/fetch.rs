use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().await.unwrap()
}

// ── Test HTTP server ──────────────────────────────────────────────────────────

struct TestServer {
    port: u16,
    handle: tokio::task::JoinHandle<()>,
}

impl TestServer {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn start_server() -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(handle_conn(socket));
        }
    });

    TestServer { port, handle }
}

async fn handle_conn(socket: tokio::net::TcpStream) {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_err() {
            return;
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(val) = line.to_lowercase().trim().strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    let mut req_body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut req_body).await.ok();
    }

    let (status, content_type, resp_body) = route(&method, &path, req_body);
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         X-Custom: webatom-test\r\n\
         Connection: close\r\n\r\n{resp_body}",
        resp_body.len()
    );
    writer.write_all(response.as_bytes()).await.ok();
}

fn route(method: &str, path: &str, body: Vec<u8>) -> (&'static str, &'static str, String) {
    match (method, path) {
        (_, "/hello") => ("200 OK", "text/plain", "hello world".into()),
        (_, "/json") => (
            "200 OK",
            "application/json",
            r#"{"ok":true,"value":42}"#.into(),
        ),
        (_, "/large") => ("200 OK", "text/plain", "X".repeat(64 * 1024)),
        (_, "/status/404") => ("404 Not Found", "text/plain", "not found".into()),
        ("POST", "/echo") => (
            "200 OK",
            "text/plain",
            String::from_utf8_lossy(&body).into_owned(),
        ),
        _ => ("404 Not Found", "text/plain", "not found".into()),
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Wrap async JS so errors are always re-thrown via setTimeout, guaranteeing
/// they surface from `rt.run()` regardless of top-level-await rejection handling.
fn js_async(body: &str) -> String {
    format!("(async () => {{ {body} }})().catch(e => setTimeout(() => {{ throw e; }}, 0));")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_text() {
    let srv = start_server().await;
    let url = srv.url("/hello");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            const text = await resp.text();
            if (text !== "hello world")
                throw new Error(`expected "hello world", got "${{text}}"`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_json() {
    let srv = start_server().await;
    let url = srv.url("/json");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            const data = await resp.json();
            if (data.ok !== true)   throw new Error("data.ok !== true");
            if (data.value !== 42)  throw new Error("data.value !== 42");
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_ok_and_status() {
    let srv = start_server().await;
    let ok_url = srv.url("/hello");
    let err_url = srv.url("/status/404");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const r1 = await fetch("{ok_url}");
            if (!r1.ok || r1.status !== 200)
                throw new Error(`expected 200 ok, got ${{r1.status}}`);

            const r2 = await fetch("{err_url}");
            if (r2.ok || r2.status !== 404)
                throw new Error(`expected 404 not-ok, got ${{r2.status}}`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_post_with_body() {
    let srv = start_server().await;
    let url = srv.url("/echo");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}", {{
                method: "POST",
                body: "ping",
                headers: {{ "Content-Type": "text/plain" }},
            }});
            const text = await resp.text();
            if (text !== "ping")
                throw new Error(`echo expected "ping", got "${{text}}"`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_streaming_via_reader() {
    let srv = start_server().await;
    let url = srv.url("/large");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            const reader = resp.body.getReader();
            let total = 0;
            while (true) {{
                const {{ value, done }} = await reader.read();
                if (done) break;
                total += value.byteLength;
            }}
            if (total !== 65536)
                throw new Error(`expected 65536 bytes, got ${{total}}`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_array_buffer() {
    let srv = start_server().await;
    let url = srv.url("/hello");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            const buf = await resp.arrayBuffer();
            if (!(buf instanceof ArrayBuffer))
                throw new Error("expected ArrayBuffer");
            if (buf.byteLength !== 11)
                throw new Error(`expected 11 bytes, got ${{buf.byteLength}}`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_clone() {
    let srv = start_server().await;
    let url = srv.url("/hello");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            const copy = resp.clone();
            const [t1, t2] = await Promise.all([resp.text(), copy.text()]);
            if (t1 !== "hello world")
                throw new Error(`resp: expected "hello world", got "${{t1}}"`);
            if (t2 !== "hello world")
                throw new Error(`copy: expected "hello world", got "${{t2}}"`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn fetch_pipe_through_transform() {
    let srv = start_server().await;
    let url = srv.url("/large");
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        js_async(&format!(
            r#"
            const resp = await fetch("{url}");
            let total = 0;
            const counter = new TransformStream({{
                transform(chunk, ctrl) {{
                    total += chunk.byteLength;
                    ctrl.enqueue(chunk);
                }}
            }});
            const reader = resp.body.pipeThrough(counter).getReader();
            while (true) {{
                const {{ done }} = await reader.read();
                if (done) break;
            }}
            if (total !== 65536)
                throw new Error(`expected 65536, got ${{total}}`);
            "#
        )),
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}
