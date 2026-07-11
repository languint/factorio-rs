//! Rust SDK for Factorio's modding API.

pub use factorio_api::{self, event_type_to_name};
pub use factorio_macros::{
    control, data, data_final_fixes, data_updates, event, mod_settings, settings,
    settings_final_fixes, settings_updates, shared,
};
pub use factorio_macros::{
    control_mod, data_final_fixes_mod, data_mod, data_updates_mod, settings_final_fixes_mod,
    settings_mod, settings_updates_mod, shared_mod,
};

pub mod prelude {
    pub use crate::{
        control, control_mod, data, data_final_fixes, data_final_fixes_mod, data_mod, data_updates,
        data_updates_mod, event, mod_settings, settings, settings_final_fixes,
        settings_final_fixes_mod, settings_mod, settings_updates, settings_updates_mod, shared,
        shared_mod,
    };
    pub use factorio_api::prelude::*;
}
