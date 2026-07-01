use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::{AsyncContext, AsyncRuntime};
use tokio::sync::{mpsc, watch};

use super::task::{MacroTask, RafTask};
use super::handle::{EventLoopHandle, HostBridge, KeepAliveCount, RuntimeBridge, RuntimeIo, SchedulerBridge};
use super::idle::{IdleQueue, IdleScheduler};
use super::render_scheduler::{HeadlessRenderScheduler, RenderScheduler, VsyncSignal};

pub struct EventLoop {
    pub(crate) host: HostBridge,
    runtime: AsyncRuntime,
    task_rx: mpsc::Receiver<MacroTask>,
    vsync_rx: watch::Receiver<Option<VsyncSignal>>,
    raf_queue: Arc<Mutex<VecDeque<RafTask>>>,
    idle_queue: Arc<Mutex<IdleQueue>>,
    idle_scheduler: IdleScheduler,
    // Phase 0 stub; will be polled in the run loop once vsync is wired.
    #[allow(dead_code)]
    render_scheduler: Box<dyn RenderScheduler>,
    handle: EventLoopHandle,
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime) -> Self {
        let (task_tx, task_rx) = mpsc::channel(64);
        let (vsync_tx, vsync_rx) = watch::channel(None);
        let vsync_tx = Arc::new(vsync_tx);

        let keepalive = KeepAliveCount::new();
        let raf_queue: Arc<Mutex<VecDeque<RafTask>>> = Arc::new(Mutex::new(VecDeque::new()));
        let idle_queue: Arc<Mutex<IdleQueue>> = Arc::new(Mutex::new(IdleQueue::new()));

        let host = HostBridge {
            runtime: RuntimeBridge { keepalive: keepalive.clone() },
            io: RuntimeIo { task_tx: task_tx.clone() },
            scheduler: SchedulerBridge {
                raf: raf_queue.clone(),
                idle: idle_queue.clone(),
            },
        };

        let handle = EventLoopHandle {
            keepalive_count: keepalive,
            task_tx,
            vsync_tx,
        };

        Self {
            host,
            runtime,
            task_rx,
            vsync_rx,
            raf_queue,
            idle_queue,
            idle_scheduler: IdleScheduler::new(),
            render_scheduler: Box::new(HeadlessRenderScheduler),
            handle,
        }
    }

    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }

    pub fn handle(&self) -> EventLoopHandle {
        self.handle.clone()
    }

    pub async fn run(&mut self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        // Flush any microtasks queued before run() (e.g. from eval_module).
        self.microtask_checkpoint(ctx).await?;
        self.drain_events(ctx).await?;

        // Fast path: nothing to wait for (no keepalives, no pending events).
        if self.should_exit() {
            return Ok(());
        }

        loop {
            // Drive the QJS job queue and spawned futures, OR exit when all
            // keepalives are released. drive() parks when there is no pending
            // QJS work, so we must race it against wait_idle() to avoid
            // blocking forever when the last keepalive drops while drive() is
            // parked.
            tokio::select! {
                _ = self.runtime.drive() => {}
                Some(task) = self.task_rx.recv() => {
                    // A MacroTask (e.g. timer callback) arrived while drive() was
                    // parked. Execute it immediately so its keepalive is released;
                    // drain_events below will pick up any remaining tasks.
                    self.process_event(task, ctx).await?;
                    self.microtask_checkpoint(ctx).await?;
                }
                _ = self.handle.keepalive_count.wait_idle() => {
                    // Flush one last time before exiting.
                    self.drain_events(ctx).await?;
                    self.microtask_checkpoint(ctx).await?;
                    break;
                }
            }

            self.microtask_checkpoint(ctx).await?;
            self.drain_events(ctx).await?;

            // Flush idle callbacks that have timed out.
            ctx.with(|qctx| self.idle_scheduler.flush_timed_out(qctx)).await?;

            // Check vsync (headless: always skipped in Phase 0).
            if self.vsync_rx.has_changed().unwrap_or(false) {
                let signal = self.vsync_rx.borrow_and_update().clone();
                if let Some(sig) = signal {
                    self.process_vsync(ctx, sig).await?;
                }
            }

            if self.should_exit() {
                break;
            }

            // Drain idle inbox and run untimed callbacks when otherwise idle.
            {
                let mut inbox = self.idle_queue.lock().unwrap();
                self.idle_scheduler.drain_inbox(&mut inbox);
            }
            if !self.idle_scheduler.is_empty() {
                ctx.with(|qctx| {
                    self.idle_scheduler.flush(qctx, Duration::from_millis(5))
                })
                .await?;
                self.microtask_checkpoint(ctx).await?;
            }

            // Yield so Tokio can service I/O before the next iteration.
            tokio::task::yield_now().await;
        }

        Ok(())
    }

    /// Drive the QJS job queue until it is empty.
    async fn microtask_checkpoint(&self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        ctx.with(|qctx| {
            while qctx.execute_pending_job() {}
            Ok::<(), rquickjs::Error>(())
        })
        .await
    }

    /// Drain all pending macro-tasks from the channel without blocking.
    async fn drain_events(&mut self, ctx: &AsyncContext) -> rquickjs::Result<()> {
        loop {
            match self.task_rx.try_recv() {
                Ok(task) => {
                    self.process_event(task, ctx).await?;
                    self.microtask_checkpoint(ctx).await?;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    async fn process_event(
        &mut self,
        task: MacroTask,
        ctx: &AsyncContext,
    ) -> rquickjs::Result<()> {
        ctx.with(move |qctx| {
            use rquickjs::CatchResultExt;
            if let Err(e) = task(qctx.clone()).catch(&qctx) {
                tracing::error!("JS uncaught exception: {e}");
                // Don't re-throw — match browser behavior where uncaught
                // exceptions don't stop other scripts from running.
            }
            Ok(())
        }).await
    }

    async fn process_vsync(
        &mut self,
        ctx: &AsyncContext,
        signal: VsyncSignal,
    ) -> rquickjs::Result<()> {
        loop {
            let task = self.raf_queue.lock().unwrap().pop_front();
            let Some(task) = task else { break };
            ctx.with(|qctx| (task.callback)(qctx, signal.timestamp_ms)).await?;
            self.microtask_checkpoint(ctx).await?;
        }
        Ok(())
    }

    fn should_exit(&self) -> bool {
        self.handle.keepalive_count.count() <= 0
            && self.task_rx.is_empty()
    }
}
