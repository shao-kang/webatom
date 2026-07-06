use strum::IntoEnumIterator; // 1. 引入扩展 Trait
use strum::EnumIter;  // 2. 引入派生宏
// ──────────────────────────────────────────────────────────────
// event types
// ──────────────────────────────────────────────────────────────
pub trait Event: std::any::Any + Send + 'static  {
    fn event_type(&self) -> EventType {
        EventType::Macro
    }
}
/// Dispatch priority. Variants define the number of buckets — add a variant
/// to introduce a new priority level; `COUNT` stays in sync automatically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
pub enum EventType{
    AfterMicro,
    Raf,
    Macro,
    Idle,
}

impl EventType {
    /// Number of event types; equals the number of enum variants.
    pub const COUNT: usize = 4;

    fn as_index(self) -> usize {
        match self {
            EventType::AfterMicro => 0,
            EventType::Raf => 1,
            EventType::Macro => 2,
            EventType::Idle => 3,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Handler trait
// ──────────────────────────────────────────────────────────────

/// Typed handler for event `E`. Side effects are the handler's own responsibility.
/// Registered and called on the JS thread — no `Send` required.
pub trait EventHandler<E: Event>: 'static {
    fn handle(&self, event: &E);
}

impl<E, F> EventHandler<E> for F
where
    E: Event,
    F: Fn(&E) + 'static,
{
    fn handle(&self, event: &E) {
        self(event);
    }
}
