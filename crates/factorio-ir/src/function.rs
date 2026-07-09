use crate::{block::Block, debug::FunctionDebug, r#type::Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub r#type: Type,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub body: Block,
    pub doc: Option<String>,
    pub debug: Option<FunctionDebug>,
    /// Factorio event name when this function is registered with `#[factorio::event(...)]`.
    pub event: Option<String>,
}
