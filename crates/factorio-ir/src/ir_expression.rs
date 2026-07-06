use crate::{ir_literal::IrLiteral, ir_operator::IrOperator};

#[derive(Debug, PartialEq, Clone)]
pub enum IrExpression {
    Literal(IrLiteral),
    BinaryOp {
        left: Box<IrExpression>,
        op: IrOperator,
        right: Box<IrExpression>,
    },
}
