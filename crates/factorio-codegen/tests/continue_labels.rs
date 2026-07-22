#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::missing_const_for_fn
)]

mod common;

use common::must_ok;
use factorio_codegen::LuaGenerator;
use factorio_ir::{
    block::Block,
    expression::Expression,
    function::{Function, Parameter},
    literal::Literal,
    module::{Module, Symbol},
    operator::Operator,
    scope::Scope,
    stage::Stage,
    statement::Statement,
    r#type::Type,
};

fn module_with_body(body: Vec<Statement>) -> Module {
    Module {
        name: "loops".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![Symbol {
            scope: Scope::Public,
            statement: Statement::FunctionDecl(Function {
                name: "run".to_string(),
                params: vec![Parameter {
                    name: "n".to_string(),
                    r#type: Type::Int,
                    source_type: None,
                }],
                body: Block { statements: body },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
                inline: false,
            }),
        }],
    }
}

fn true_cond() -> Expression {
    Expression::Literal(Literal::Bool(true))
}

#[test]
fn nested_while_continue_uses_distinct_labels() {
    let module = module_with_body(vec![Statement::While {
        condition: true_cond(),
        body: vec![
            Statement::Continue,
            Statement::While {
                condition: true_cond(),
                body: vec![Statement::Continue],
            },
        ],
    }]);

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(
        output.contains("goto __continue_1"),
        "outer continue missing:\n{output}"
    );
    assert!(
        output.contains("goto __continue_2"),
        "inner continue missing:\n{output}"
    );
    assert!(
        output.contains("::__continue_1::"),
        "outer label missing:\n{output}"
    );
    assert!(
        output.contains("::__continue_2::"),
        "inner label missing:\n{output}"
    );
}

#[test]
fn nested_for_in_and_numeric_continue_labels() {
    let module = module_with_body(vec![Statement::ForIn {
        var: "item".to_string(),
        iter: Expression::Identifier("items".to_string()),
        ipairs: true,
        body: vec![
            Statement::Continue,
            Statement::ForNumeric {
                var: "i".to_string(),
                start: Expression::Literal(Literal::Int(0)),
                limit: Expression::Identifier("n".to_string()),
                body: vec![Statement::Continue],
            },
        ],
    }]);

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(output.contains("goto __continue_1"), "{output}");
    assert!(output.contains("goto __continue_2"), "{output}");
    assert!(output.contains("::__continue_1::"), "{output}");
    assert!(output.contains("::__continue_2::"), "{output}");
}

#[test]
fn continue_inside_conditional_still_targets_enclosing_loop() {
    let module = module_with_body(vec![Statement::While {
        condition: Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("n".to_string())),
            op: Operator::Gt,
            rhs: Box::new(Expression::Literal(Literal::Int(0))),
        },
        body: vec![Statement::Conditional {
            condition: Expression::Literal(Literal::Bool(true)),
            then_block: vec![Statement::Continue],
            else_block: vec![Statement::Break],
        }],
    }]);

    let output = must_ok(LuaGenerator::new().generate_module(&module));
    assert!(output.contains("goto __continue_1"), "{output}");
    assert!(output.contains("::__continue_1::"), "{output}");
    assert!(output.contains("break"), "{output}");
}
