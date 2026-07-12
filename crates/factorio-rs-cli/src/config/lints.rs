use std::collections::BTreeMap;

use serde::Deserialize;

use factorio_ir::lint::{LintConfig, LintLevel, LintsTable};

use crate::error::{CliError, CliResult};

/// `[lints]` section: map lint identifiers to `allow` / `warn` / `deny`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LintsConfig {
    #[serde(flatten)]
    pub levels: BTreeMap<String, LintLevel>,
}

impl LintsConfig {
    /// Resolve overrides into a [`LintConfig`].
    pub fn resolve(&self) -> CliResult<LintConfig> {
        LintsTable {
            levels: self.levels.clone(),
        }
        .into_config()
        .map_err(|message| CliError::InvalidLints { message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorio_ir::lint::LintId;

    #[test]
    fn resolves_allow_override() {
        let toml = r#"
unwrap = "allow"
format_spec = "warn"
"#;
        let config: LintsConfig = toml::from_str(toml).unwrap();
        let resolved = config.resolve().unwrap();
        assert_eq!(resolved.level(LintId::Unwrap), LintLevel::Allow);
        assert_eq!(resolved.level(LintId::FormatSpec), LintLevel::Warn);
        assert_eq!(resolved.level(LintId::Expect), LintLevel::Deny);
    }

    #[test]
    fn rejects_unknown_lint() {
        let toml = r#"bogus = "allow""#;
        let config: LintsConfig = toml::from_str(toml).unwrap();
        assert!(config.resolve().is_err());
    }
}
