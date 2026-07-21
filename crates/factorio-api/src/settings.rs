/// Typed Factorio mod-settings tables (`settings.startup`, ...).
///
/// Prefer [`SettingsDictionary::get_bool`] / [`get_int`] / [`get_double`] /
/// [`get_string`] over indexing into opaque values.

/// One mod setting entry (`settings.startup["name"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModSettingValue {
    pub value: crate::LuaAny,
}

pub static UNIT_MOD_SETTING: ModSettingValue = ModSettingValue {
    value: crate::LuaAny,
};

/// A settings stage dictionary (`startup` / `global` / `player_default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettingsDictionary;

impl SettingsDictionary {
    /// Read a bool setting: `settings.startup["name"].value`.
    #[must_use]
    pub const fn get_bool(self, _name: &'static str) -> bool {
        false
    }

    /// Read an integer setting.
    #[must_use]
    pub const fn get_int(self, _name: &'static str) -> i64 {
        0
    }

    /// Read a double setting.
    #[must_use]
    pub const fn get_double(self, _name: &'static str) -> f64 {
        0.0
    }

    /// Read a string setting.
    #[must_use]
    pub const fn get_string(self, _name: &'static str) -> &'static str {
        ""
    }

    /// Fall back to the generic typed read used by older mods.
    #[must_use]
    pub const fn get<T: crate::SettingValue>(self, _name: &'static str) -> T {
        T::STUB
    }

    /// Index into a setting entry (`.value` still opaque for uncommon types).
    #[must_use]
    pub fn setting(self, _name: &'static str) -> ModSettingValue {
        UNIT_MOD_SETTING
    }
}

impl std::ops::Index<&str> for SettingsDictionary {
    type Output = ModSettingValue;

    fn index(&self, _key: &str) -> &ModSettingValue {
        &UNIT_MOD_SETTING
    }
}

pub struct SettingTable {
    pub startup: SettingsDictionary,
    pub global: SettingsDictionary,
    pub player_default: SettingsDictionary,
}

pub const settings: SettingTable = SettingTable {
    startup: SettingsDictionary,
    global: SettingsDictionary,
    player_default: SettingsDictionary,
};

pub struct LuaDataInterface;

impl LuaDataInterface {
    /// Register one or more prototype definitions. Translates to `data:extend({...})`.
    #[allow(unused_variables)]
    pub fn extend<T, I: IntoIterator<Item = T>>(&self, items: I) {}
}

/// The global `data` object used to register prototypes and settings.
pub static data: LuaDataInterface = LuaDataInterface;

pub struct BoolSetting {
    /// Internal mod-namespaced name (e.g. `"my-mod-enabled"`).
    pub name: &'static str,
    /// When the setting takes effect: `"startup"`, `"runtime-global"`, or `"runtime-per-user"`.
    pub setting_type: &'static str,
    /// The default value for this setting.
    pub default_value: bool,
}

pub struct IntSetting {
    /// Internal mod-namespaced name (e.g. `"my-mod-count"`).
    pub name: &'static str,
    /// When the setting takes effect: `"startup"`, `"runtime-global"`, or `"runtime-per-user"`.
    pub setting_type: &'static str,
    /// The default value for this setting.
    pub default_value: i64,
    /// Optional minimum allowed value.
    pub minimum_value: Option<i64>,
    /// Optional maximum allowed value.
    pub maximum_value: Option<i64>,
}

pub struct DoubleSetting {
    /// Internal mod-namespaced name.
    pub name: &'static str,
    /// When the setting takes effect: `"startup"`, `"runtime-global"`, or `"runtime-per-user"`.
    pub setting_type: &'static str,
    /// The default value for this setting.
    pub default_value: f64,
    /// Optional minimum allowed value.
    pub minimum_value: Option<f64>,
    /// Optional maximum allowed value.
    pub maximum_value: Option<f64>,
}

pub struct StringSetting {
    /// Internal mod-namespaced name.
    pub name: &'static str,
    /// When the setting takes effect: `"startup"`, `"runtime-global"`, or `"runtime-per-user"`.
    pub setting_type: &'static str,
    /// The default value for this setting.
    pub default_value: &'static str,
    /// If `true`, the value is not shown in-game (useful for internal state).
    pub hidden: bool,
}
