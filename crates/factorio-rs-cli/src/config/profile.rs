use std::collections::BTreeMap;

use serde::Deserialize;

/// Optimization / emit settings for a named transpile profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ProfileSettings {
    /// When set, emit Rust-oriented comments in generated Lua.
    ///
    /// - `0`: module/function header comments only
    /// - `1+`: also emit inline type annotations
    #[serde(default)]
    pub debug_level: Option<u8>,

    /// Remove unreachable functions and exports from generated Lua.
    #[serde(default)]
    pub prune_dead_code: Option<bool>,
}

/// Fully resolved settings used by a single build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub name: String,
    pub debug_level: Option<u8>,
    pub prune_dead_code: bool,
}

impl ResolvedProfile {
    fn debug_defaults(name: &str) -> Self {
        Self {
            name: name.to_string(),
            debug_level: Some(1),
            prune_dead_code: false,
        }
    }

    fn release_defaults(name: &str) -> Self {
        Self {
            name: name.to_string(),
            debug_level: None,
            prune_dead_code: true,
        }
    }
}

/// Resolve the effective settings for `profile_name`.
///
/// Unknown custom profiles start from release defaults, then apply any
/// `[profiles.<name>]` overrides from the config map.
#[must_use]
pub fn resolve_profile(
    profiles: &BTreeMap<String, ProfileSettings>,
    profile_name: &str,
) -> ResolvedProfile {
    let mut resolved = match profile_name {
        "debug" => ResolvedProfile::debug_defaults(profile_name),
        _ => ResolvedProfile::release_defaults(profile_name),
    };

    if let Some(overlay) = profiles.get(profile_name) {
        if let Some(level) = overlay.debug_level {
            resolved.debug_level = Some(level);
        }
        if let Some(prune) = overlay.prune_dead_code {
            resolved.prune_dead_code = prune;
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::{ProfileSettings, resolve_profile};
    use std::collections::BTreeMap;

    #[test]
    fn debug_defaults_disable_pruning() {
        let settings = resolve_profile(&BTreeMap::new(), "debug");
        assert!(!settings.prune_dead_code);
        assert_eq!(settings.debug_level, Some(1));
    }

    #[test]
    fn release_defaults_enable_pruning() {
        let settings = resolve_profile(&BTreeMap::new(), "release");
        assert!(settings.prune_dead_code);
        assert_eq!(settings.debug_level, Some(0));
    }

    #[test]
    fn toml_partial_override_keeps_base_prune() {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "debug".to_string(),
            ProfileSettings {
                debug_level: Some(2),
                prune_dead_code: None,
            },
        );

        let settings = resolve_profile(&profiles, "debug");
        assert_eq!(settings.debug_level, Some(2));
        assert!(!settings.prune_dead_code);
    }

    #[test]
    fn toml_can_enable_prune_on_debug() {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "debug".to_string(),
            ProfileSettings {
                debug_level: None,
                prune_dead_code: Some(true),
            },
        );

        let settings = resolve_profile(&profiles, "debug");
        assert!(settings.prune_dead_code);
        assert_eq!(settings.debug_level, Some(1));
    }
}
