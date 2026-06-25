use rquickjs::{Function, Persistent};

use super::handle::KeepaliveGuard;

pub enum RuntimeEvent {
    Timer(TimerEvent),
    // Fetch / WebSocket: Phase 1+
}

pub struct TimerEvent {
    pub id: i32,
    pub callback: Persistent<Function<'static>>,
    /// Dropping this releases the keepalive so the event loop can exit.
    pub keepalive: KeepaliveGuard,
}
