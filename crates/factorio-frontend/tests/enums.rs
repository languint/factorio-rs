#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{enumeration::EnumVariantFields, expression::Expression, statement::Statement};

#[test]
fn lowers_enum_variants_constructors_matches_and_methods() {
    let module = must_ok_parse(parse_module(
        r#"
        pub enum Msg {
            Quit,
            Move(i64, i64),
            Write { text: String },
        }

        impl Msg {
            pub fn is_quit(&self) -> bool {
                match self {
                    Self::Quit => true,
                    _ => false,
                }
            }
        }

        pub fn make() -> Msg {
            let move_msg = Msg::Move(1, 2);
            let write_msg = Msg::Write { text: "hello".to_string() };
            match move_msg {
                Msg::Move(x, y) => Msg::Write { text: "moved".to_string() },
                Msg::Quit => Msg::Quit,
                Msg::Write { text } => write_msg,
            }
        }
        "#,
        "shared.enums",
    ));

    let Some(Statement::EnumDecl(enum_decl)) = module
        .symbols
        .iter()
        .map(|symbol| &symbol.statement)
        .find(|statement| matches!(statement, Statement::EnumDecl(_)))
    else {
        panic!("expected enum declaration");
    };
    assert_eq!(enum_decl.name, "Msg");
    assert!(matches!(
        enum_decl.variants[0].fields,
        EnumVariantFields::Unit
    ));
    assert!(matches!(
        enum_decl.variants[1].fields,
        EnumVariantFields::Tuple { .. }
    ));
    assert!(matches!(
        enum_decl.variants[2].fields,
        EnumVariantFields::Named(_)
    ));
    assert_eq!(enum_decl.methods.len(), 1);

    let Some(Statement::FunctionDecl(make)) = module
        .symbols
        .iter()
        .map(|symbol| &symbol.statement)
        .find(|statement| matches!(statement, Statement::FunctionDecl(function) if function.name == "make"))
    else {
        panic!("expected make function");
    };
    let Statement::VariableDecl { value, .. } = &make.body.statements[0] else {
        panic!("expected constructor binding");
    };
    assert!(matches!(
        value,
        Expression::EnumLiteral { enum_name, variant, fields }
            if enum_name == "Msg" && variant == "Move" && fields.len() == 2
    ));
}

#[test]
fn enum_method_after_unwrap_or_lowers_to_type_call() {
    let module = must_ok_parse(parse_module(
        r#"
        pub enum Phase {
            Idle,
            Mining { ticks: i64 },
        }

        impl Phase {
            pub fn tick(self) -> Phase {
                Phase::Idle
            }
        }

        pub fn step(opt: Option<Phase>) -> Phase {
            let mut phase = opt.unwrap_or(Phase::Idle);
            phase = phase.tick();
            phase
        }
        "#,
        "shared.phase_tick",
    ));

    let Some(Statement::FunctionDecl(step)) = module
        .symbols
        .iter()
        .map(|symbol| &symbol.statement)
        .find(|statement| {
            matches!(statement, Statement::FunctionDecl(function) if function.name == "step")
        })
    else {
        panic!("expected step function");
    };
    let saw_phase_tick = step.body.statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::Assignment {
                value: Expression::Call {
                    func,
                    ..
                },
                ..
            } if matches!(
                func.as_ref(),
                Expression::QualifiedPath { segments }
                    if segments.as_slice() == ["Phase", "tick"]
            )
        )
    });
    assert!(
        saw_phase_tick,
        "expected Phase.tick(phase), got {:?}",
        step.body.statements
    );
}
