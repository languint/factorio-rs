#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]

mod common;

use common::{assert_lua_fragment_parses, must_ok};
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    enumeration::{Enum, EnumVariant, EnumVariantFields},
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol, VTable},
    operator::Operator,
    scope::Scope,
    stage::Stage,
    statement::Statement,
    structure::{Struct, StructField},
    r#type::Type,
};

fn id(name: &str) -> Expression {
    Expression::Identifier(name.to_string())
}

fn lit_int(n: i64) -> Expression {
    Expression::Literal(Literal::Int(n))
}

fn empty_fn(name: &str, body: Vec<Statement>) -> Function {
    Function {
        name: name.to_string(),
        params: vec![],
        body: Block { statements: body },
        doc: None,
        debug: None,
        event: None,
        event_filter: None,
        export: None,
        inline: false,
    }
}

fn module_with_public_fn(name: &str, body: Vec<Statement>) -> Module {
    Module {
        name: "probe".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(empty_fn(name, body)),
        }],
    }
}

fn emit_expr(expr: Expression) -> String {
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_lua_fragment_parses(&lua);
    lua
}

#[test]
fn every_literal_variant_emits() {
    for lit in [
        Literal::Int(42),
        Literal::Float(1.5),
        Literal::String("hi".to_string()),
        Literal::Bool(true),
        Literal::Nil,
    ] {
        let _ = emit_expr(Expression::Literal(lit));
    }
}

#[test]
fn every_expression_variant_emits_and_parses() {
    let closure_body = Block {
        statements: vec![Statement::Return(Some(lit_int(1)))],
    };

    let cases: Vec<(&str, Expression)> = vec![
        ("Literal", Expression::Literal(Literal::Int(1))),
        ("Identifier", id("x")),
        (
            "QualifiedPath",
            Expression::QualifiedPath {
                segments: vec!["defines".to_string(), "events".to_string()],
            },
        ),
        (
            "FieldAccess",
            Expression::FieldAccess {
                base: Box::new(id("player")),
                field: "name".to_string(),
            },
        ),
        (
            "Call",
            Expression::Call {
                func: Box::new(id("f")),
                args: vec![lit_int(1)],
            },
        ),
        (
            "MethodCall",
            Expression::MethodCall {
                receiver: Box::new(id("e")),
                method: "get_health".to_string(),
                args: vec![],
            },
        ),
        (
            "StructLiteral",
            Expression::StructLiteral {
                struct_name: Some("Point".to_string()),
                fields: vec![("x".to_string(), lit_int(1))],
            },
        ),
        (
            "EnumLiteral",
            Expression::EnumLiteral {
                enum_name: "Msg".to_string(),
                variant: "Quit".to_string(),
                fields: vec![],
            },
        ),
        (
            "BinaryOp",
            Expression::BinaryOp {
                lhs: Box::new(id("a")),
                op: Operator::Add,
                rhs: Box::new(id("b")),
            },
        ),
        (
            "FormatConcat",
            Expression::FormatConcat {
                parts: vec![
                    Expression::Literal(Literal::String("n=".to_string())),
                    id("n"),
                ],
            },
        ),
        (
            "Array",
            Expression::Array {
                elements: vec![lit_int(1), lit_int(2)],
            },
        ),
        (
            "Index",
            Expression::Index {
                base: Box::new(id("xs")),
                key: Box::new(lit_int(0)),
            },
        ),
        ("Not", Expression::Not(Box::new(id("ok")))),
        ("Len", Expression::Len(Box::new(id("xs")))),
        (
            "If",
            Expression::If {
                condition: Box::new(id("cond")),
                then_expr: Box::new(lit_int(1)),
                else_expr: Box::new(lit_int(0)),
            },
        ),
        (
            "Closure",
            Expression::Closure {
                params: vec!["n".to_string()],
                body: closure_body,
            },
        ),
        (
            "FatPointer",
            Expression::FatPointer {
                data: Box::new(id("value")),
                vtable: "__vt_Display_Point".to_string(),
            },
        ),
        (
            "DynMethodCall",
            Expression::DynMethodCall {
                receiver: Box::new(id("fp")),
                method: "show".to_string(),
                args: vec![lit_int(1)],
            },
        ),
    ];

    for (name, expr) in cases {
        let lua = emit_expr(expr);
        assert!(!lua.is_empty(), "{name} emitted empty");
    }
}

