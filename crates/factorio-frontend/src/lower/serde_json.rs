//! Lower `serde_json::*` calls to Factorio helpers / `string.pack`.
//!
//! Serde does not run in Factorio. Struct values are already Lua tables, so
//! these path calls rewrite to builtins at transpile time (feature `serde`).

use syn::Expr;

use crate::error::FrontendError;

use super::util::location;

/// If `func` is `…::serde_json::name`, return the function name.
pub fn serde_json_path_name(func: &Expr) -> Option<String> {
    let Expr::Path(path) = func else {
        return None;
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [.., crate_name, name] if crate_name == "serde_json" => Some(name.clone()),
        _ => None,
    }
}

/// Reject `serde_json::json!` (and similar) with a clear error.
pub fn reject_serde_json_macro(mac: &syn::ExprMacro) -> Option<FrontendError> {
    let segments: Vec<_> = mac
        .mac
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [.., crate_name, name] if crate_name == "serde_json" => {
            Some(FrontendError::UnsupportedMacro {
                name: format!("serde_json::{name}"),
                location: location(mac),
            })
        }
        _ => None,
    }
}

#[cfg(feature = "serde")]
mod lower {
    use super::FrontendError;

    #[derive(Clone, Copy)]
    pub enum SerdeJsonFn {
        ToString,
        FromStr,
        ToValue,
        FromValue,
        ToVec,
        FromSlice,
    }

    pub fn classify_serde_json_fn(name: &str) -> Option<SerdeJsonFn> {
        match name {
            "to_string" | "to_string_pretty" => Some(SerdeJsonFn::ToString),
            "from_str" => Some(SerdeJsonFn::FromStr),
            "to_value" => Some(SerdeJsonFn::ToValue),
            "from_value" => Some(SerdeJsonFn::FromValue),
            "to_vec" => Some(SerdeJsonFn::ToVec),
            "from_slice" => Some(SerdeJsonFn::FromSlice),
            _ => None,
        }
    }

    pub fn unsupported_serde_json_fn_error(
        func_name: &str,
        call_span_location: &str,
    ) -> FrontendError {
        FrontendError::UnsupportedExpression {
            location: format!(
                "{call_span_location} (unsupported serde_json::{func_name}; \
                 supported: to_string, to_string_pretty, from_str, to_value, \
                 from_value, to_vec, from_slice)"
            ),
        }
    }

    pub fn lower_serde_json_fn(
        kind: SerdeJsonFn,
        value: factorio_ir::expression::Expression,
    ) -> factorio_ir::expression::Expression {
        match kind {
            SerdeJsonFn::ToValue | SerdeJsonFn::FromValue => value,
            SerdeJsonFn::ToString => helpers_method("table_to_json", value),
            SerdeJsonFn::FromStr => helpers_method("json_to_table", value),
            SerdeJsonFn::ToVec => string_method(
                "pack",
                vec![
                    factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::String("s".to_string()),
                    ),
                    helpers_method("table_to_json", value),
                ],
            ),
            SerdeJsonFn::FromSlice => helpers_method(
                "json_to_table",
                string_method(
                    "unpack",
                    vec![
                        factorio_ir::expression::Expression::Literal(
                            factorio_ir::literal::Literal::String("s".to_string()),
                        ),
                        value,
                    ],
                ),
            ),
        }
    }

    fn helpers_method(
        method: &str,
        arg: factorio_ir::expression::Expression,
    ) -> factorio_ir::expression::Expression {
        factorio_ir::expression::Expression::MethodCall {
            receiver: Box::new(factorio_ir::expression::Expression::Identifier(
                "helpers".to_string(),
            )),
            method: method.to_string(),
            args: vec![arg],
        }
    }

    fn string_method(
        method: &str,
        args: Vec<factorio_ir::expression::Expression>,
    ) -> factorio_ir::expression::Expression {
        factorio_ir::expression::Expression::MethodCall {
            receiver: Box::new(factorio_ir::expression::Expression::Identifier(
                "string".to_string(),
            )),
            method: method.to_string(),
            args,
        }
    }
}

#[cfg(feature = "serde")]
pub use lower::{
    classify_serde_json_fn, lower_serde_json_fn, unsupported_serde_json_fn_error,
};
