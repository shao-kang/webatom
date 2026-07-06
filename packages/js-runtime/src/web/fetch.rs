use rquickjs::{
    ArrayBuffer, Ctx, Function, Object, Persistent, Result, Value,
    module::{Declarations, Exports, ModuleDef},
};

use crate::event_loop::{HostBridge, MacroTask};
use crate::extension::Extension;

// Persistent<T> holds *mut JSRuntime which is !Send.
// MacroTask closures always run inside context.with() on the JS thread, so this is safe.
struct SendPersistent<T>(Persistent<T>);
unsafe impl<T> Send for SendPersistent<T> {}

impl<T: Clone> Clone for SendPersistent<T> {
    fn clone(&self) -> Self {
        SendPersistent(self.0.clone())
    }
}

impl<T: rquickjs::JsLifetime<'static>> SendPersistent<T> {
    fn restore<'js>(self, ctx: &Ctx<'js>) -> rquickjs::Result<T::Changed<'js>> {
        self.0.restore(ctx)
    }
}

fn do_fetch<'js>(
    ctx: Ctx<'js>,
    url: String,
    options: Object<'js>,
    resolve: Function<'js>,
    reject: Function<'js>,
    callbacks: Object<'js>,
    host: &HostBridge,
) -> Result<()> {
    let on_chunk: Function = callbacks.get("onChunk")?;
    let on_done: Function = callbacks.get("onDone")?;
    let on_stream_error: Function = callbacks.get("onStreamError")?;
    let keepalive = match host.runtime.keepalive.acquire() {
        Some(g) => g,
        None => return Ok(()),
    };

    let method: String = options
        .get::<_, String>("method")
        .unwrap_or_else(|_| "GET".to_string())
        .to_uppercase();

    let body: Option<String> = options.get::<_, String>("body").ok();

    let mut req_headers: Vec<(String, String)> = Vec::new();
    if let Ok(headers_val) = options.get::<_, Value>("headers") {
        if let Some(headers_obj) = headers_val.as_object() {
            for prop in headers_obj.props::<String, String>() {
                if let Ok((k, v)) = prop {
                    req_headers.push((k, v));
                }
            }
        }
    }

    let resolve = SendPersistent(Persistent::save(&ctx, resolve));
    let reject = SendPersistent(Persistent::save(&ctx, reject));
    let on_chunk = SendPersistent(Persistent::save(&ctx, on_chunk));
    let on_done = SendPersistent(Persistent::save(&ctx, on_done));
    let on_stream_error = SendPersistent(Persistent::save(&ctx, on_stream_error));
    let task_tx = host.io.task_tx.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let method_parsed: reqwest::Method = method.parse().unwrap_or(reqwest::Method::GET);
        let mut builder = client.request(method_parsed, &url);

        for (k, v) in req_headers {
            builder = builder.header(k, v);
        }

        if let Some(b) = body {
            builder = builder.body(b);
        }

        match builder.send().await {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
                let ok = resp.status().is_success();
                let final_url = resp.url().to_string();
                let resp_headers: Vec<(String, String)> = resp
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();

                // Resolve with headers immediately; body arrives via on_chunk/on_done
                let resolve_task: MacroTask = Box::new(move |ctx| {
                    let headers_obj = Object::new(ctx.clone())?;
                    for (k, v) in &resp_headers {
                        headers_obj.set(k.as_str(), v.as_str())?;
                    }
                    let init = Object::new(ctx.clone())?;
                    init.set("ok", ok)?;
                    init.set("status", status)?;
                    init.set("statusText", status_text.as_str())?;
                    init.set("url", final_url.as_str())?;
                    init.set("headers", headers_obj)?;
                    resolve.restore(&ctx)?.call::<_, ()>((init,))?;
                    Ok(())
                });
                if task_tx.send(resolve_task).await.is_err() {
                    return;
                }

                // Stream body chunks
                loop {
                    match resp.chunk().await {
                        Ok(Some(chunk)) => {
                            let bytes = chunk.to_vec();
                            let on_chunk_clone = on_chunk.clone();
                            let task: MacroTask = Box::new(move |ctx| {
                                let buffer = ArrayBuffer::new(ctx.clone(), bytes)?;
                                on_chunk_clone.restore(&ctx)?.call::<_, ()>((buffer,))?;
                                Ok(())
                            });
                            if task_tx.send(task).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            let task: MacroTask = Box::new(move |ctx| {
                                drop(on_chunk);
                                drop(on_stream_error);
                                on_done.restore(&ctx)?.call::<_, ()>(())?;
                                drop(keepalive);
                                Ok(())
                            });
                            task_tx.send(task).await.ok();
                            return;
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            let task: MacroTask = Box::new(move |ctx| {
                                drop(on_chunk);
                                drop(on_done);
                                on_stream_error.restore(&ctx)?.call::<_, ()>((msg.as_str(),))?;
                                drop(keepalive);
                                Ok(())
                            });
                            task_tx.send(task).await.ok();
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let task: MacroTask = Box::new(move |ctx| {
                    drop(on_chunk);
                    drop(on_done);
                    drop(on_stream_error);
                    reject.restore(&ctx)?.call::<_, ()>((msg.as_str(),))?;
                    drop(keepalive);
                    Ok(())
                });
                task_tx.send(task).await.ok();
            }
        }
    });

    Ok(())
}

