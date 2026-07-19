use std::fmt::Write as _;

use factorio_ir::expression::Expression;

use crate::{LuaGenerator, attribute_property_for_setter};

/// Map a Rust prototype struct name to its fixed Factorio `type` discriminant string.
/// Returns `None` for non-prototype structs.
fn prototype_lua_type(struct_name: &str) -> Option<&'static str> {
    match struct_name {
        "BoolSetting" => Some("bool-setting"),
        "IntSetting" => Some("int-setting"),
        "DoubleSetting" => Some("double-setting"),
        "StringSetting" => Some("string-setting"),
        "Item" => Some("item"),
        _ => None,
    }
}

impl LuaGenerator {
    #[must_use]
    pub fn generate_expression(&self, expression: &Expression) -> String {
        self.generate_expression_prec(expression, 0)
    }

    pub(crate) fn generate_expression_prec(&self, expression: &Expression, min_prec: u8) -> String {
        match expression {
            Expression::BinaryOp { lhs, op, rhs } => {
                // `0 - x` (or `0.0 - x`) is the frontend's encoding of unary negation.
                // Emit as Lua `-x` directly.
                let is_zero_lhs = match lhs.as_ref() {
                    Expression::Literal(factorio_ir::literal::Literal::Int(0)) => true,
                    Expression::Literal(factorio_ir::literal::Literal::Float(f)) => *f == 0.0,
                    _ => false,
                };
                if matches!(op, factorio_ir::operator::Operator::Sub) && is_zero_lhs {
                    let rhs_str = self.generate_expression_prec(rhs, 100);
                    let result = format!("-{rhs_str}");
                    return if 0 < min_prec {
                        format!("({result})")
                    } else {
                        result
                    };
                }

                let prec = Self::operator_precedence(*op);
                let lhs_str = self.generate_expression_prec(lhs, prec);
                let rhs_str = self.generate_expression_prec(rhs, prec.saturating_add(1));
                let result = format!("{} {} {}", lhs_str, Self::generate_operator(*op), rhs_str);

                if prec < min_prec {
                    format!("({result})")
                } else {
                    result
                }
            }
            _ => self.generate_atom(expression),
        }
    }

