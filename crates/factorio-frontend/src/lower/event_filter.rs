use syn::{Expr, ExprCall, ExprLit, Lit};

use factorio_ir::{expression::Expression, literal::Literal};

use crate::error::{FrontendError, FrontendResult};

use super::util::location;

pub fn lower_event_filter_list(expr: &Expr) -> FrontendResult<Expression> {
    match expr {
        Expr::Reference(reference) => lower_event_filter_list(&reference.expr),
        Expr::Array(array) => {
            let elements = array
                .elems
                .iter()
                .map(lower_event_filter_entry)
                .collect::<FrontendResult<Vec<_>>>()?;
            Ok(Expression::Array { elements })
        }
        other => Ok(Expression::Array {
            elements: vec![lower_event_filter_entry(other)?],
        }),
    }
}

pub fn lower_event_filter_entry(expr: &Expr) -> FrontendResult<Expression> {
    let Expr::Call(call) = expr else {
        return Err(FrontendError::InvalidEventFilter {
            location: location(expr),
        });
    };
    lower_filter_builder_call(call)
}

/// Lower `*Filter::method(...)` builder calls used for event filters and choose-elem
/// `PrototypeFilter` entries.
pub fn try_lower_filter_builder_call(call: &ExprCall) -> Option<FrontendResult<Expression>> {
    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let type_name = &segments[segments.len() - 2];
    let method = &segments[segments.len() - 1];
    if !type_name.ends_with("Filter") {
        return None;
    }
    factorio_api::filter_method_spec(method)?;
    Some(lower_filter_builder_call(call))
}

pub fn lower_filter_builder_call(call: &ExprCall) -> FrontendResult<Expression> {
    let method = call_method_name(&call.func).ok_or_else(|| FrontendError::InvalidEventFilter {
        location: location(&call.func),
    })?;

    let spec = factorio_api::filter_method_spec(&method).ok_or_else(|| {
        FrontendError::UnsupportedEventFilterMethod {
            method,
            location: location(&call.func),
        }
    })?;

    let mut fields = vec![(
        "filter".to_string(),
        Expression::Literal(Literal::String(spec.filter.to_string())),
    )];

    if spec.arg_count == 1 {
        let Some(value_field) = spec.value_field else {
            return Err(FrontendError::InvalidEventFilter {
                location: location(call),
            });
        };
        let Some(arg) = call.args.first() else {
            return Err(FrontendError::InvalidEventFilter {
                location: location(call),
            });
        };
        if value_field == "elem_filters" {
            fields.push((value_field.to_string(), lower_event_filter_list(arg)?));
        } else {
            fields.push((
                value_field.to_string(),
                Expression::Literal(Literal::String(string_literal(arg, location(call))?)),
            ));
        }
    } else if spec.arg_count == 2 {
        if call.args.len() != 2 {
            return Err(FrontendError::InvalidEventFilter {
                location: location(call),
            });
        }
        fields.push((
            "comparison".to_string(),
            Expression::Literal(Literal::String(string_literal(
                &call.args[0],
                location(call),
            )?)),
        ));
        fields.push((
            "value".to_string(),
            lower_number_literal(&call.args[1], location(call))?,
        ));
    } else if !call.args.is_empty() {
        return Err(FrontendError::InvalidEventFilter {
            location: location(call),
        });
    }

    Ok(Expression::StructLiteral {
        struct_name: None,
        fields,
    })
}

fn call_method_name(func: &Expr) -> Option<String> {
    match func {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Field(field) => match &field.member {
            syn::Member::Named(ident) => Some(ident.to_string()),
            syn::Member::Unnamed(_) => None,
        },
        _ => None,
    }
}

fn string_literal(expr: &Expr, location: factorio_ir::span::SourceLoc) -> FrontendResult<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(string),
        ..
    }) = expr
    else {
        return Err(FrontendError::InvalidEventFilter { location });
    };

    Ok(string.value())
}

fn lower_number_literal(
    expr: &Expr,
    location: factorio_ir::span::SourceLoc,
) -> FrontendResult<Expression> {
    let Expr::Lit(ExprLit { lit, .. }) = expr else {
        return Err(FrontendError::InvalidEventFilter { location });
    };
    match lit {
        Lit::Float(float) => Ok(Expression::Literal(Literal::Float(
            float
                .base10_parse::<f64>()
                .map_err(|_| FrontendError::InvalidEventFilter { location })?,
        ))),
        Lit::Int(int) => Ok(Expression::Literal(Literal::Float(
            int.base10_parse::<f64>()
                .map_err(|_| FrontendError::InvalidEventFilter { location })?,
        ))),
        _ => Err(FrontendError::InvalidEventFilter { location }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use syn::{Expr, parse_str};

    use super::lower_event_filter_list;
    use factorio_ir::{expression::Expression, literal::Literal};

    #[test]
    fn lowers_type_filter_entry() {
        let expr =
            parse_str::<Expr>(r#"[OnBuiltEntityFilter::type_("inserter")]"#).expect("filter expr");

        let lowered = lower_event_filter_list(&expr).expect("lower filter");
        let Expression::Array { elements } = lowered else {
            panic!("expected array");
        };
        assert_eq!(elements.len(), 1);
        let Expression::StructLiteral { fields, .. } = &elements[0] else {
            panic!("expected struct literal");
        };
        assert_eq!(
            fields,
            &[
                (
                    "filter".to_string(),
                    Expression::Literal(Literal::String("type".to_string()))
                ),
                (
                    "type".to_string(),
                    Expression::Literal(Literal::String("inserter".to_string()))
                ),
            ]
        );
    }

    #[test]
    fn lowers_entity_prototype_filter_name() {
        let expr =
            parse_str::<Expr>(r#"[EntityPrototypeFilter::name("furnace")]"#).expect("filter expr");

        let lowered = lower_event_filter_list(&expr).expect("lower filter");
        let Expression::Array { elements } = lowered else {
            panic!("expected array");
        };
        let Expression::StructLiteral { fields, .. } = &elements[0] else {
            panic!("expected struct literal");
        };
        assert_eq!(
            fields,
            &[
                (
                    "filter".to_string(),
                    Expression::Literal(Literal::String("name".to_string()))
                ),
                (
                    "name".to_string(),
                    Expression::Literal(Literal::String("furnace".to_string()))
                ),
            ]
        );
    }

    #[test]
    fn lowers_nested_elem_filters() {
        let expr = parse_str::<Expr>(
            r#"[ItemPrototypeFilter::place_result(&[EntityPrototypeFilter::name("furnace")])]"#,
        )
        .expect("filter expr");

        let lowered = lower_event_filter_list(&expr).expect("lower filter");
        let Expression::Array { elements } = lowered else {
            panic!("expected array");
        };
        let Expression::StructLiteral { fields, .. } = &elements[0] else {
            panic!("expected struct literal");
        };
        assert_eq!(fields[0].0, "filter");
        assert_eq!(
            fields[0].1,
            Expression::Literal(Literal::String("place-result".to_string()))
        );
        assert_eq!(fields[1].0, "elem_filters");
        let Expression::Array { elements: nested } = &fields[1].1 else {
            panic!("expected nested elem_filters array");
        };
        assert_eq!(nested.len(), 1);
    }
}
