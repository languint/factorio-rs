use crate::{block::Block, r#type::Type};

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub r#type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub body: Block,
}
