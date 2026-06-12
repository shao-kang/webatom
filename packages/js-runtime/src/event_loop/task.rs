use rquickjs::{Ctx, Function, Persistent};

#[allow(dead_code)]
pub struct RafTask {
    pub id: u32,
    pub func: Persistent<Function<'static>>,
}

pub struct AfterMicrotaskTask {
    pub inner: Box<dyn for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<()> + Send + 'static>,
}

impl AfterMicrotaskTask {
    pub fn from_js(func: Persistent<Function<'static>>) -> Self {
        Self {
            inner: Box::new(move |ctx| {
                let f = func.restore(&ctx)?;
                let _ = f.call::<_, rquickjs::Value>(());
                Ok(())
            }),
        }
    }

    pub fn from_rust<F>(f: F) -> Self
    where
        F: for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<()> + Send + 'static,
    {
        Self { inner: Box::new(f) }
    }
}
