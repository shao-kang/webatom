use std::sync::Arc;

use js_runtime::JsRuntime;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use webatom_blitz_msg::create_channels;
use webatom_extension_dom::{DomExtension, DomExtensionState, html_entry::HtmlEntry};
use webatom_extension_web_common::WebCommonExtension;
// ── JS console → clean stdout/stderr ─────────────────────────────────────

struct JsConsoleLayer;

struct MsgVisitor(Option<String>);

impl tracing::field::Visit for MsgVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = Some(format!("{value:?}"));
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = Some(value.to_string());
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for JsConsoleLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let target = event.metadata().target();
        if !target.starts_with("web_console") {
            return;
        }
        let mut v = MsgVisitor(None);
        event.record(&mut v);
        let msg = v.0.unwrap_or_default();
        match target {
            "web_console_warn"  => eprintln!("\x1b[33m[warn]\x1b[0m  {msg}"),
            "web_console_error" => eprintln!("\x1b[31m[error]\x1b[0m {msg}"),
            "web_console_debug" => println!("\x1b[90m[debug]\x1b[0m {msg}"),
            _                   => println!("{msg}"),
        }
    }
}

// ── entry ─────────────────────────────────────────────────────────────────

fn main() {
    tracing_subscriber::registry()
        .with(JsConsoleLayer)
        .with(
            tracing_subscriber::fmt::layer().with_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
            ),
        )
        .init();

    let (js_side, blitz_side) = create_channels();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let entry: Arc<HtmlEntry> = match std::env::args().nth(1) {
                Some(path) => HtmlEntry::load_or_error(path, None).await,
                None => Arc::new(HtmlEntry::new(
                    format!("file://{}/assets/index.html", env!("CARGO_MANIFEST_DIR")),
                    include_str!("../assets/index.html"),
                )),
            };

            let state = DomExtensionState::new(js_side).with_entry(entry);

            let mut js_rt = JsRuntime::builder()
                .with_extension(WebCommonExtension::new())
                .with_extension(DomExtension::with_state(state))
                .build()
                .expect("JS 运行时初始化失败");
            // let _: () = js_rt.eval("console.log('hello worldAtom')");

            if let Err(e) = js_rt.run().await {
                eprintln!("JS 事件循环错误: {e}");
                let mut source = std::error::Error::source(&e);
                while let Some(cause) = source {
                    eprintln!("  caused by: {cause}");
                    source = cause.source();
                }
                eprintln!("  debug: {e:?}");
            }
        });
    });

    webatom_blitz::run(blitz_side);
}
