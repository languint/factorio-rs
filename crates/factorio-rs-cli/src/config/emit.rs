use serde::Deserialize;

/// Settings to configure transpiling.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct EmitConfig {
    /// Optional prefix prepended to every generated Lua module's filename
    #[serde(default)]
    pub lua_module_prefix: Option<String>,
}
