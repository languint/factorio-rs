pub mod generator;
pub mod pack;

include!(concat!(env!("OUT_DIR"), "/attribute_setters.rs"));
include!(concat!(env!("OUT_DIR"), "/prototype_type_map.rs"));

pub use generator::LuaGenerator;
pub use generator::error::{LuaGeneratorError, LuaGeneratorResult};
pub use pack::{
    EmitModOptions, EventRegistration, RemoteExport, StageModule, collect_event_registrations,
    collect_remote_exports, collect_stage_module, emit_mod_tree, generate_control_lua,
    generate_info_json, generate_stage_entry_lua, merge_locale_files, module_lua_identifier,
    prefixed_lua_path,
};
