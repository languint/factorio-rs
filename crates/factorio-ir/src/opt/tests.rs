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
                    statements: vec![Statement::Return(Some(Expression::Literal(Literal::Int(
                        42,
                    ))))],
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
            Statement::VariableDecl {
                name,
                value: Expression::Literal(Literal::Int(42)),
                ..
            } if name == "x"
        ) || matches!(
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
fn hoists_if_from_binop_rhs() {
    let mut module = module_with_fn(vec![Statement::Assignment {
        target: Expression::Index {
            base: Box::new(Expression::Identifier("storage".to_string())),
            key: Box::new(Expression::Literal(Literal::String("n".to_string()))),
        },
        value: Expression::BinaryOp {
            lhs: Box::new(Expression::If {
                condition: Box::new(Expression::BinaryOp {
                    lhs: Box::new(Expression::Identifier("x".to_string())),
                    op: Operator::Ne,
                    rhs: Box::new(Expression::Literal(Literal::Nil)),
                }),
                then_expr: Box::new(Expression::Identifier("x".to_string())),
                else_expr: Box::new(Expression::Literal(Literal::Int(0))),
            }),
            op: Operator::Add,
            rhs: Box::new(Expression::Literal(Literal::Int(1))),
        },
    }]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    let still_nested = body.iter().any(|s| {
        matches!(
            s,
            Statement::Assignment {
                value: Expression::BinaryOp { lhs, .. },
                ..
            } if matches!(lhs.as_ref(), Expression::If { .. } | Expression::Call { .. })
        )
    });
    assert!(
        !still_nested,
        "mid-expr If should be hoisted out of binop:\n{body:?}"
    );
    assert!(
        body.iter()
            .any(|s| matches!(s, Statement::Conditional { .. }))
            || body.iter().any(|s| {
                matches!(
                    s,
                    Statement::VariableDecl { name, .. } if name.starts_with("__h")
                )
            }),
        "{body:?}"
    );
}

#[test]
fn collapses_nil_init_then_assign() {
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "a".to_string(),
            ty: Type::Void,
            source_type: None,
            value: Expression::Literal(Literal::Nil),
        },
        Statement::Assignment {
            target: Expression::Identifier("a".to_string()),
            value: Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("msg".to_string())),
                    field: "tag".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::String("Move".to_string()))),
            },
        },
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert_eq!(body.len(), 1, "{body:?}");
    assert!(
        matches!(
            &body[0],
            Statement::VariableDecl {
                name,
                value: Expression::BinaryOp { .. },
                ..
            } if name == "a"
        ),
        "{body:?}"
    );
}

#[test]
fn folds_bool_eq_true_condition() {
    let mut module = module_with_fn(vec![Statement::Conditional {
        condition: Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("flag".to_string())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Bool(true))),
        },
        then_block: vec![Statement::Return(Some(Expression::Literal(Literal::Int(
            1,
        ))))],
        else_block: vec![Statement::Return(Some(Expression::Literal(Literal::Int(
            0,
        ))))],
    }]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    let Statement::Conditional { condition, .. } = &body[0] else {
        panic!("{body:?}");
    };
    assert_eq!(condition, &Expression::Identifier("flag".to_string()));
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
                                    base: Box::new(Expression::Identifier("__match_0".to_string())),
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
fn peephole_mutates_local_for_repeated_add() {
    let boots_plus = Expression::BinaryOp {
        lhs: Box::new(Expression::Identifier("boots".to_string())),
        op: Operator::Add,
        rhs: Box::new(Expression::Literal(Literal::Int(1))),
    };
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "boots".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Literal(Literal::Int(0)),
        },
        Statement::Assignment {
            target: Expression::Index {
                base: Box::new(Expression::Identifier("storage".to_string())),
                key: Box::new(Expression::Literal(Literal::String("boots".to_string()))),
            },
            value: boots_plus.clone(),
        },
        Statement::Expr(Expression::FormatConcat {
            parts: vec![
                Expression::Literal(Literal::String("boot=".to_string())),
                boots_plus,
            ],
        }),
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert!(
        matches!(
            &body[1],
            Statement::Assignment {
                target: Expression::Identifier(name),
                value: Expression::BinaryOp { op: Operator::Add, .. },
            } if name == "boots"
        ),
        "expected boots = boots + 1, got {body:?}"
    );
    assert!(
        matches!(
            &body[2],
            Statement::Assignment {
                value: Expression::Identifier(name),
                ..
            } if name == "boots"
        ),
        "storage write should reuse boots, got {body:?}"
    );
    assert!(
        matches!(
            &body[3],
            Statement::Expr(Expression::FormatConcat { parts })
                if parts.iter().any(|p| matches!(p, Expression::Identifier(n) if n == "boots"))
                    && !parts.iter().any(|p| matches!(p, Expression::BinaryOp { .. }))
        ),
        "print should reuse boots, got {body:?}"
    );
}

