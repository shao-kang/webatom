use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rquickjs::{AsyncContext, AsyncRuntime};

use super::task::{MacroTask, RafTask};

pub struct EventLoop {
    runtime: AsyncRuntime,
    macro_queue: Rc<RefCell<VecDeque<MacroTask>>>,
    #[allow(dead_code)]
    raf_queue: Rc<RefCell<VecDeque<RafTask>>>,
}

impl EventLoop {
    pub fn new(runtime: AsyncRuntime) -> Self {
        Self {
            runtime,
            macro_queue: Rc::new(RefCell::new(VecDeque::new())),
            raf_queue: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }

    pub fn macro_sender(&self) -> Rc<RefCell<VecDeque<MacroTask>>> {
        self.macro_queue.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn raf_sender(&self) -> Rc<RefCell<VecDeque<RafTask>>> {
        self.raf_queue.clone()
    }

    pub async fn run(&self, context: &AsyncContext) -> rquickjs::Result<()> {
        loop {
            // Wait until all spawn'd futures (timers, microtasks, etc.) are fully drained.
            self.runtime.idle().await;

            // Drain any MacroTasks enqueued by the settled futures above.
            let mut executed_any = false;
            loop {
                let task = self.macro_queue.borrow_mut().pop_front();
                let Some(task) = task else { break };
                executed_any = true;
                context
                    .with(|ctx| {
                        let f = task.func.restore(&ctx)?;
                        let _ = f.call::<_, rquickjs::Value>(());
                        Ok::<(), rquickjs::Error>(())
                    })
                    .await?;
            }

            // No macro tasks remain after idle → nothing left to do.
            if !executed_any {
                break;
            }
        }
        Ok(())
    }
}
