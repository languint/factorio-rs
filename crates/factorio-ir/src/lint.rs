//! Transpile-time safety lints with stable identifiers for `Factorio.toml`.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::span::SourceLoc;

/// Stable lint identifier used in diagnostics and `[lints]` config keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LintId {
    /// `.unwrap()` is a no-op in Lua (nil is not checked).
    Unwrap,
    /// `.expect(...)` is a no-op in Lua (message discarded, nil not checked).
    Expect,
    /// Non-`?` format specs (e.g. `{:.2}`) are ignored when lowering.
    FormatSpec,
    /// Non-literal array indices are not shifted for Lua's 1-based tables.
    VariableIndex,
    /// Identification enum constructors (e.g. `ForceID::Name(...)`) are not lowered; use `.into()`.
    IdentificationCtor,
}

impl LintId {
    /// Config / diagnostic name (`unwrap`, `expect`, ...) for `Factorio.toml` `[lints]`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unwrap => "unwrap",
            Self::Expect => "expect",
            Self::FormatSpec => "format_spec",
            Self::VariableIndex => "variable_index",
            Self::IdentificationCtor => "identification_ctor",
        }
    }

    /// Rustc-style diagnostic code shown in reports (`E0001`, ...).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unwrap => "E0001",
            Self::Expect => "E0002",
            Self::FormatSpec => "E0003",
            Self::VariableIndex => "E0004",
            Self::IdentificationCtor => "E0005",
        }
    }

    /// Default severity when unset in `Factorio.toml`.
    ///
    /// Most lints default to deny (they can miscompile). `format_spec` only
    /// drops unsupported precision/width and still emits working Lua, so it
    /// defaults to warn.
    #[must_use]
    pub const fn default_level(self) -> LintLevel {
        match self {
            Self::FormatSpec => LintLevel::Warn,
            Self::Unwrap | Self::Expect | Self::VariableIndex | Self::IdentificationCtor => {
                LintLevel::Deny
            }
        }
    }

    /// Short help shown under ariadne reports.
    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::Unwrap => "use `if let Some(x) = ...` (or set `[lints] unwrap = \"allow\"`)",
            Self::Expect => "use `if let Some(x) = ...` (or set `[lints] expect = \"allow\"`)",
            #[allow(clippy::literal_string_with_formatting_args)]
            Self::FormatSpec => "only `{}`, `{:?}`, and `{:#?}` are supported when lowering",
            Self::VariableIndex => {
                "literal indices are shifted `n -> n+1`; pass a 1-based index or use a literal"
            }
            Self::IdentificationCtor => {
                "pass a payload with `.into()` instead, e.g. `force.into()` or `\"enemy\".into()`"
            }
        }
    }

    /// All built-in lint identifiers.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Unwrap,
            Self::Expect,
            Self::FormatSpec,
            Self::VariableIndex,
            Self::IdentificationCtor,
        ]
    }

    /// Parse a config key into a lint id.
    #[must_use]
    pub fn from_config_str(name: &str) -> Option<Self> {
        Self::all().iter().copied().find(|id| id.as_str() == name)
    }
}

impl std::fmt::Display for LintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// How a lint is treated when it fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintLevel {
    /// Do not emit the lint (disabled).
    Allow,
    /// Emit a warning; build still succeeds.
    Warn,
    /// Emit an error; build fails.
    #[default]
    Deny,
}

impl LintLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Deny => "deny",
        }
    }
}

/// Resolved lint levels from `Factorio.toml` `[lints]` (and defaults).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintConfig {
    levels: BTreeMap<LintId, LintLevel>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            levels: LintId::all()
                .iter()
                .map(|id| (*id, id.default_level()))
                .collect(),
        }
    }
}

impl LintConfig {
    /// All lints allowed (useful in unit tests that exercise transparent methods).
    #[must_use]
    pub fn allow_all() -> Self {
        Self {
            levels: LintId::all()
                .iter()
                .map(|id| (*id, LintLevel::Allow))
                .collect(),
        }
    }

    /// Apply overrides from a `[lints]` table (`unwrap = "allow"`, ...).
    ///
    /// # Errors
    /// Returns the unknown lint name when a key is not a known [`LintId`].
    pub fn with_overrides(
        mut self,
        overrides: &BTreeMap<String, LintLevel>,
    ) -> Result<Self, String> {
        for (name, level) in overrides {
            let Some(id) = LintId::from_config_str(name) else {
                let known = LintId::all()
                    .iter()
                    .copied()
                    .map(LintId::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!("unknown lint `{name}` (known: {known})"));
            };
            self.levels.insert(id, *level);
        }
        Ok(self)
    }

    #[must_use]
    pub fn level(&self, id: LintId) -> LintLevel {
        self.levels
            .get(&id)
            .copied()
            .unwrap_or_else(|| id.default_level())
    }

    #[must_use]
    pub fn is_allowed(&self, id: LintId) -> bool {
        matches!(self.level(id), LintLevel::Allow)
    }

    /// Allow a single lint (builder-style).
    #[must_use]
    pub fn allowing(mut self, id: LintId) -> Self {
        self.levels.insert(id, LintLevel::Allow);
        self
    }
}

/// A single transpile diagnostic tied to a lint code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: LintId,
    pub level: LintLevel,
    pub message: String,
    pub loc: SourceLoc,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        id: LintId,
        level: LintLevel,
        message: impl Into<String>,
        loc: impl Into<SourceLoc>,
    ) -> Self {
        Self {
            id,
            level,
            message: message.into(),
            loc: loc.into(),
        }
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self.level, LintLevel::Deny)
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {} ({}) at {}",
            self.level.as_str(),
            self.id.code(),
            self.message,
            self.id.as_str(),
            self.loc
        )
    }
}

/// Raw `[lints]` table as deserialized from `Factorio.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LintsTable {
    #[serde(flatten)]
    pub levels: BTreeMap<String, LintLevel>,
}

impl LintsTable {
    /// Resolve into a [`LintConfig`], validating lint names.
    ///
    /// # Errors
    /// Unknown lint identifiers.
    pub fn into_config(self) -> Result<LintConfig, String> {
        LintConfig::default().with_overrides(&self.levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_levels() {
        let config = LintConfig::default();
        assert_eq!(config.level(LintId::Unwrap), LintLevel::Deny);
        assert_eq!(config.level(LintId::FormatSpec), LintLevel::Warn);
    }

    #[test]
    fn overrides_allow_unwrap() {
        let mut table = BTreeMap::new();
        table.insert("unwrap".to_string(), LintLevel::Allow);
        let config = LintConfig::default().with_overrides(&table).unwrap();
        assert!(config.is_allowed(LintId::Unwrap));
        assert!(!config.is_allowed(LintId::Expect));
    }

    #[test]
    fn rejects_unknown_lint_name() {
        let mut table = BTreeMap::new();
        table.insert("not_a_lint".to_string(), LintLevel::Allow);
        let err = LintConfig::default().with_overrides(&table).unwrap_err();
        assert!(err.contains("not_a_lint"));
    }

    #[test]
    fn lint_codes_are_stable() {
        assert_eq!(LintId::Unwrap.code(), "E0001");
        assert_eq!(LintId::Expect.code(), "E0002");
        assert_eq!(LintId::FormatSpec.code(), "E0003");
        assert_eq!(LintId::VariableIndex.code(), "E0004");
        assert_eq!(LintId::IdentificationCtor.code(), "E0005");
    }
}
