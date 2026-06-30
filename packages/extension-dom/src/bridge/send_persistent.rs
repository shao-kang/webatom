use rquickjs::{Ctx, Persistent, Result};

/// `Persistent<T>` wrapped with unsafe Send/Sync so values can cross thread boundaries
/// in `Arc<Mutex<...>>`. Safe because rquickjs pins the JSRuntime to a single thread
/// and we only restore values on that thread.
#[derive(Clone)]
pub(crate) struct SendPersistent<T>(pub Persistent<T>);
unsafe impl<T> Send for SendPersistent<T> {}
unsafe impl<T> Sync for SendPersistent<T> {}

impl<T: rquickjs::JsLifetime<'static> + Clone> SendPersistent<T> {
    pub fn restore<'js>(&self, ctx: &Ctx<'js>) -> Result<T::Changed<'js>> {
        self.0.clone().restore(ctx)
    }
}
