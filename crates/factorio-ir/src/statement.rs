use crate::{expression::Expression, function::Function, r#type::Type};

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    FunctionDecl(Function),
    VariableDecl {
        name: String,
        ty: Type,
        value: Expression,
    },
    Assignment {
        target: Expression,
        value: Expression,
    },
    Conditional {
        condition: Expression,
        then_block: Vec<Statement>,
        else_block: Vec<Statement>,
    },
    Return(Option<Expression>),
}
