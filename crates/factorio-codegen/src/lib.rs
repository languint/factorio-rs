pub mod generator;

include!(concat!(env!("OUT_DIR"), "/attribute_setters.rs"));

pub use generator::LuaGenerator;
pub use generator::error::{LuaGeneratorError, LuaGeneratorResult};
