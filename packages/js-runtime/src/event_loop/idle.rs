use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::time::{Duration, Instant};

use rquickjs::{Ctx, Result};

use super::task::{IdleDeadline, IdleTask};

// ──────────────────────────────────────────────────────────────
// IdleQueue — push buffer shared with SchedulerBridge
// ──────────────────────────────────────────────────────────────

pub struct IdleQueue(VecDeque<IdleTask>);

impl IdleQueue {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn push(&mut self, task: IdleTask) {
        self.0.push_back(task);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = IdleTask> + '_ {
        self.0.drain(..)
    }
}

// ──────────────────────────────────────────────────────────────
// Internal heap entry for timed tasks
// ──────────────────────────────────────────────────────────────

struct TimedIdleEntry {
    timeout_at: Instant,
    insertion_order: u64,
    task: IdleTask,
}

// Min-heap by timeout_at, then insertion_order.
impl PartialEq for TimedIdleEntry {
    fn eq(&self, other: &Self) -> bool {
        self.timeout_at == other.timeout_at && self.insertion_order == other.insertion_order
    }
}
impl Eq for TimedIdleEntry {}

impl PartialOrd for TimedIdleEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimedIdleEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse so BinaryHeap becomes a min-heap.
        other
            .timeout_at
            .cmp(&self.timeout_at)
            .then(other.insertion_order.cmp(&self.insertion_order))
    }
}

// ──────────────────────────────────────────────────────────────
// IdleScheduler
// ──────────────────────────────────────────────────────────────

pub struct IdleScheduler {
    timed: BinaryHeap<TimedIdleEntry>,
    untimed: VecDeque<IdleTask>,
    insertion_counter: u64,
}

impl IdleScheduler {
    pub fn new() -> Self {
        Self {
            timed: BinaryHeap::new(),
            untimed: VecDeque::new(),
            insertion_counter: 0,
        }
    }

    /// Move all tasks from the shared push buffer into the scheduler.
    pub fn drain_inbox(&mut self, inbox: &mut IdleQueue) {
        for task in inbox.drain() {
            match task.timeout_at {
                Some(timeout_at) => {
                    self.timed.push(TimedIdleEntry {
                        timeout_at,
                        insertion_order: self.insertion_counter,
                        task,
                    });
                }
                None => {
                    self.untimed.push_back(task);
                }
            }
            self.insertion_counter += 1;
        }
    }

    /// Run untimed idle callbacks within the given budget window.
    pub fn flush(&mut self, ctx: Ctx<'_>, budget: Duration) -> Result<()> {
        let deadline = Instant::now() + budget;
        while let Some(task) = self.untimed.pop_front() {
            let dl = IdleDeadline { deadline };
            (task.callback)(ctx.clone(), dl)?;
            if Instant::now() >= deadline {
                break;
            }
        }
        Ok(())
    }

    /// Run timed callbacks whose timeout has already expired.
    pub fn flush_timed_out(&mut self, ctx: Ctx<'_>) -> Result<()> {
        let now = Instant::now();
        while let Some(entry) = self.timed.peek() {
            if entry.timeout_at > now {
                break;
            }
            let entry = self.timed.pop().unwrap();
            let dl = IdleDeadline { deadline: now };
            (entry.task.callback)(ctx.clone(), dl)?;
        }
        Ok(())
    }

    /// Earliest timeout among timed tasks, if any.
    pub fn next_timeout(&self) -> Option<Instant> {
        self.timed.peek().map(|e| e.timeout_at)
    }

    pub fn is_empty(&self) -> bool {
        self.timed.is_empty() && self.untimed.is_empty()
    }
}
