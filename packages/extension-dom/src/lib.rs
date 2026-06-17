pub mod core;
pub mod bridge;
mod dom_extension;
mod blitz_bridge;

pub use core::Document;
pub use bridge::{DocumentHandle, NodeHandle};
pub use dom_extension::DomExtension;
