#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LuaGeneratorError {
    // It could *technically*, but it's likely bad practice
    #[error("function {0} cannot be `local` and exported!")]
    FunctionLocalAndExported(String),
}

pub type LuaGeneratorResult<T> = Result<T, LuaGeneratorError>;
