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
    host: &HostBridge,
) -> Result<()> {
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
            Ok(resp) => {
                let status = resp.status().as_u16();
                let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
                let ok = resp.status().is_success();
                let final_url = resp.url().to_string();
                let resp_headers: Vec<(String, String)> = resp
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();

                match resp.bytes().await {
                    Ok(bytes) => {
                        let bytes = bytes.to_vec();
                        let task: MacroTask = Box::new(move |ctx| {
                            let text = String::from_utf8_lossy(&bytes).into_owned();
                            let buffer = ArrayBuffer::new(ctx.clone(), bytes)?;

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
                            init.set("_text", text.as_str())?;
                            init.set("_buffer", buffer)?;

                            resolve.restore(&ctx)?.call::<_, ()>((init,))?;
                            drop(keepalive);
                            Ok(())
                        });
                        task_tx.send(task).await.ok();
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        let task: MacroTask = Box::new(move |ctx| {
                            reject.restore(&ctx)?.call::<_, ()>((msg.as_str(),))?;
                            drop(keepalive);
                            Ok(())
                        });
                        task_tx.send(task).await.ok();
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let task: MacroTask = Box::new(move |ctx| {
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
                  reject: Function<'js>| {
                do_fetch(ctx, url, options, resolve, reject, &host)
            },
        )?;

        exports.export("_fetch", fetch_fn)?;
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

const JS_GLUE: &str = r#"import { _fetch } from '@webatom/fetch';

class Response {
    constructor(init) {
        this.ok = init.ok;
        this.status = init.status;
        this.statusText = init.statusText;
        this.url = init.url;
        this.headers = init.headers;
        this._text = init._text;
        this._buffer = init._buffer;
    }

    text() { return Promise.resolve(this._text); }
    json() { return this.text().then(JSON.parse); }
    arrayBuffer() { return Promise.resolve(this._buffer); }

    clone() {
        return new Response({
            ok: this.ok, status: this.status, statusText: this.statusText,
            url: this.url, headers: this.headers,
            _text: this._text, _buffer: this._buffer,
        });
    }
}

function fetch(url, options) {
    return new Promise((resolve, reject) => {
        _fetch(String(url), options || {}, resolve, (msg) => reject(new TypeError(msg)));
    });
}

globalThis.fetch = fetch;
globalThis.Response = Response;
"#;
