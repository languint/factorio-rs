#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    Int,
    Float,
    Str,
    Void,
    FactorioHandle(String), // "LuaEntity", etc.
}
