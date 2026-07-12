mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

#[test]
fn omit_trailing_nil_args_from_calls() {
    let module = Module {
        name: "control".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "die".to_string(),
                params: vec![Parameter {
                    name: "entity".to_string(),
                    r#type: Type::Void,
                    source_type: None,
                }],
                body: Block {
                    statements: vec![Statement::Expr(Expression::MethodCall {
                        receiver: Box::new(Expression::Identifier("entity".to_string())),
                        method: "die".to_string(),
                        args: vec![
                            Expression::Literal(Literal::Nil),
                            Expression::Literal(Literal::Nil),
                        ],
                    })],
                },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
            }),
        }],
    };

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        output.contains("entity.die()"),
        "trailing nils should become empty call, got:\n{output}"
    );
}

#[test]
fn keeps_non_trailing_nil_args() {
    let expr = Expression::Call {
        func: Box::new(Expression::Identifier("f".to_string())),
        args: vec![
            Expression::Literal(Literal::Nil),
            Expression::Literal(Literal::Int(1)),
            Expression::Literal(Literal::Nil),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "f(nil, 1)");
}

#[test]
fn generates_safe_if_expression() {
    let expr = Expression::If {
        condition: Box::new(Expression::Identifier("cond".to_string())),
        then_expr: Box::new(Expression::Literal(Literal::Int(0))),
        else_expr: Box::new(Expression::Literal(Literal::Int(1))),
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(
        lua,
        "(function() if cond then return 0 else return 1 end end)()"
    );
}

#[test]
fn omits_nil_fields_from_struct_literals() {
    let expr = Expression::StructLiteral {
        struct_name: Some("PrintSettings".to_string()),
        fields: vec![
            (
                "color".to_string(),
                Expression::Identifier("c".to_string()),
            ),
            (
                "skip".to_string(),
                Expression::Literal(Literal::Nil),
            ),
        ],
    };
    let lua = LuaGenerator::new().generate_expression(&expr);
    assert_eq!(lua, "{ color = c }");
}
