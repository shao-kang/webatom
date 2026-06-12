mod attributes;
mod document;
mod element;
mod html_parser;
mod node;

pub use attributes::{Attribute, Attributes};
pub use document::Document;
pub use element::{ElementData, TextNodeData};
pub use html_parser::parse_html;
pub use node::{Node, NodeData, NodeKind};
