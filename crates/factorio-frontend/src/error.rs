use factorio_ir::span::SourceLoc;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FrontendError {
    #[error("failed to parse Rust source: {0}")]
    Syn(String),

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

    #[error("invalid event filter at {location}")]
    InvalidEventFilter { location: SourceLoc },

    #[error("unsupported event filter method `{method}` at {location}")]
    UnsupportedEventFilterMethod { method: String, location: SourceLoc },

    #[error("could not resolve locale key `{path}` to a string constant in this module")]
    LocaleKeyUnresolved { path: String },

    #[error("invalid locale entry: {message}")]
    InvalidLocale { message: String },

    #[error("{0}")]
    Lint(factorio_ir::lint::Diagnostic),
}

pub type FrontendResult<T> = Result<T, FrontendError>;

impl From<syn::Error> for FrontendError {
    fn from(error: syn::Error) -> Self {
        Self::Syn(error.to_string())
    }
}

impl FrontendError {
    /// Primary source location when this error points into a file.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLoc> {
        match self {
            Self::UnsupportedItem { location, .. }
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
            Self::Syn(_)
            | Self::InvalidModuleStage { .. }
            | Self::EventOutsideControlStage { .. }
            | Self::LocaleKeyUnresolved { .. }
            | Self::InvalidLocale { .. } => None,
        }
    }

    /// Short headline used by ariadne reports.
    #[must_use]
    pub fn report_message(&self) -> String {
        match self {
            Self::Syn(message) => format!("failed to parse Rust source: {message}"),
            Self::UnsupportedItem { item, .. } => format!("unsupported item `{item}`"),
            Self::UnsupportedStatement { .. } => "unsupported statement".to_string(),
            Self::UnsupportedExpression { location } => location
                .note
                .as_ref()
                .map_or_else(|| "unsupported expression".to_string(), |n| {
                    format!("unsupported expression ({n})")
                }),
            Self::UnsupportedType { ty, .. } => format!("unsupported type `{ty}`"),
            Self::UnsupportedOperator { .. } => "unsupported operator".to_string(),
            Self::ExpectedIdentifierPattern { .. } => {
                "expected an identifier pattern".to_string()
            }
            Self::ExpectedIdentifierAssignmentTarget { .. } => {
                "expected an identifier assignment target".to_string()
            }
            Self::MissingLetInitializer { .. } => {
                "let binding requires an initializer".to_string()
            }
            Self::UnsupportedMacro { name, .. } => format!("unsupported macro `{name}`"),
            Self::FormatArgumentMismatch {
                template,
                expected,
                found,
                ..
            } => format!(
                "format string `{template}` expects {expected} argument(s), got {found}"
            ),
            Self::InvalidModuleStage { module } => format!(
                "module `{module}` must be marked with a Factorio stage attribute or live under a stage path"
            ),
            Self::EventOutsideControlStage { module } => {
                format!("event handlers are only allowed in control-stage modules, found in `{module}`")
            }
            Self::InvalidEventFilter { .. } => "invalid event filter".to_string(),
            Self::UnsupportedEventFilterMethod { method, .. } => {
                format!("unsupported event filter method `{method}`")
            }
            Self::LocaleKeyUnresolved { path } => {
                format!("could not resolve locale key `{path}` to a string constant in this module")
            }
            Self::InvalidLocale { message } => format!("invalid locale entry: {message}"),
            Self::Lint(diagnostic) => diagnostic.message.clone(),
        }
    }
}
