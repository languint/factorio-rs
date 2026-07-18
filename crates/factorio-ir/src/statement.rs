use crate::{
    enumeration::Enum, expression::Expression, function::Function, structure::Struct, r#type::Type,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    FunctionDecl(Function),
    StructDecl(Struct),
    EnumDecl(Enum),
    VariableDecl {
        name: String,
        ty: Type,
        source_type: Option<String>,
        value: Expression,
    },
    Assignment {
        target: Expression,
        value: Expression,
    },
    Conditional {
        condition: Expression,
        then_block: Vec<Self>,
        else_block: Vec<Self>,
    },
    Return(Option<Expression>),
    Expr(Expression),
    /// `for _, VAR in pairs/ipairs(ITER) do BODY end` in Lua.
    ForIn {
        var: String,
        iter: Expression,
        body: Vec<Self>,
        /// When true, emit `ipairs` (ordered); otherwise `pairs`.
        ipairs: bool,
    },
    /// `for VAR = START, LIMIT do BODY end` in Lua (inclusive limit, step 1).
    ForNumeric {
        var: String,
        start: Expression,
        limit: Expression,
        body: Vec<Self>,
    },
    /// `while CONDITION do BODY end` in Lua. Rust `loop { }` lowers with
    /// `condition = true`.
    While {
        condition: Expression,
        body: Vec<Self>,
    },
    /// `goto __continue_N` in Lua (the label `::__continue_N::` is emitted by
    /// the enclosing `ForIn` / `ForNumeric` / `While`).
    Continue,
    /// Lua `break` (exits the innermost loop).
    Break,
}
