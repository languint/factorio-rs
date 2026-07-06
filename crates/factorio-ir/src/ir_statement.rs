use crate::{ir_expression::IrExpression, ir_type::IrType};

pub enum IrStatement {
    VariableDecl {
        name: String,
        ty: IrType,
        value: IrExpression,
    },
    Assignment {
        target: IrExpression,
        value: IrExpression,
    },
    Conditional {
        condition: IrExpression,
        then_block: Vec<IrStatement>,
        else_block: Vec<IrStatement>,
    },
    Return(Option<IrExpression>),
}
