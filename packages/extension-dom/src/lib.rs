pub mod core;
pub mod bridge;
pub mod html_entry;
mod dom_extension;

pub use core::Document;
pub use bridge::{DocumentHandle, NodeHandle};
pub use dom_extension::{DomExtension, DomExtensionState};
