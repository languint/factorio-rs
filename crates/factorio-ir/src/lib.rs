//! Intermediate representation for factorio-rs.
//!
//! Sources are grouped by role (`ast`, `module`, `meta`, `opt`, `prune`). The
//! historical crate-root module paths (`expression`, `statement`, ...) remain
//! available as re-exports.

pub mod ast;
pub mod lint;
pub mod meta;
pub mod module;
pub mod opt;
pub mod prune;

#[doc(inline)]
pub use ast::r#type;
pub use ast::{
    block, enumeration, expression, function, literal, operator, scope, statement, structure,
};
pub use meta::{debug, span};
pub use module::{locale, stage};