#[test]
fn peephole_cse_temp_when_bare_local_still_needed() {
    let x_plus = Expression::BinaryOp {
        lhs: Box::new(Expression::Identifier("x".to_string())),
        op: Operator::Add,
        rhs: Box::new(Expression::Literal(Literal::Int(1))),
    };
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "x".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Literal(Literal::Int(0)),
        },
        Statement::Assignment {
            target: Expression::Identifier("a".to_string()),
            value: x_plus.clone(),
        },
        // Bare read of old `x` blocks mutate-local.
        Statement::Expr(Expression::Identifier("x".to_string())),
        Statement::Assignment {
            target: Expression::Identifier("b".to_string()),
            value: x_plus,
        },
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert!(
        body.iter().any(|s| matches!(
            s,
            Statement::VariableDecl { name, .. } if name.starts_with("__a_")
        )),
        "expected CSE temp when bare local is still read, got {body:?}"
    );
}

#[test]
fn folds_unwrap_or_hoist_temp_into_user_binding() {
    // Frontend hoist_safe_if + let: local __h = nil; if ...; local n = __h
    // After unwrap_or simplify: local __h = recv; if __h == nil; local n = __h
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "__h_2".to_string(),
            ty: Type::Void,
            source_type: None,
            value: Expression::Index {
                base: Box::new(Expression::Identifier("storage".to_string())),
                key: Box::new(Expression::Literal(Literal::String("boots".to_string()))),
            },
        },
        Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::Identifier("__h_2".to_string())),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::Nil)),
            },
            then_block: vec![Statement::Assignment {
                target: Expression::Identifier("__h_2".to_string()),
                value: Expression::Literal(Literal::Int(0)),
            }],
            else_block: vec![],
        },
        Statement::VariableDecl {
            name: "n".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Identifier("__h_2".to_string()),
        },
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert!(
        matches!(
            &body[0],
            Statement::VariableDecl {
                name,
                value: Expression::Index { .. },
                ..
            } if name == "n"
        ),
        "expected unwrap_or temp renamed into `n`, got {body:?}"
    );
    assert!(
        matches!(
            &body[1],
            Statement::Conditional {
                condition: Expression::BinaryOp {
                    lhs,
                    op: Operator::Eq,
                    ..
                },
                ..
            } if matches!(lhs.as_ref(), Expression::Identifier(id) if id == "n")
        ),
        "expected nil-check on `n`, got {body:?}"
    );
    assert_eq!(body.len(), 2, "copy binding should be removed: {body:?}");
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
                            value: Expression::method_call(
                                Expression::Identifier("storage".to_string()),
                                "get",
                                vec![Expression::Literal(Literal::String("boots".to_string()))],
                            ),
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

#[test]
fn simplifies_result_unwrap_or() {
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "n".to_string(),
            ty: Type::Int,
            source_type: None,
            value: Expression::Literal(Literal::Nil),
        },
        Statement::Conditional {
            condition: Expression::BinaryOp {
                lhs: Box::new(Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("r".to_string())),
                    field: "err".to_string(),
                }),
                op: Operator::Eq,
                rhs: Box::new(Expression::Literal(Literal::Nil)),
            },
            then_block: vec![Statement::Assignment {
                target: Expression::Identifier("n".to_string()),
                value: Expression::FieldAccess {
                    base: Box::new(Expression::Identifier("r".to_string())),
                    field: "ok".to_string(),
                },
            }],
            else_block: vec![Statement::Assignment {
                target: Expression::Identifier("n".to_string()),
                value: Expression::Literal(Literal::Int(0)),
            }],
        },
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert!(
        matches!(
            &body[0],
            Statement::VariableDecl {
                name,
                value: Expression::FieldAccess { field, .. },
                ..
            } if name == "n" && field == "ok"
        ),
        "expected local n = r.ok, got {body:?}"
    );
    assert!(
        matches!(
            &body[1],
            Statement::Conditional {
                condition: Expression::BinaryOp {
                    lhs,
                    op: Operator::Ne,
                    ..
                },
                then_block,
                else_block,
            } if matches!(
                lhs.as_ref(),
                Expression::FieldAccess { field, .. } if field == "err"
            ) && else_block.is_empty()
                && matches!(
                    then_block.as_slice(),
                    [Statement::Assignment {
                        value: Expression::Literal(Literal::Int(0)),
                        ..
                    }]
                )
        ),
        "expected if r.err ~= nil then n = 0, got {body:?}"
    );
}