#[test]
fn every_statement_variant_emits_in_a_module() {
    let module = Module {
        name: "probe".to_string(),
        stage: Stage::Control,
        body: Block {
            statements: vec![
                Statement::FunctionDecl(empty_fn(
                    "helper",
                    vec![Statement::Return(Some(lit_int(1)))],
                )),
                Statement::StructDecl(Struct {
                    name: "Point".to_string(),
                    fields: vec![StructField {
                        name: "x".to_string(),
                        ty: Type::Int,
                        source_type: None,
                    }],
                    constants: vec![],
                    methods: vec![],
                    doc: None,
                    debug: None,
                }),
                Statement::EnumDecl(Enum {
                    name: "Msg".to_string(),
                    variants: vec![EnumVariant {
                        name: "Quit".to_string(),
                        fields: EnumVariantFields::Unit,
                    }],
                    constants: vec![],
                    methods: vec![],
                    doc: None,
                    debug: None,
                }),
                Statement::VariableDecl {
                    name: "n".to_string(),
                    ty: Type::Int,
                    source_type: None,
                    value: lit_int(0),
                },
            ],
        },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![VTable {
            name: "__vt_Display_Point".to_string(),
            concrete_type: "Point".to_string(),
            methods: vec!["show".to_string()],
        }],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "run".to_string(),
                params: vec![Parameter {
                    name: "xs".to_string(),
                    r#type: Type::Void,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![
                        Statement::VariableDecl {
                            name: "acc".to_string(),
                            ty: Type::Int,
                            source_type: None,
                            value: lit_int(0),
                        },
                        Statement::Assignment {
                            target: id("acc"),
                            value: lit_int(1),
                        },
                        Statement::Conditional {
                            condition: Expression::Literal(Literal::Bool(true)),
                            then_block: vec![Statement::Expr(Expression::Call {
                                func: Box::new(id("print")),
                                args: vec![id("acc")],
                            })],
                            else_block: vec![Statement::Return(None)],
                        },
                        Statement::ForIn {
                            var: "item".to_string(),
                            iter: id("xs"),
                            ipairs: true,
                            body: vec![Statement::Continue],
                        },
                        Statement::ForNumeric {
                            var: "i".to_string(),
                            start: lit_int(0),
                            limit: lit_int(3),
                            body: vec![Statement::Break],
                        },
                        Statement::While {
                            condition: Expression::Literal(Literal::Bool(false)),
                            body: vec![Statement::Break],
                        },
                        Statement::Return(Some(id("acc"))),
                    ],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        output.contains("Point = {}") || output.contains("local Point = {}"),
        "{output}"
    );
    assert!(output.contains("local Msg = {}"), "{output}");
    assert!(output.contains("__vt_Display_Point"), "{output}");
    assert!(output.contains("goto __continue_1"), "{output}");
}

#[test]
fn closure_as_callee_is_parenthesized() {
    let expr = Expression::Call {
        func: Box::new(Expression::Closure {
            params: vec!["n".to_string()],
            body: Block {
                statements: vec![Statement::Return(Some(id("n")))],
            },
        }),
        args: vec![lit_int(1)],
    };
    let lua = emit_expr(expr);
    assert!(
        lua.starts_with("(function"),
        "closure callee must be parenthesized: {lua}"
    );
}

#[test]
fn index_literal_is_one_based() {
    let expr = Expression::Index {
        base: Box::new(id("xs")),
        key: Box::new(lit_int(0)),
    };
    assert_eq!(emit_expr(expr), "xs[1]");
}

#[test]
fn public_module_with_expr_statement_parses() {
    let module = module_with_public_fn(
        "tick",
        vec![Statement::Expr(Expression::Call {
            func: Box::new(id("print")),
            args: vec![lit_int(1)],
        })],
    );
    let _ = must_ok(LuaGenerator::new().generate_module(&module));
}
