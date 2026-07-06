#[derive(Debug, Clone, PartialEq)]
pub enum IrLiteral {
    Int(i64),
    Float(f64),
    Str(String),
}
