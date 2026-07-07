use crate::{block::Block, scope::Scope, statement::Statement};

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub scope: Scope,
    pub statement: Statement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub body: Block,
    pub symbols: Vec<Symbol>,
}
