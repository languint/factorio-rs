//! Parse a small subset of Rust source code into [`factorio_ir`].

mod discovery;
mod error;
mod lower;
mod paths;
mod report;

pub use discovery::{DiscoveredModule, discover_modules};
pub use error::{FrontendError, FrontendResult};
pub use lower::{
    ParseOptions, parse_discovered_module, parse_discovered_module_with_options,
    parse_discovered_module_with_prefix, parse_module, parse_module_with_options,
    parse_module_with_prefix,
};
pub use paths::{lua_output_path, module_name_from_source, require_local_name};
pub use report::{
    display_filename, eprint_diagnostic, eprint_frontend_error, write_diagnostic,
    write_frontend_error,
};
