//! Parse a small subset of Rust source code into [`factorio_ir`].

mod error;
mod lower;
mod paths;

pub use error::{FrontendError, FrontendResult};
pub use lower::parse_module;
pub use paths::{lua_output_path, module_name_from_source, require_local_name};
