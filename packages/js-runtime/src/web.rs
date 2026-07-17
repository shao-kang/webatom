pub mod console;
// pub mod fetch;
// pub mod microtask;
// pub mod stream;
pub mod timer;

pub use console::ConsoleExtension;
// pub use fetch::FetchExtension;
// pub use microtask::MicrotaskExtension;
// pub use stream::StreamExtension;
// pub use timer::TimerExtension;

use crate::extension::Extension;

/// Returns the ordered default extension set.
/// StreamExtension must precede FetchExtension because the fetch glue
/// references ReadableStream as a global that stream glue registers first.
pub fn default_extensions() -> Vec<Box<dyn Extension>> {
    vec![
        Box::new(ConsoleExtension),
        // Box::new(MicrotaskExtension),
        // Box::new(StreamExtension),
        Box::new(timer::TimerExtension),
        // Box::new(FetchExtension),
    ]
}
