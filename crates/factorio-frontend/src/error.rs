#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FrontendError {
    #[error("failed to parse Rust source: {0}")]
    Syn(String),

    #[error("unsupported item `{item}` at {location}")]
    UnsupportedItem { item: String, location: String },

    #[error("unsupported statement at {location}")]
    UnsupportedStatement { location: String },

    #[error("unsupported expression at {location}")]
    UnsupportedExpression { location: String },

    #[error("unsupported type `{ty}` at {location}")]
    UnsupportedType { ty: String, location: String },

    #[error("unsupported operator at {location}")]
    UnsupportedOperator { location: String },

    #[error("expected an identifier pattern at {location}")]
    ExpectedIdentifierPattern { location: String },

    #[error("expected an identifier assignment target at {location}")]
    ExpectedIdentifierAssignmentTarget { location: String },

    #[error("let binding requires an initializer at {location}")]
    MissingLetInitializer { location: String },

    #[error("unsupported macro `{name}` at {location}")]
    UnsupportedMacro { name: String, location: String },

    #[error("format string `{template}` expects {expected} argument(s), got {found} at {location}")]
    FormatArgumentMismatch {
        template: String,
        expected: usize,
        found: usize,
        location: String,
    },

    #[error(
        "module `{module}` must be marked with `factorio_rs::control_mod!`, `#[factorio_rs::control]`, or live under `src/control/`, `src/shared/`, or `src/data/`"
    )]
    InvalidModuleStage { module: String },

    #[error("event handlers are only allowed in control-stage modules, found in `{module}`")]
    EventOutsideControlStage { module: String },

    #[error("invalid event filter at {location}")]
    InvalidEventFilter { location: String },

    #[error("unsupported event filter method `{method}` at {location}")]
    UnsupportedEventFilterMethod { method: String, location: String },

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
