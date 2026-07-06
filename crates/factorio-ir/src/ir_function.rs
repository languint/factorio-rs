use crate::{ir_statement::IrStatement, ir_type::IrType};

pub struct IrArg {
    pub name: String,
    pub ir_type: IrType,
}

pub struct IrFunction {
    pub name: String,
    pub inputs: Vec<IrArg>,
    pub output_type: IrType,
    pub body: Vec<IrStatement>,
}
