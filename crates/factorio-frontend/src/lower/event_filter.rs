use syn::{Expr, ExprLit, Lit};

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
                location: location(expr),
            });
        };
        let Some(arg) = call.args.first() else {
            return Err(FrontendError::InvalidEventFilter {
                location: location(expr),
            });
        };
        fields.push((
            value_field.to_string(),
            Expression::Literal(Literal::String(string_literal(arg, location(expr))?)),
        ));
    } else if !call.args.is_empty() {
        return Err(FrontendError::InvalidEventFilter {
            location: location(expr),
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

fn string_literal(
    expr: &Expr,
    location: factorio_ir::span::SourceLoc,
) -> FrontendResult<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(string),
        ..
    }) = expr
    else {
        return Err(FrontendError::InvalidEventFilter { location });
    };

    Ok(string.value())
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
}
