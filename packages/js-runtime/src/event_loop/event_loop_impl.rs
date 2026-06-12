use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use rquickjs::{AsyncContext, AsyncRuntime};
use tokio::sync::mpsc;

use super::handle::ActiveHandles;
use super::task::{AfterMicrotaskTask, RafTask};

pub struct FrameInfo {
    pub timestamp_ms: f64,
}

/// Shareable handle stored as context userdata.
/// Extensions access active_handles, raf_queue and frame_tx through this.
#[derive(Clone)]
pub struct EventLoopHandle {
    pub active_handles: ActiveHandles,
    pub raf_queue: Arc<Mutex<VecDeque<RafTask>>>,
    pub after_microtask_queue: Arc<Mutex<VecDeque<AfterMicrotaskTask>>>,
    pub frame_tx: mpsc::Sender<FrameInfo>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for EventLoopHandle {
    type Changed<'to> = EventLoopHandle;
}

pub struct EventLoop {
    handle: EventLoopHandle,
    runtime: AsyncRuntime,
    frame_rx: mpsc::Receiver<FrameInfo>,
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime) -> Self {
        let (frame_tx, frame_rx) = mpsc::channel(1);
        Self {
            handle: EventLoopHandle {
                active_handles: ActiveHandles::new(),
                raf_queue: Arc::new(Mutex::new(VecDeque::new())),
                after_microtask_queue: Arc::new(Mutex::new(VecDeque::new())),
                frame_tx,
            },
            runtime,
            frame_rx,
        }
    }

    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }

    /// Returns a cloneable handle suitable for storing as context userdata.
    pub fn handle(&self) -> EventLoopHandle {
        self.handle.clone()
    }

    async fn flush_raf(
        handle: &EventLoopHandle,
        context: &AsyncContext,
        timestamp_ms: f64,
    ) -> rquickjs::Result<()> {
        loop {
            let task = handle.raf_queue.lock().unwrap().pop_front();
            let Some(task) = task else { break };
            context
                .with(|ctx| {
                    let f = task.func.restore(&ctx)?;
                    let _ = f.call::<_, rquickjs::Value>((timestamp_ms,));
                    Ok::<(), rquickjs::Error>(())
                })
                .await?;
        }
        Ok(())
    }

    async fn flush_after_microtask(
        handle: &EventLoopHandle,
        context: &AsyncContext,
    ) -> rquickjs::Result<()> {
        loop {
            let task = handle.after_microtask_queue.lock().unwrap().pop_front();
            let Some(task) = task else { break };
            context.with(|ctx| (task.inner)(ctx)).await?;
        }
        Ok(())
    }

    pub async fn run(mut self, context: &AsyncContext) -> rquickjs::Result<()> {
        if self.handle.active_handles.count() == 0 {
            return Ok(());
        }
        loop {
            tokio::select! {
                _ = self.runtime.drive() => {
                    Self::flush_after_microtask(&self.handle, context).await?;
                    if self.handle.active_handles.count() == 0 { break; }
                }
                Some(frame) = self.frame_rx.recv() => {
                    Self::flush_raf(&self.handle, context, frame.timestamp_ms).await?;
                }
                _ = self.handle.active_handles.wait_idle() => {
                    break;
                }
            }
        }
        Ok(())
    }
}
