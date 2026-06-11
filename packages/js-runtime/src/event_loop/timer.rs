use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rquickjs::{Function, Persistent};

pub(crate) struct TimerHeapEntry {
    pub(crate) deadline: Instant,
    pub(crate) id: u32,
}

impl PartialEq for TimerHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}
impl Eq for TimerHeapEntry {}
impl PartialOrd for TimerHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimerHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse so BinaryHeap (max-heap) becomes a min-heap by deadline.
        other.deadline.cmp(&self.deadline)
    }
}

pub(crate) struct PendingTimer {
    pub(crate) id: u32,
    pub(crate) func: Persistent<Function<'static>>,
    pub(crate) interval: Option<Duration>,
    pub(crate) cancelled: bool,
}

pub struct TimerState {
    pub(crate) next_id: u32,
    pub(crate) pending: Vec<PendingTimer>,
    pub(crate) heap: BinaryHeap<TimerHeapEntry>,
}

pub type SharedTimers = Arc<Mutex<TimerState>>;

impl TimerState {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            pending: Vec::new(),
            heap: BinaryHeap::new(),
        }
    }

    pub fn schedule(
        &mut self,
        func: Persistent<Function<'static>>,
        delay_ms: u64,
        interval_ms: Option<u64>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(PendingTimer {
            id,
            func,
            interval: interval_ms.map(Duration::from_millis),
            cancelled: false,
        });
        self.heap.push(TimerHeapEntry {
            deadline: Instant::now() + Duration::from_millis(delay_ms),
            id,
        });
        id
    }

    pub fn cancel(&mut self, id: u32) {
        if let Some(t) = self.pending.iter_mut().find(|t| t.id == id) {
            t.cancelled = true;
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.heap.is_empty()
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.heap.peek().map(|e| e.deadline)
    }
}
