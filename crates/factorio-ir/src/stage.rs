#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Settings,
    SettingsUpdates,
    SettingsFinalFixes,
    Data,
    DataUpdates,
    DataFinalFixes,
    Control,
    Shared,
}

impl Stage {
    /// Stages that emit a Factorio root entry file with side-effect entry functions
    /// (`settings*.lua` / `data*.lua`). Control uses event registration instead.
    pub const SIDE_EFFECT_STAGES: [Self; 6] = [
        Self::Settings,
        Self::SettingsUpdates,
        Self::SettingsFinalFixes,
        Self::Data,
        Self::DataUpdates,
        Self::DataFinalFixes,
    ];

    /// Infer a stage from a dotted module name (e.g. `"control.on_built_entity"`).
    ///
    /// Longer phase names are matched before shorter ones so
    /// `settings_updates` does not fall through to `settings`.
    #[must_use]
    pub fn from_module_name(module_name: &str) -> Option<Self> {
        const PREFIXES: &[(&str, Stage)] = &[
            ("settings_final_fixes", Stage::SettingsFinalFixes),
            ("settings_updates", Stage::SettingsUpdates),
            ("settings", Stage::Settings),
            ("data_final_fixes", Stage::DataFinalFixes),
            ("data_updates", Stage::DataUpdates),
            ("data", Stage::Data),
            ("control", Stage::Control),
            ("shared", Stage::Shared),
        ];

        for &(prefix, stage) in PREFIXES {
            if module_name == prefix
                || module_name
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
            {
                return Some(stage);
            }
        }
        None
    }

    /// The canonical root module name for this stage.
    #[must_use]
    pub const fn default_module_name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::SettingsUpdates => "settings_updates",
            Self::SettingsFinalFixes => "settings_final_fixes",
            Self::Data => "data",
            Self::DataUpdates => "data_updates",
            Self::DataFinalFixes => "data_final_fixes",
            Self::Control => "control",
            Self::Shared => "shared",
        }
    }

    /// The Factorio entry-point file name for this stage, if any.
    ///
    /// `Shared` modules have no entry file - they are required by other stages.
    #[must_use]
    pub const fn entry_file_name(self) -> Option<&'static str> {
        match self {
            Self::Settings => Some("settings.lua"),
            Self::SettingsUpdates => Some("settings-updates.lua"),
            Self::SettingsFinalFixes => Some("settings-final-fixes.lua"),
            Self::Data => Some("data.lua"),
            Self::DataUpdates => Some("data-updates.lua"),
            Self::DataFinalFixes => Some("data-final-fixes.lua"),
            Self::Control => Some("control.lua"),
            Self::Shared => None,
        }
    }

    /// Whether public functions/structs in this stage are load-time entry points.
    #[must_use]
    pub const fn has_side_effect_entry(self) -> bool {
        matches!(
            self,
            Self::Settings
                | Self::SettingsUpdates
                | Self::SettingsFinalFixes
                | Self::Data
                | Self::DataUpdates
                | Self::DataFinalFixes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Stage;

    #[test]
    fn matches_phase_module_names_before_base() {
        assert_eq!(
            Stage::from_module_name("settings_updates"),
            Some(Stage::SettingsUpdates)
        );
        assert_eq!(
            Stage::from_module_name("settings_updates.extra"),
            Some(Stage::SettingsUpdates)
        );
        assert_eq!(
            Stage::from_module_name("settings_final_fixes"),
            Some(Stage::SettingsFinalFixes)
        );
        assert_eq!(Stage::from_module_name("settings"), Some(Stage::Settings));
        assert_eq!(
            Stage::from_module_name("data_updates"),
            Some(Stage::DataUpdates)
        );
        assert_eq!(
            Stage::from_module_name("data_final_fixes.foo"),
            Some(Stage::DataFinalFixes)
        );
        assert_eq!(Stage::from_module_name("data"), Some(Stage::Data));
    }

    #[test]
    fn entry_file_names_match_factorio_layout() {
        assert_eq!(Stage::Settings.entry_file_name(), Some("settings.lua"));
        assert_eq!(
            Stage::SettingsUpdates.entry_file_name(),
            Some("settings-updates.lua")
        );
        assert_eq!(
            Stage::SettingsFinalFixes.entry_file_name(),
            Some("settings-final-fixes.lua")
        );
        assert_eq!(Stage::Data.entry_file_name(), Some("data.lua"));
        assert_eq!(
            Stage::DataUpdates.entry_file_name(),
            Some("data-updates.lua")
        );
        assert_eq!(
            Stage::DataFinalFixes.entry_file_name(),
            Some("data-final-fixes.lua")
        );
    }
}
