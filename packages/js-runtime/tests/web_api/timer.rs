use js_runtime::JsRuntime;

async fn build() -> JsRuntime {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    JsRuntime::builder().build().await.unwrap()
}

#[tokio::test]
async fn set_timeout_fires() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        console.log("hello");
        let fired = false;
        setTimeout(() => { fired = true; }, 10);
        setTimeout(() => {
            if (!fired) throw new Error("setTimeout callback did not fire");
        }, 50);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn set_timeout_zero_delay_fires() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let fired = false;
        setTimeout(() => { fired = true; }, 0);
        setTimeout(() => {
            if (!fired) throw new Error("zero-delay setTimeout did not fire");
        }, 10);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn clear_timeout_cancels() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let fired = false;
        const id = setTimeout(() => { fired = true; }, 10);
        clearTimeout(id);
        setTimeout(() => {
            if (fired) throw new Error("timer fired after clearTimeout");
        }, 50);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn clear_timeout_nonexistent_id_does_not_throw() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        const id = setTimeout(() => {}, 10);
        clearTimeout(id);
        clearTimeout(id);    // double-clear
        clearTimeout(99999); // nonexistent
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn set_timeout_ordering_by_delay() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        const order = [];
        setTimeout(() => order.push("B"), 50);
        setTimeout(() => order.push("A"), 10);
        setTimeout(() => {
            if (order[0] !== "A" || order[1] !== "B")
                throw new Error(`wrong order: ${JSON.stringify(order)}`);
        }, 200);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn multiple_timers_all_fire() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let count = 0;
        setTimeout(() => count++, 10);
        setTimeout(() => count++, 20);
        setTimeout(() => count++, 30);
        setTimeout(() => {
            if (count !== 3) throw new Error(`expected 3, got ${count}`);
        }, 100);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn nested_set_timeout() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        const order = [];
        setTimeout(() => {
            order.push(1);
            setTimeout(() => {
                order.push(2);
                if (order[0] !== 1 || order[1] !== 2)
                    throw new Error(`wrong order: ${JSON.stringify(order)}`);
            }, 10);
        }, 10);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn set_interval_fires_multiple_times() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let count = 0;
        const id = setInterval(() => {
            count++;
            if (count >= 3) clearInterval(id);
        }, 10);
        setTimeout(() => {
            if (count !== 3) throw new Error(`expected 3 firings, got ${count}`);
        }, 200);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn clear_interval_stops_firing() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let count = 0;
        const id = setInterval(() => {
            count++;
            if (count === 2) clearInterval(id);
        }, 10);
        setTimeout(() => {
            if (count !== 2) throw new Error(`expected exactly 2 firings, got ${count}`);
        }, 200);
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}

#[tokio::test]
async fn set_timeout_callback_runs_after_sync_code() {
    let mut rt = build().await;
    rt.eval_module(
        "test.js",
        r#"
        let syncDone = false;
        setTimeout(() => {
            if (!syncDone) throw new Error("timer ran before sync code finished");
        }, 0);
        syncDone = true;
        "#,
    )
    .await
    .unwrap();
    rt.run().await.unwrap();
}
