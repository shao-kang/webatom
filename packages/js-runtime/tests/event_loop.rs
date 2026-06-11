use std::collections::HashMap;
use rquickjs::{CatchResultExt, Context, Function, Runtime};

fn setup() -> (js_runtime::EventLoop, Context) {
    let rt = Runtime::new().unwrap();
    js_runtime::setup_module_system(&rt, HashMap::new());
    let el = js_runtime::EventLoop::new(rt);
    let ctx = Context::full(el.runtime()).unwrap();
    (el, ctx)
}

// ── setTimeout ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn settimeout_fires() {
    use std::sync::{Arc, Mutex};

    let (mut el, ctx) = setup();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.globals()
            .set(
                "__markFired",
                Function::new(ctx.clone(), move || {
                    *fired_clone.lock().unwrap() = true;
                    Ok::<(), rquickjs::Error>(())
                })
                .unwrap(),
            )
            .unwrap();
        ctx.eval::<(), _>("setTimeout(() => __markFired(), 10)")
            .catch(&ctx)
            .unwrap();
    });

    el.run(&ctx).await.unwrap();
    assert!(*fired.lock().unwrap(), "setTimeout callback was not fired");
}

#[tokio::test(flavor = "current_thread")]
async fn settimeout_zero_fires() {
    use std::sync::{Arc, Mutex};

    let (mut el, ctx) = setup();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.globals()
            .set(
                "__mark",
                Function::new(ctx.clone(), move || {
                    *fired_clone.lock().unwrap() = true;
                    Ok::<(), rquickjs::Error>(())
                })
                .unwrap(),
            )
            .unwrap();
        ctx.eval::<(), _>("setTimeout(() => __mark(), 0)")
            .catch(&ctx)
            .unwrap();
    });

    el.run(&ctx).await.unwrap();
    assert!(*fired.lock().unwrap(), "setTimeout(fn, 0) was not fired");
}

// ── clearTimeout ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn cleartimeout_cancels() {
    use std::sync::{Arc, Mutex};

    let (mut el, ctx) = setup();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.globals()
            .set(
                "__markFired",
                Function::new(ctx.clone(), move || {
                    *fired_clone.lock().unwrap() = true;
                    Ok::<(), rquickjs::Error>(())
                })
                .unwrap(),
            )
            .unwrap();
        ctx.eval::<(), _>(
            "const id = setTimeout(() => __markFired(), 10); clearTimeout(id);",
        )
        .catch(&ctx)
        .unwrap();
    });

    el.run(&ctx).await.unwrap();
    assert!(
        !*fired.lock().unwrap(),
        "clearTimeout did not prevent the callback"
    );
}

// ── setInterval ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn setinterval_fires_repeatedly_then_clears() {
    use std::sync::{Arc, Mutex};

    let (mut el, ctx) = setup();
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.globals()
            .set(
                "__inc",
                Function::new(ctx.clone(), move || {
                    *count_clone.lock().unwrap() += 1;
                    Ok::<(), rquickjs::Error>(())
                })
                .unwrap(),
            )
            .unwrap();
        ctx.eval::<(), _>(
            "let n = 0; const id = setInterval(() => { __inc(); if (++n >= 3) clearInterval(id); }, 10);",
        )
        .catch(&ctx)
        .unwrap();
    });

    el.run(&ctx).await.unwrap();
    assert_eq!(
        *count.lock().unwrap(),
        3,
        "setInterval should fire exactly 3 times before clearInterval"
    );
}

// ── Microtask ordering ────────────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn promise_microtask_executes() {
    let (mut el, ctx) = setup();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.eval::<(), _>(
            "globalThis.__resolved = false; Promise.resolve().then(() => { globalThis.__resolved = true; });",
        )
        .catch(&ctx)
        .unwrap();
    });

    el.run(&ctx).await.unwrap();

    let resolved: bool = ctx.with(|ctx| ctx.globals().get("__resolved").unwrap());
    assert!(resolved, "Promise microtask was not executed");
}

#[tokio::test(flavor = "current_thread")]
async fn microtask_runs_before_next_macrotask() {
    let (mut el, ctx) = setup();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.eval::<(), _>(
            r#"
            globalThis.__order = [];
            setTimeout(() => __order.push('macro1'), 0);
            Promise.resolve().then(() => __order.push('micro'));
            setTimeout(() => __order.push('macro2'), 0);
            "#,
        )
        .catch(&ctx)
        .unwrap();
    });

    el.run(&ctx).await.unwrap();

    // flush_microtasks() runs before any macrotask:
    //   micro → macro1 → macro2
    let order: Vec<String> = ctx.with(|ctx| ctx.eval("__order").unwrap());
    assert_eq!(order, vec!["micro", "macro1", "macro2"]);
}

// ── Multiple timers ordering ──────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn timers_fire_in_deadline_order() {
    let (mut el, ctx) = setup();

    ctx.with(|ctx| {
        js_runtime::setup_web_api(&ctx, el.timers()).unwrap();
        ctx.eval::<(), _>(
            r#"
            globalThis.__order = [];
            setTimeout(() => __order.push('b'), 20);
            setTimeout(() => __order.push('a'), 10);
            "#,
        )
        .catch(&ctx)
        .unwrap();
    });

    el.run(&ctx).await.unwrap();

    let order: Vec<String> = ctx.with(|ctx| ctx.eval("__order").unwrap());
    assert_eq!(order, vec!["a", "b"], "shorter deadline should fire first");
}
