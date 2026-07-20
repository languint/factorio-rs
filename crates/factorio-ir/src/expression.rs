use crate::{block::Block, literal::Literal, operator::Operator};

#[derive(Debug, PartialEq, Clone)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    QualifiedPath {
        segments: Vec<String>,
    },
    FieldAccess {
        base: Box<Self>,
        field: String,
    },
    Call {
        func: Box<Self>,
        args: Vec<Self>,
    },
    MethodCall {
        receiver: Box<Self>,
        method: String,
        args: Vec<Self>,
    },
    StructLiteral {
        /// The Rust struct name that produced this literal, used by codegen to inject
        /// fixed Factorio prototype fields (e.g. `type = "bool-setting"`).
        struct_name: Option<String>,
        fields: Vec<(String, Self)>,
    },
    /// Tagged enum value `{ tag = "Variant", ...payload }`.
    EnumLiteral {
        enum_name: String,
        variant: String,
        /// Named fields, or `_1` / `_2` / ... for tuple variants.
        fields: Vec<(String, Self)>,
    },
    /// An operation between a `lhs` and a `rhs` with an [`Operator`]
    BinaryOp {
        lhs: Box<Self>,
        op: Operator,
        rhs: Box<Self>,
    },
    /// String interpolation parts joined with `..` in Lua.
    FormatConcat {
        parts: Vec<Self>,
    },
    /// Lua array literal `{ a, b, c }`.
    Array {
        elements: Vec<Self>,
    },
    /// Lua table index expression `base[key]`.
    Index {
        base: Box<Self>,
        key: Box<Self>,
    },
    /// Logical `not EXPR` in Lua.
    Not(Box<Self>),
    /// Length operator `#EXPR` in Lua.
    Len(Box<Self>),
    /// Safe if-expression (avoids falsey `and`/`or` pitfalls).
    If {
        condition: Box<Self>,
        then_expr: Box<Self>,
        else_expr: Box<Self>,
    },
    /// Anonymous Lua function value (`function(params) ... end`).
    Closure {
        params: Vec<String>,
        body: Block,
    },
    /// Fat pointer for `dyn Trait`: `{ _data = ..., _vt = __vt_Trait_Concrete }`.
    FatPointer {
        data: Box<Self>,
        /// Fully qualified vtable symbol name, e.g. `__vt_Display_Point`.
        vtable: String,
    },
    /// Dynamic dispatch: `recv._vt.method(recv, args...)`.
    DynMethodCall {
        receiver: Box<Self>,
        method: String,
        args: Vec<Self>,
    },
}
