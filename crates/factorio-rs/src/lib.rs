//! Rust SDK for Factorio's modding API.

pub use factorio_api::{self, event_type_to_name};
pub use factorio_macros::{
    assembling_machine, autoplace_control, bench, container, control, data, data_final_fixes,
    data_updates, event, export, fluid, furnace, inline, inserter, item, item_group, item_subgroup,
    lab, locale, lua, mining_drill, mod_settings, module, recipe, recipe_category, resource,
    settings, settings_final_fixes, settings_updates, shared, technology, tile, transport_belt,
};
pub use factorio_macros::{
    control_mod, data_final_fixes_mod, data_mod, data_updates_mod, settings_final_fixes_mod,
    settings_mod, settings_updates_mod, shared_mod,
};

#[cfg(feature = "tracing")]
pub use tracing;

#[cfg(feature = "serde")]
pub use serde;
#[cfg(feature = "serde")]
pub use serde_json;

/// Multi-tick helpers for `factorio-rs test` (compile-only stubs; harness provides Lua).
pub mod test;

pub mod prelude {
    pub use crate::{
        assembling_machine, autoplace_control, bench, container, control, control_mod, data,
        data_final_fixes, data_final_fixes_mod, data_mod, data_updates, data_updates_mod, event,
        export, fluid, furnace, inline, inserter, item, item_group, item_subgroup, lab, locale,
        lua, mining_drill, mod_settings, module, recipe, recipe_category, resource, settings,
        settings_final_fixes, settings_final_fixes_mod, settings_mod, settings_updates,
        settings_updates_mod, shared, shared_mod, technology, tile, transport_belt,
    };
    pub use factorio_api::prelude::*;

    #[cfg(feature = "tracing")]
    pub use tracing::{debug, error, info, trace, warn};

    #[cfg(feature = "serde")]
    pub use serde::{Deserialize, Serialize};
}
