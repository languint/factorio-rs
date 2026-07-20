pub mod generator;

include!(concat!(env!("OUT_DIR"), "/attribute_setters.rs"));
include!(concat!(env!("OUT_DIR"), "/prototype_type_map.rs"));

pub use generator::LuaGenerator;
pub use generator::error::{LuaGeneratorError, LuaGeneratorResult};
