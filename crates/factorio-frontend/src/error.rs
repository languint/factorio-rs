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

    #[error("module `{module}` must be marked with `factorio_rs::control_mod!`, `#[factorio_rs::control]`, or live under `src/control/`, `src/shared/`, or `src/data/`")]
    InvalidModuleStage { module: String },

    #[error("event handlers are only allowed in control-stage modules, found in `{module}`")]
    EventOutsideControlStage { module: String },
}

pub type FrontendResult<T> = Result<T, FrontendError>;

impl From<syn::Error> for FrontendError {
    fn from(error: syn::Error) -> Self {
        Self::Syn(error.to_string())
    }
}
