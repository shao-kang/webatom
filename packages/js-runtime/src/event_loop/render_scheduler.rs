use std::task::{Context, Poll};

/// Signal delivered when the render pipeline is ready for a new frame.
#[derive(Clone)]
pub struct VsyncSignal {
    pub frame_id: u64,
    pub timestamp_ms: f64,
}

/// Trait implemented by platform render schedulers.
/// The event loop polls this each iteration to detect vsync events.
pub trait RenderScheduler: Send + 'static {
    fn poll_vsync(&mut self, cx: &mut Context<'_>) -> Poll<VsyncSignal>;
}

/// Phase 0 stub: never fires vsync.
/// Used in headless / test environments where no renderer is attached.
pub struct HeadlessRenderScheduler;

impl RenderScheduler for HeadlessRenderScheduler {
    fn poll_vsync(&mut self, _cx: &mut Context<'_>) -> Poll<VsyncSignal> {
        Poll::Pending
    }
}
