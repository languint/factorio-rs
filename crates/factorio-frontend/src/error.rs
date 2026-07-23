use factorio_ir::span::SourceLoc;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FrontendError {
    #[error("failed to parse Rust source: {message}")]
    Syn {
        message: String,
        location: SourceLoc,
    },

    #[error("unsupported item `{item}` at {location}")]
    UnsupportedItem { item: String, location: SourceLoc },

    #[error("unsupported statement at {location}")]
    UnsupportedStatement { location: SourceLoc },

    #[error("unsupported expression at {location}")]
    UnsupportedExpression { location: SourceLoc },

    #[error("unsupported type `{ty}` at {location}")]
    UnsupportedType { ty: String, location: SourceLoc },

    #[error("unsupported operator at {location}")]
    UnsupportedOperator { location: SourceLoc },

    #[error("expected an identifier pattern at {location}")]
    ExpectedIdentifierPattern { location: SourceLoc },

    #[error("expected an identifier assignment target at {location}")]
    ExpectedIdentifierAssignmentTarget { location: SourceLoc },

    #[error("let binding requires an initializer at {location}")]
    MissingLetInitializer { location: SourceLoc },

    #[error("unsupported macro `{name}` at {location}")]
    UnsupportedMacro { name: String, location: SourceLoc },

    #[error("format string `{template}` expects {expected} argument(s), got {found} at {location}")]
    FormatArgumentMismatch {
        template: String,
        expected: usize,
        found: usize,
        location: SourceLoc,
    },

    #[error(
        "module `{module}` must be marked with `factorio_rs::control_mod!`, `#[factorio_rs::control]`, or live under `src/control/`, `src/shared/`, or `src/data/`"
    )]
    InvalidModuleStage { module: String },

    #[error("event handlers are only allowed in control-stage modules, found in `{module}`")]
    EventOutsideControlStage { module: String },

    #[error(
        "`#[factorio_rs::inline]` is only allowed in shared-stage modules (found `{function}` in `{module}`); move hot helpers to `shared` so dependents call them via require, not remote"
    )]
    InlineOutsideShared { module: String, function: String },

    #[error("invalid event filter at {location}")]
    InvalidEventFilter { location: SourceLoc },

    #[error("unsupported event filter method `{method}` at {location}")]
    UnsupportedEventFilterMethod { method: String, location: SourceLoc },

    #[error("could not resolve locale key `{path}` to a string constant in this module")]
    LocaleKeyUnresolved { path: String },

    #[error("invalid locale entry: {message}")]
    InvalidLocale { message: String },

    #[error(
        "item! relative icon `{path}` needs the package mod name; use a full `__mod__/...` path or build with factorio-rs"
    )]
    ItemIconNeedsModName { path: String },

    #[error("{0}")]
    Lint(factorio_ir::lint::Diagnostic),
}

pub type FrontendResult<T> = Result<T, FrontendError>;

impl From<syn::Error> for FrontendError {
    fn from(error: syn::Error) -> Self {
        let range = error.span().byte_range();
        Self::Syn {
            message: error.to_string(),
            location: SourceLoc::new(factorio_ir::span::SourceSpan::from(range)),
        }
    }
}

