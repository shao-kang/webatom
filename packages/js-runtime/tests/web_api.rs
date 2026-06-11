use js_runtime::JsRuntime;

fn build() -> JsRuntime {
    JsRuntime::builder().build().unwrap()
}

#[test]
fn setup_web_api_succeeds() {
    build();
}

#[test]
fn console_object_is_available() {
    let rt = build();
    let result: bool = rt.eval("typeof globalThis.console === 'object'").unwrap();
    assert!(result);
}

#[test]
fn console_has_expected_methods() {
    let rt = build();
    let result: bool = rt
        .eval("['log','info','warn','error'].every(m => typeof console[m] === 'function')")
        .unwrap();
    assert!(result);
}

#[test]
fn console_log_does_not_panic() {
    let rt = build();
    rt.eval::<()>("console.log('hello', 42, true)").unwrap();
}

#[test]
fn console_warn_error_do_not_panic() {
    let rt = build();
    rt.eval::<()>("console.warn('warn msg'); console.error('error msg')")
        .unwrap();
}
