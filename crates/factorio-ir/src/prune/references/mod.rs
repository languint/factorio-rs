//! Collect item references from IR functions and expressions for reachability.

mod resolve;
mod walk;

pub use walk::{collect_references_from_expression, collect_references_from_function};
