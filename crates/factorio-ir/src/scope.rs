#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Scope {
    /// Local to the module
    Private,
    /// Exported from the module
    Public,
}