    /// Generate the smallest level of code (an atom).
    pub(crate) fn generate_atom(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(literal) => Self::generate_literal(literal),
            Expression::Identifier(name) => self.generate_identifier(name),
            Expression::FieldAccess { base, field } => {
                let base = self.generate_expression(base);
                format!("{base}.{field}")
            }
            Expression::QualifiedPath { segments } => self.generate_qualified_path(segments),
            Expression::Call { func, args } => self.generate_call(func, args),
            Expression::MethodCall {
                receiver,
                method,
                args,
            } => self.generate_method_call(receiver, method, args),
            Expression::StructLiteral {
                struct_name,
                fields,
            } => self.generate_struct_literal(struct_name.as_deref(), fields),
            Expression::EnumLiteral {
                enum_name,
                variant,
                fields,
            } => self.generate_enum_literal(enum_name, variant, fields),
            Expression::FormatConcat { parts } => parts
                .iter()
                .map(|part| self.generate_expression(part))
                .collect::<Vec<_>>()
                .join(" .. "),
            Expression::Array { elements } => {
                let elements = elements
                    .iter()
                    .map(|element| self.generate_expression(element))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {elements} }}")
            }
            Expression::Index { base, key } => self.generate_index(base, key),
            Expression::Not(inner) => self.generate_not(inner),
            Expression::Len(inner) => {
                let inner = self.generate_expression(inner);
                format!("#{inner}")
            }
            Expression::If {
                condition,
                then_expr,
                else_expr,
            } => self.generate_if_expr(condition, then_expr, else_expr),
            Expression::Closure { params, body } => self.generate_closure(params, body),
            Expression::BinaryOp { .. } => {
                unreachable!("binary operators are handled by generate_expression_prec")
            }
        }
    }

    /// Resolve a bare name: exported `pub fn`s become `module.name`, locals stay bare.
    fn generate_identifier(&self, name: &str) -> String {
        if self.exported_functions.contains(name)
            && let Some(module_table) = &self.current_module_table
        {
            return format!("{module_table}.{name}");
        }
        name.to_string()
    }

    fn generate_qualified_path(&self, segments: &[String]) -> String {
        if let Some((struct_name, table_path)) = &self.struct_table_context
            && segments
                .first()
                .is_some_and(|segment| segment == struct_name)
        {
            let suffix = segments
                .get(1..)
                .map_or_else(String::new, |rest| rest.join("."));
            if suffix.is_empty() {
                return table_path.clone();
            }
            return format!("{table_path}.{suffix}");
        }

        segments.join(".")
    }

    fn generate_call(&self, func: &Expression, args: &[Expression]) -> String {
        if let Expression::QualifiedPath { segments } = func
            && args.is_empty()
            && segments
                .last()
                .is_some_and(|s| s == "new" || s == "default")
        {
            match segments[0].as_str() {
                "LuaAny" => return "nil".to_string(),
                "Vec" if segments.last().is_some_and(|s| s == "new") => {
                    return "{}".to_string();
                }
                _ if segments.last().is_some_and(|s| s == "default") => {
                    return "{}".to_string();
                }
                _ => {}
            }
        }

        let func_is_closure = matches!(func, Expression::Closure { .. });
        let func = self.generate_expression(func);
        let args = self.generate_arg_list(args);
        if func_is_closure {
            format!("({func})({args})")
        } else {
            format!("{func}({args})")
        }
    }

    fn generate_method_call(
        &self,
        receiver: &Expression,
        method: &str,
        args: &[Expression],
    ) -> String {
        // `storage.get(key)` -> `storage[key]` (missing -> nil / Option::None).
        // Must run before the settings `.get` rewrite (`recv[key].value`).
        if method == "get" && args.len() == 1 && is_storage_receiver(receiver) {
            let receiver = self.generate_expression(receiver);
            let key = self.generate_expression(&args[0]);
            return format!("{receiver}[{key}]");
        }

        if method == "get" && args.len() == 1 {
            let receiver = self.generate_expression(receiver);
            let key = self.generate_expression(&args[0]);
            return format!("{receiver}[{key}].value");
        }

        // `storage.set(key, value)` -> `storage[key] = value` (Factorio persistent table).
        if method == "set" && args.len() == 2 && is_storage_receiver(receiver) {
            let receiver = self.generate_expression(receiver);
            let key = self.generate_expression(&args[0]);
            let value = self.generate_expression(&args[1]);
            return format!("{receiver}[{key}] = {value}");
        }

        if method == "len" && args.is_empty() {
            let receiver = self.generate_expression(receiver);
            return format!("#{receiver}");
        }

        if method == "push" && args.len() == 1 {
            let receiver = self.generate_expression(receiver);
            let item = self.generate_expression(&args[0]);
            return format!("table.insert({receiver}, {item})");
        }

        if method == "is_empty" && args.is_empty() {
            let receiver = self.generate_expression(receiver);
            return format!("#{receiver} == 0");
        }

        let trimmed = trim_trailing_nils(args);
        if trimmed.is_empty() {
            let receiver = self.generate_expression(receiver);
            // Zero-arg API attributes are property reads (`entity.surface`).
            // Calls that only passed trailing `None`s stay invocations (`entity.die()`).
            if args.is_empty() {
                return format!("{receiver}.{method}");
            }
            return format!("{receiver}.{method}()");
        }

        // Attribute writers (`set_caption` / `write_driving`) -> property assign.
        // Real Factorio `set_*` methods and user methods are absent from the lookup.
        if trimmed.len() == 1
            && let Some(property) = attribute_property_for_setter(method)
        {
            let receiver = self.generate_expression(receiver);
            let value = self.generate_expression(&trimmed[0]);
            return format!("{receiver}.{property} = {value}");
        }

        let receiver = self.generate_expression(receiver);
        let args_lua = self.generate_arg_list(trimmed);
        format!("{receiver}.{method}({args_lua})")
    }

    /// Join call arguments, omitting trailing `nil` so Factorio optional params stay unset.
    fn generate_arg_list(&self, args: &[Expression]) -> String {
        trim_trailing_nils(args)
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn generate_struct_literal(
        &self,
        struct_name: Option<&str>,
        fields: &[(String, Expression)],
    ) -> String {
        let injected_type = struct_name.and_then(prototype_lua_type);
        let type_prefix = injected_type.map(|t| format!("type = \"{t}\", "));

        let field_strs = fields
            .iter()
            .filter(|(name, value)| {
                if matches!(
                    value,
                    Expression::Literal(factorio_ir::literal::Literal::Nil)
                ) {
                    return false;
                }
                injected_type.is_none() || (name != "type" && name != "r#type")
            })
            .map(|(name, value)| {
                let lua_name = if name == "r#type" {
                    "type"
                } else {
                    name.as_str()
                };
                format!("{lua_name} = {}", self.generate_expression(value))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let inner = match type_prefix {
            Some(prefix) if !field_strs.is_empty() => format!("{prefix}{field_strs}"),
            Some(prefix) => prefix.trim_end_matches(", ").to_string(),
            None => field_strs,
        };
        let literal = format!("{{ {inner} }}");

        if let Some((_, table_path)) = &self.struct_table_context {
            format!("setmetatable({literal}, {{ __index = {table_path} }})")
        } else {
            literal
        }
    }

    fn generate_enum_literal(
        &self,
        enum_name: &str,
        variant: &str,
        fields: &[(String, Expression)],
    ) -> String {
        let mut parts = vec![format!("tag = \"{variant}\"")];
        parts.extend(
            fields
                .iter()
                .map(|(name, value)| format!("{name} = {}", self.generate_expression(value))),
        );
        let literal = format!("{{ {} }}", parts.join(", "));
        if let Some((name, table_path)) = &self.struct_table_context
            && name == enum_name
        {
            format!("setmetatable({literal}, {{ __index = {table_path} }})")
        } else {
            literal
        }
    }

    fn generate_index(&self, base: &Expression, key: &Expression) -> String {
        let base = self.generate_expression(base);

        // Lua is 1-indexed: shift Rust integer literals (`0` -> `1`, `1` -> `2`, ...).
        // Variable indices are left as-is (callers should use 1-based values).
        let key = match key {
            Expression::Literal(factorio_ir::literal::Literal::Int(index)) => {
                (*index + 1).to_string()
            }
            _ => self.generate_expression(key),
        };
        format!("{base}[{key}]")
    }

    fn generate_not(&self, inner: &Expression) -> String {
        if let Expression::MethodCall {
            receiver,
            method,
            args,
        } = inner
            && method == "is_empty"
            && args.is_empty()
        {
            let receiver = self.generate_expression(receiver);
            return format!("#{receiver} ~= 0");
        }

        let needs_parens = matches!(inner, Expression::BinaryOp { .. });
        let inner_str = self.generate_expression(inner);
        if needs_parens {
            format!("not ({inner_str})")
        } else {
            format!("not {inner_str}")
        }
    }

    /// Emit a real Lua if/else inside an IIFE so falsey then-arms stay correct.
    fn generate_if_expr(
        &self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
    ) -> String {
        let condition = self.generate_expression(condition);
        let then_expr = self.generate_expression(then_expr);
        let else_expr = self.generate_expression(else_expr);
        format!(
            "(function() if {condition} then return {then_expr} else return {else_expr} end end)()"
        )
    }

    fn generate_closure(&self, params: &[String], body: &factorio_ir::block::Block) -> String {
        let params = params.join(", ");
        // Single-statement `return expr` -> compact one-liner.
        if let [factorio_ir::statement::Statement::Return(Some(expr))] = body.statements.as_slice()
        {
            let expr = self.generate_expression(expr);
            return format!("function({params}) return {expr} end");
        }

        let mut temp = self.fork_expr_emitter();
        let _ = writeln!(temp.output, "function({params})");
        temp.indent_level = 1;
        let _ = temp.generate_block(body, None);
        temp.indent_level = 0;
        temp.write_line("end");
        // Drop the trailing newline so call-sites can append `)` cleanly.
        while temp.output.ends_with('\n') {
            temp.output.pop();
        }
        temp.output
    }
}

fn is_storage_receiver(receiver: &Expression) -> bool {
    match receiver {
        Expression::Identifier(name) => name == "storage",
        Expression::QualifiedPath { segments } => {
            segments.last().is_some_and(|name| name == "storage")
        }
        _ => false,
    }
}

/// Drop trailing `nil` literals from call/method argument lists.
fn trim_trailing_nils(args: &[Expression]) -> &[Expression] {
    let mut end = args.len();
    while end > 0 {
        match &args[end - 1] {
            Expression::Literal(factorio_ir::literal::Literal::Nil) => end -= 1,
            _ => break,
        }
    }
    &args[..end]
}