pub struct FetchModule;

impl ModuleDef for FetchModule {
    fn declare(decl: &Declarations) -> Result<()> {
        decl.declare("_fetch")?;
        decl.declare("_decodeUtf8")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        let host = ctx
            .userdata::<HostBridge>()
            .expect("HostBridge not registered")
            .clone();

        let fetch_fn = Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>,
                  url: String,
                  options: Object<'js>,
                  resolve: Function<'js>,
                  reject: Function<'js>,
                  callbacks: Object<'js>| {
                do_fetch(ctx, url, options, resolve, reject, callbacks, &host)
            },
        )?;

        let decode_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx<'js>, buffer: ArrayBuffer<'js>| -> Result<String> {
                Ok(String::from_utf8_lossy(buffer.as_bytes().unwrap_or_default()).into_owned())
            },
        )?;

        exports.export("_fetch", fetch_fn)?;
        exports.export("_decodeUtf8", decode_fn)?;
        Ok(())
    }
}

pub struct FetchExtension;

impl Extension for FetchExtension {
    fn name(&self) -> &'static str {
        "fetch"
    }

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &["@webatom/fetch"]
    }

    fn install(&self, ctx: &Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        rquickjs::Module::declare_def::<FetchModule, _>(ctx.clone(), "@webatom/fetch")?;
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(JS_GLUE)
    }
}

const JS_GLUE: &str = r#"import { _fetch, _decodeUtf8 } from '@webatom/fetch';

async function _consumeBody(body) {
    const reader = body.getReader();
    const chunks = [];
    let totalLength = 0;
    while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        chunks.push(value);
        totalLength += value.byteLength;
    }
    reader.releaseLock();
    const result = new Uint8Array(totalLength);
    let offset = 0;
    for (const chunk of chunks) {
        result.set(chunk, offset);
        offset += chunk.byteLength;
    }
    return result;
}

class Response {
    constructor(init, body) {
        this.ok = init.ok;
        this.status = init.status;
        this.statusText = init.statusText;
        this.url = init.url;
        this.headers = init.headers;
        this.body = body;
        this.bodyUsed = false;
    }

    async text() {
        this.bodyUsed = true;
        const bytes = await _consumeBody(this.body);
        return _decodeUtf8(bytes.buffer);
    }

    async json() {
        return JSON.parse(await this.text());
    }

    async arrayBuffer() {
        this.bodyUsed = true;
        const bytes = await _consumeBody(this.body);
        return bytes.buffer;
    }

    async bytes() {
        this.bodyUsed = true;
        return _consumeBody(this.body);
    }

    clone() {
        if (this.bodyUsed) throw new TypeError('Response body has already been consumed');
        const [b1, b2] = this.body.tee();
        this.body = b1;
        return new Response({
            ok: this.ok,
            status: this.status,
            statusText: this.statusText,
            url: this.url,
            headers: this.headers,
        }, b2);
    }
}

function fetch(url, options) {
    return new Promise((resolve, reject) => {
        let controller;
        const body = new ReadableStream({
            start(c) { controller = c; }
        });

        _fetch(
            String(url),
            options || {},
            (init) => resolve(new Response(init, body)),
            (msg) => reject(new TypeError(msg)),
            {
                onChunk: (chunk) => controller.enqueue(new Uint8Array(chunk)),
                onDone: () => controller.close(),
                onStreamError: (msg) => controller.error(new TypeError(msg)),
            },
        );
    });
}

globalThis.fetch = fetch;
globalThis.Response = Response;
"#;
