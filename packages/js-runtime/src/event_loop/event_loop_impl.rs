use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rquickjs::{AsyncContext, AsyncRuntime};
use tokio::sync::mpsc;

use super::handle::ActiveHandles;
use super::task::RafTask;

pub struct FrameInfo {
    pub timestamp_ms: f64,
}

pub struct EventLoop {
    runtime: AsyncRuntime,
    active_handles: ActiveHandles,
    raf_queue: Rc<RefCell<VecDeque<RafTask>>>,
    frame_tx: mpsc::Sender<FrameInfo>,
    frame_rx: mpsc::Receiver<FrameInfo>,
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime) -> Self {
        let (frame_tx, frame_rx) = mpsc::channel(1);
        Self {
            runtime,
            active_handles: ActiveHandles::new(),
            raf_queue: Rc::new(RefCell::new(VecDeque::new())),
            frame_tx,
            frame_rx,
        }
    }

    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }

    pub fn active_handles(&self) -> ActiveHandles {
        self.active_handles.clone()
    }

    pub fn raf_sender(&self) -> Rc<RefCell<VecDeque<RafTask>>> {
        self.raf_queue.clone()
    }

    /// Cloneable sender given to the render layer to signal each completed frame.
    pub fn frame_sender(&self) -> mpsc::Sender<FrameInfo> {
        self.frame_tx.clone()
    }

    async fn flush_raf(
        raf_queue: &Rc<RefCell<VecDeque<RafTask>>>,
        context: &AsyncContext,
        timestamp_ms: f64,
    ) -> rquickjs::Result<()> {
        loop {
            let task = raf_queue.borrow_mut().pop_front();
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

    pub async fn run(mut self, context: &AsyncContext) -> rquickjs::Result<()> {
        if self.active_handles.count() == 0 {
            return Ok(());
        }
        loop {
            tokio::select! {
                _ = self.runtime.drive() => {
                    if self.active_handles.count() == 0 { break; }
                }
                Some(frame) = self.frame_rx.recv() => {
                    Self::flush_raf(&self.raf_queue, context, frame.timestamp_ms).await?;
                }
                _ = self.active_handles.wait_idle() => {
                    break;
                }
            }
        }
        Ok(())
    }
}
