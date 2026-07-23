mod concat;
mod hoist;
mod inline;
mod simplify;

use crate::module::Module;

/// Run all IR optimization passes on every module.
pub fn optimize_modules(modules: &mut [Module]) {
    for module in modules {
        optimize_module(module);
    }
}

fn optimize_module(module: &mut Module) {
    hoist::optimize_module(module);
    simplify::optimize_module(module);
    inline::optimize_module(module);
    concat::optimize_module(module);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::Block,
        expression::Expression,
        function::Function,
        literal::Literal,
        module::{Module, Symbol},
        operator::Operator,
        scope::Scope,
        stage::Stage,
        statement::Statement,
        r#type::Type,
    };

    fn module_with_fn(body: Vec<Statement>) -> Module {
        Module {
            name: "m".to_string(),
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
                    name: "f".to_string(),
                    params: vec![],
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

    fn fn_body(module: &Module) -> &[Statement] {
        let Statement::FunctionDecl(f) = &module.symbols[0].statement else {
            panic!("expected function");
        };
        &f.body.statements
    }

    #[test]
    fn hoists_if_expr_in_variable_decl() {
        let mut module = module_with_fn(vec![Statement::VariableDecl {
            name: "x".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::If {
                condition: Box::new(Expression::Identifier("c".to_string())),
                then_expr: Box::new(Expression::Literal(Literal::Int(1))),
                else_expr: Box::new(Expression::Literal(Literal::Int(0))),
            },
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert!(
            matches!(body[0], Statement::VariableDecl { .. }),
            "{body:?}"
        );
        assert!(
            matches!(body[1], Statement::Conditional { .. }),
            "expected conditional after decl: {body:?}"
        );
    }

    #[test]
    fn hoists_if_expr_in_return() {
        let mut module = module_with_fn(vec![Statement::Return(Some(Expression::If {
            condition: Box::new(Expression::Identifier("c".to_string())),
            then_expr: Box::new(Expression::Literal(Literal::Int(1))),
            else_expr: Box::new(Expression::Literal(Literal::Int(0))),
        }))]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert_eq!(body.len(), 1);
        assert!(matches!(body[0], Statement::Conditional { .. }));
    }

    #[test]
    fn expands_empty_iife_in_let() {
        let mut module = module_with_fn(vec![Statement::VariableDecl {
            name: "x".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Call {
                func: Box::new(Expression::Closure {
                    params: vec![],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::Literal(
                            Literal::Int(42),
                        )))],
                    },
                }),
                args: vec![],
            },
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert!(
            body.iter().any(|s| matches!(
                s,
                Statement::Assignment {
                    target: Expression::Identifier(name),
                    value: Expression::Literal(Literal::Int(42)),
                } if name == "x"
            )),
            "{body:?}"
        );
    }

    #[test]
    fn inlines_trivial_closure_call() {
        let mut module = module_with_fn(vec![Statement::VariableDecl {
            name: "y".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Call {
                func: Box::new(Expression::Closure {
                    params: vec!["n".to_string()],
                    body: Block {
                        statements: vec![Statement::Return(Some(Expression::BinaryOp {
                            lhs: Box::new(Expression::Identifier("n".to_string())),
                            op: Operator::Add,
                            rhs: Box::new(Expression::Literal(Literal::Int(1))),
                        }))],
                    },
                }),
                args: vec![Expression::Literal(Literal::Int(3))],
            },
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        let Statement::VariableDecl { value, .. } = &body[0] else {
            panic!("{body:?}");
        };
        assert_eq!(
            value,
            &Expression::BinaryOp {
                lhs: Box::new(Expression::Literal(Literal::Int(3))),
                op: Operator::Add,
                rhs: Box::new(Expression::Literal(Literal::Int(1))),
            }
        );
    }

    #[test]
    fn flattens_nested_format_concat() {
        let mut module = module_with_fn(vec![Statement::VariableDecl {
            name: "s".to_string(),
            ty: Type::Void,
            source_type: None,
            value: Expression::FormatConcat {
                parts: vec![
                    Expression::Literal(Literal::String("a".to_string())),
                    Expression::FormatConcat {
                        parts: vec![
                            Expression::Literal(Literal::String("b".to_string())),
                            Expression::Identifier("x".to_string()),
                        ],
                    },
                    Expression::Literal(Literal::String("c".to_string())),
                ],
            },
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        let Statement::VariableDecl { value, .. } = &body[0] else {
            panic!("{body:?}");
        };
        assert_eq!(
            value,
            &Expression::FormatConcat {
                parts: vec![
                    Expression::Literal(Literal::String("ab".to_string())),
                    Expression::Identifier("x".to_string()),
                    Expression::Literal(Literal::String("c".to_string())),
                ],
            }
        );
    }

    #[test]
    fn folds_bool_return_conditional() {
        let mut module = module_with_fn(vec![Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("phase".to_string())),
                    field: "tag".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::String("Running".to_string()))),
            },
            then_block: vec![Statement::Return(Some(Expression::Literal(Literal::Bool(
                true,
            ))))],
            else_block: vec![Statement::Return(Some(Expression::Literal(Literal::Bool(
                false,
            ))))],
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert_eq!(
            body,
            &[Statement::Return(Some(Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("phase".to_string())),
                    field: "tag".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::String("Running".to_string(),))),
            }))]
        );
    }

    #[test]
    fn folds_match_iife_in_if_condition() {
        let mut module = module_with_fn(vec![Statement::Conditional {
            condition: Expression::Call {
                func: Box::new(Expression::Closure {
                    params: vec![],
                    body: Block {
                        statements: vec![
                            Statement::VariableDecl {
                                name: "__match_0".to_string(),
                                ty: Type::Void,
                                source_type: None,
                                value: Expression::Identifier("phase".to_string()),
                            },
                            Statement::Conditional {
                                condition: Expression::BinaryOp {
                                    lhs: Box::new(Expression::FieldAccess {
                                        base: Box::new(Expression::Identifier(
                                            "__match_0".to_string(),
                                        )),
                                        field: "tag".to_string(),
                                    }),
                                    op: Operator::Eq,
                                    rhs: Box::new(Expression::Literal(Literal::String(
                                        "Mining".to_string(),
                                    ))),
                                },
                                then_block: vec![Statement::Return(Some(Expression::Literal(
                                    Literal::Bool(true),
                                )))],
                                else_block: vec![Statement::Return(Some(Expression::Literal(
                                    Literal::Bool(false),
                                )))],
                            },
                        ],
                    },
                }),
                args: vec![],
            },
            then_block: vec![Statement::Expr(Expression::Call {
                func: Box::new(Expression::Identifier("print".to_string())),
                args: vec![Expression::Literal(Literal::String(
                    "mining started".to_string(),
                ))],
            })],
            else_block: vec![],
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        let Statement::Conditional { condition, .. } = &body[0] else {
            panic!("{body:?}");
        };
        assert_eq!(
            condition,
            &Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("phase".to_string())),
                    field: "tag".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::String("Mining".to_string(),))),
            }
        );
    }

    #[test]
    fn folds_match_tag_bool_through_temp() {
        let mut module = module_with_fn(vec![
            Statement::VariableDecl {
                name: "__match_0".to_string(),
                ty: Type::Void,
                source_type: None,
                value: Expression::Identifier("phase".to_string()),
            },
            Statement::Conditional {
                condition: Expression::BinaryOp {
                    lhs: Box::new(Expression::FieldAccess {
                        base: Box::new(Expression::Identifier("__match_0".to_string())),
                        field: "tag".to_string(),
                    }),
                    op: Operator::Eq,
                    rhs: Box::new(Expression::Literal(Literal::String("Running".to_string()))),
                },
                then_block: vec![Statement::Return(Some(Expression::Literal(Literal::Bool(
                    true,
                ))))],
                else_block: vec![Statement::Return(Some(Expression::Literal(Literal::Bool(
                    false,
                ))))],
            },
        ]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert_eq!(
            body,
            &[Statement::Return(Some(Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("phase".to_string())),
                    field: "tag".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::String("Running".to_string(),))),
            }))]
        );
    }

    #[test]
    fn simplifies_hoisted_option_unwrap_or() {
        let mut module = module_with_fn(vec![Statement::VariableDecl {
            name: "n".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Call {
                func: Box::new(Expression::Closure {
                    params: vec![],
                    body: Block {
                        statements: vec![
                            Statement::VariableDecl {
                                name: "__o".to_string(),
                                ty: Type::Void,
                                source_type: None,
                                value: Expression::MethodCall {
                                    receiver: Box::new(Expression::Identifier(
                                        "storage".to_string(),
                                    )),
                                    method: "get".to_string(),
                                    args: vec![Expression::Literal(Literal::String(
                                        "boots".to_string(),
                                    ))],
                                },
                            },
                            Statement::Conditional {
                                condition: Expression::BinaryOp {
                                    lhs: Box::new(Expression::Identifier("__o".to_string())),
                                    op: Operator::Ne,
                                    rhs: Box::new(Expression::Literal(Literal::Nil)),
                                },
                                then_block: vec![Statement::Return(Some(Expression::Identifier(
                                    "__o".to_string(),
                                )))],
                                else_block: vec![Statement::Return(Some(Expression::Literal(
                                    Literal::Int(0),
                                )))],
                            },
                        ],
                    },
                }),
                args: vec![],
            },
        }]);
        optimize_modules(std::slice::from_mut(&mut module));
        let body = fn_body(&module);
        assert!(
            matches!(
                &body[0],
                Statement::VariableDecl {
                    name,
                    value: Expression::MethodCall { method, .. },
                    ..
                } if name == "n" && method == "get"
            ),
            "expected direct init from get, got {body:?}"
        );
        assert!(
            matches!(
                &body[1],
                Statement::Conditional {
                    condition: Expression::BinaryOp {
                        lhs,
                        op: Operator::Eq,
                        rhs,
                    },
                    then_block,
                    else_block,
                } if matches!(lhs.as_ref(), Expression::Identifier(id) if id == "n")
                    && matches!(rhs.as_ref(), Expression::Literal(Literal::Nil))
                    && else_block.is_empty()
                    && matches!(
                        then_block.as_slice(),
                        [Statement::Assignment {
                            target: Expression::Identifier(t),
                            value: Expression::Literal(Literal::Int(0)),
                        }] if t == "n"
                    )
            ),
            "expected if n == nil then n = 0, got {body:?}"
        );
    }
}