#[test]
fn folds_not_not_and_negated_comparison() {
    let mut module = module_with_fn(vec![Statement::Return(Some(Expression::Not(Box::new(
        Expression::Not(Box::new(Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        })),
    ))))]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert_eq!(
        body,
        &[Statement::Return(Some(Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }))]
    );

    let mut module = module_with_fn(vec![Statement::Conditional {
        condition: Expression::Not(Box::new(Expression::BinaryOp {
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Eq,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        })),
        then_block: vec![Statement::Return(None)],
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
            lhs: Box::new(Expression::Identifier("x".to_string())),
            op: Operator::Ne,
            rhs: Box::new(Expression::Literal(Literal::Nil)),
        }
    );
}

#[test]
fn drops_empty_concat_parts() {
    let mut module = module_with_fn(vec![Statement::Return(Some(Expression::FormatConcat {
        parts: vec![
            Expression::Literal(Literal::String(String::new())),
            Expression::Identifier("x".to_string()),
            Expression::Literal(Literal::String(String::new())),
        ],
    }))]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert_eq!(
        body,
        &[Statement::Return(Some(Expression::Identifier(
            "x".to_string()
        )))]
    );
}

#[test]
fn simplify_after_inline_folds_bool_closure() {
    // `|b| if b then true else false` inlined, then folded to `b`.
    let mut module = module_with_fn(vec![Statement::Return(Some(Expression::Call {
        func: Box::new(Expression::Closure {
            params: vec!["b".to_string()],
            body: Block {
                statements: vec![Statement::Return(Some(Expression::If {
                    condition: Box::new(Expression::Identifier("b".to_string())),
                    then_expr: Box::new(Expression::Literal(Literal::Bool(true))),
                    else_expr: Box::new(Expression::Literal(Literal::Bool(false))),
                }))],
            },
        }),
        args: vec![Expression::Identifier("flag".to_string())],
    }))]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert_eq!(
        body,
        &[Statement::Return(Some(Expression::Identifier(
            "flag".to_string()
        )))],
        "expected inline+simplify to yield `return flag`, got {body:?}"
    );
}

#[test]
fn does_not_sink_impure_temp_into_conditional_body() {
    let mut module = module_with_fn(vec![
        Statement::VariableDecl {
            name: "__t".to_string(),
            ty: Type::Void,
            source_type: None,
            value: Expression::Call {
                func: Box::new(Expression::Identifier("side_effect".to_string())),
                args: vec![],
            },
        },
        Statement::Conditional {
            condition: Expression::Identifier("c".to_string()),
            then_block: vec![Statement::Expr(Expression::Call {
                func: Box::new(Expression::Identifier("use".to_string())),
                args: vec![Expression::Identifier("__t".to_string())],
            })],
            else_block: vec![],
        },
    ]);
    optimize_modules(std::slice::from_mut(&mut module));
    let body = fn_body(&module);
    assert!(
        matches!(
            &body[0],
            Statement::VariableDecl {
                name,
                value: Expression::Call { func, .. },
                ..
            } if name == "__t"
                && matches!(func.as_ref(), Expression::Identifier(f) if f == "side_effect")
        ),
        "impure temp must stay outside the branch, got {body:?}"
    );
    assert!(
        matches!(
            &body[1],
            Statement::Conditional { then_block, .. }
                if matches!(
                    then_block.as_slice(),
                    [Statement::Expr(Expression::Call { args, .. })]
                        if matches!(args.as_slice(), [Expression::Identifier(id)] if id == "__t")
                )
        ),
        "then-arm should still read __t, got {body:?}"
    );
}