impl FrontendError {
    /// Rustc-style diagnostic code shown in reports (`E0100`, ...).
    ///
    /// Hard frontend errors use the `E01xx` series so they do not collide with
    /// transpile lint codes (`E0001`-`E0017`).
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::Lint(diagnostic) => Some(diagnostic.id.code()),
            Self::Syn { .. } => Some("E0100"),
            Self::UnsupportedItem { .. } => Some("E0101"),
            Self::UnsupportedStatement { .. } => Some("E0102"),
            Self::UnsupportedExpression { .. } => Some("E0103"),
            Self::UnsupportedType { .. } => Some("E0104"),
            Self::UnsupportedOperator { .. } => Some("E0105"),
            Self::ExpectedIdentifierPattern { .. } => Some("E0106"),
            Self::ExpectedIdentifierAssignmentTarget { .. } => Some("E0107"),
            Self::MissingLetInitializer { .. } => Some("E0108"),
            Self::UnsupportedMacro { .. } => Some("E0109"),
            Self::FormatArgumentMismatch { .. } => Some("E0110"),
            Self::InvalidModuleStage { .. } => Some("E0111"),
            Self::EventOutsideControlStage { .. } => Some("E0112"),
            Self::InlineOutsideShared { .. } => Some("E0113"),
            Self::InvalidEventFilter { .. } => Some("E0114"),
            Self::UnsupportedEventFilterMethod { .. } => Some("E0115"),
            Self::LocaleKeyUnresolved { .. } => Some("E0116"),
            Self::InvalidLocale { .. } => Some("E0117"),
            Self::ItemIconNeedsModName { .. } => Some("E0118"),
        }
    }

    /// Short help shown under ariadne reports.
    #[must_use]
    #[allow(clippy::literal_string_with_formatting_args)]
    pub const fn help(&self) -> Option<&'static str> {
        match self {
            Self::Lint(diagnostic) => Some(diagnostic.id.help()),
            Self::Syn { .. } => Some(
                "fix the syntax error, or ensure macros expand to supported Rust before lowering",
            ),
            Self::UnsupportedItem { .. } => Some(
                "use a supported item form, or expand macros so the frontend sees ordinary Rust",
            ),
            Self::UnsupportedStatement { .. } => Some(
                "rewrite with supported control flow (`if`/`match`/`for`/`while`/`loop`/`return`)",
            ),
            Self::UnsupportedExpression { .. } => {
                Some("see the language guide for the supported expression subset")
            }
            Self::UnsupportedType { .. } => {
                Some("use a supported type (`Option`, `Result`, structs/enums, Factorio API stubs)")
            }
            Self::UnsupportedOperator { .. } => Some(
                "use an arithmetic, comparison, logical, or assignment operator that lowers to Lua",
            ),
            Self::ExpectedIdentifierPattern { .. } => {
                Some("bind with a simple identifier pattern (`let x = ...`)")
            }
            Self::ExpectedIdentifierAssignmentTarget { .. } => {
                Some("assign to a local name, field, or index (not a complex expression)")
            }
            Self::MissingLetInitializer { .. } => {
                Some("write `let name = expr;` with an initializer")
            }
            Self::UnsupportedMacro { .. } => Some(
                "use a supported factorio_rs macro, or expand to supported Rust (`cargo expand` / check build)",
            ),
            Self::FormatArgumentMismatch { .. } => {
                Some("match each `{}` / `{:?}` placeholder to one argument")
            }
            Self::InvalidModuleStage { .. } => Some(
                "add `factorio_rs::control_mod!` / `#[factorio_rs::control]` (or `shared`/`data`), or place the file under `src/control/`, `src/shared/`, or `src/data/`",
            ),
            Self::EventOutsideControlStage { .. } => {
                Some("move `#[event]` handlers into a control-stage module")
            }
            Self::InlineOutsideShared { .. } => {
                Some("move `#[factorio_rs::inline]` helpers into a shared-stage module")
            }
            Self::InvalidEventFilter { .. } => {
                Some("use a supported event filter builder chain from the Factorio API stubs")
            }
            Self::UnsupportedEventFilterMethod { .. } => {
                Some("call a filter method that factorio-rs knows how to lower")
            }
            Self::LocaleKeyUnresolved { .. } => {
                Some("define the key with `locale! { ... }` in this crate, or use a string literal")
            }
            Self::InvalidLocale { .. } => Some("locale values must be single-line string literals"),
            Self::ItemIconNeedsModName { .. } => Some(
                "use `__mod-name__/path.png` or build with factorio-rs so the package mod name is known",
            ),
        }
    }

    /// Primary source location when this error points into a file.
    #[must_use]
    pub const fn location(&self) -> Option<&SourceLoc> {
        match self {
            Self::Syn { location, .. }
            | Self::UnsupportedItem { location, .. }
            | Self::UnsupportedStatement { location }
            | Self::UnsupportedExpression { location }
            | Self::UnsupportedType { location, .. }
            | Self::UnsupportedOperator { location }
            | Self::ExpectedIdentifierPattern { location }
            | Self::ExpectedIdentifierAssignmentTarget { location }
            | Self::MissingLetInitializer { location }
            | Self::UnsupportedMacro { location, .. }
            | Self::FormatArgumentMismatch { location, .. }
            | Self::InvalidEventFilter { location }
            | Self::UnsupportedEventFilterMethod { location, .. } => Some(location),
            Self::Lint(diagnostic) => Some(&diagnostic.loc),
            Self::InvalidModuleStage { .. }
            | Self::EventOutsideControlStage { .. }
            | Self::InlineOutsideShared { .. }
            | Self::LocaleKeyUnresolved { .. }
            | Self::InvalidLocale { .. }
            | Self::ItemIconNeedsModName { .. } => None,
        }
    }

    /// Short headline used by ariadne reports.
    #[must_use]
    pub fn report_message(&self) -> String {
        match self {
            Self::Syn { message, .. } => format!("failed to parse Rust source: {message}"),
            Self::UnsupportedItem { item, .. } => format!("unsupported item `{item}`"),
            Self::UnsupportedStatement { .. } => "unsupported statement".to_string(),
            Self::UnsupportedExpression { location } => location.note.as_ref().map_or_else(
                || "unsupported expression".to_string(),
                |n| format!("unsupported expression ({n})"),
            ),
            Self::UnsupportedType { ty, .. } => format!("unsupported type `{ty}`"),
            Self::UnsupportedOperator { .. } => "unsupported operator".to_string(),
            Self::ExpectedIdentifierPattern { .. } => "expected an identifier pattern".to_string(),
            Self::ExpectedIdentifierAssignmentTarget { .. } => {
                "expected an identifier assignment target".to_string()
            }
            Self::MissingLetInitializer { .. } => "let binding requires an initializer".to_string(),
            Self::UnsupportedMacro { name, .. } => format!("unsupported macro `{name}`"),
            Self::FormatArgumentMismatch {
                template,
                expected,
                found,
                ..
            } => format!("format string `{template}` expects {expected} argument(s), got {found}"),
            Self::InvalidModuleStage { module } => format!(
                "module `{module}` must be marked with a Factorio stage attribute or live under a stage path"
            ),
            Self::EventOutsideControlStage { module } => {
                format!(
                    "event handlers are only allowed in control-stage modules, found in `{module}`"
                )
            }
            Self::InlineOutsideShared { module, function } => {
                format!(
                    "`#[factorio_rs::inline]` on `{function}` is only allowed in shared-stage modules (found in `{module}`)"
                )
            }
            Self::InvalidEventFilter { .. } => "invalid event filter".to_string(),
            Self::UnsupportedEventFilterMethod { method, .. } => {
                format!("unsupported event filter method `{method}`")
            }
            Self::LocaleKeyUnresolved { path } => {
                format!("could not resolve locale key `{path}` to a string constant in this module")
            }
            Self::InvalidLocale { message } => format!("invalid locale entry: {message}"),
            Self::ItemIconNeedsModName { path } => format!(
                "item! relative icon `{path}` needs the package mod name; use a full `__mod__/...` path or build with factorio-rs"
            ),
            Self::Lint(diagnostic) => diagnostic.message.clone(),
        }
    }
}
