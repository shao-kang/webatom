use std::collections::HashMap;

use tokio::sync::oneshot;

pub enum TaskEntry {
    /// Cancellable via a oneshot channel (timers, fetch, …).
    Bridge(oneshot::Sender<()>),
    /// Tracked in the scheduler queues (RAF, idle).
    Scheduler,
}

pub struct TaskRegistry {
    id_counter: i32,
    tasks: HashMap<i32, TaskEntry>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            id_counter: 0,
            tasks: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> i32 {
        self.id_counter = self.id_counter.wrapping_add(1);
        self.id_counter
    }

    /// Register a bridge task. Returns the id and a receiver that fires when cancelled.
    pub fn register_bridge(&mut self) -> (i32, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        let id = self.next_id();
        self.tasks.insert(id, TaskEntry::Bridge(tx));
        (id, rx)
    }

    /// Register a scheduler task (RAF / idle). Returns the id.
    pub fn register_scheduler(&mut self) -> i32 {
        let id = self.next_id();
        self.tasks.insert(id, TaskEntry::Scheduler);
        id
    }

    /// Cancel a task by id. For bridge tasks this sends the cancellation signal.
    pub fn cancel(&mut self, id: i32) {
        if let Some(entry) = self.tasks.remove(&id) {
            if let TaskEntry::Bridge(tx) = entry {
                let _ = tx.send(());
            }
        }
    }

    pub fn is_active(&self, id: i32) -> bool {
        self.tasks.contains_key(&id)
    }

    /// Mark a task as completed (remove without cancelling).
    pub fn complete(&mut self, id: i32) {
        self.tasks.remove(&id);
    }
}
