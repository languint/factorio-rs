use factorio_ir::expression::Expression;

use crate::LuaGenerator;

mod calls;
mod closure;
mod enums;
mod if_expr;
mod reciever;
mod tables;
mod trim;

impl LuaGenerator {
    #[must_use]
    pub fn generate_expression(&self, expression: &Expression) -> String {
        self.generate_expression_prec(expression, 0)
    }

    #[must_use]
    pub fn generate_expression_prec(&self, expression: &Expression, min_prec: u8) -> String {
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
    #[must_use]
    pub fn generate_atom(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(literal) => Self::generate_literal(literal),
            Expression::Identifier(name) => self.generate_identifier(name),
            Expression::FieldAccess { base, field } => {
                let base_lua = self.generate_expression(base);
                // Inside `impl` methods, `self.field` must not fall through `__index` to a
                // method of the same name (unset `name` -> `:name` function -> Factorio
                // "Value (function) can't be saved in property tree").
                if self.struct_table_context.is_some()
                    && matches!(base.as_ref(), Expression::Identifier(name) if name == "self")
                {
                    format!("rawget({base_lua}, \"{field}\")")
                } else {
                    format!("{base_lua}.{field}")
                }
            }
            Expression::QualifiedPath { segments } => self.generate_qualified_path(segments),
            Expression::Call { func, args } => self.generate_call(func, args),
            Expression::MethodCall {
                receiver,
                method,
                args,
                dispatch,
            } => self.generate_method_call(receiver, method, args, *dispatch),
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
            Expression::FatPointer { data, vtable } => {
                let data = self.generate_expression(data);
                format!("{{ _data = {data}, _vt = {vtable} }}")
            }
            Expression::DynMethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.generate_expression(receiver);
                let args_lua = self.generate_arg_list(args);
                if args_lua.is_empty() {
                    format!("{recv}._vt.{method}({recv})")
                } else {
                    format!("{recv}._vt.{method}({recv}, {args_lua})")
                }
            }
            Expression::BinaryOp { .. } => {
                unreachable!("binary operators are handled by generate_expression_prec")
            }
        }
    }

    /// Resolve a bare name: exported `pub fn` / `pub const` become `module.name`.
    fn generate_identifier(&self, name: &str) -> String {
        if self.exported_functions.contains(name)
            && let Some(module_table) = &self.current_module_table
        {
            return format!("{module_table}.{name}");
        }
        name.to_string()
    }

    fn generate_qualified_path(&self, segments: &[String]) -> String {
        if let Some((struct_name, table_path, _)) = &self.struct_table_context
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

        // Same-module `Widget.from_frame` / `State.new` -> `sharedWidget.Widget.from_frame`.
        if let Some(module_table) = &self.current_module_table
            && let Some(first) = segments.first()
            && self.module_type_names.contains(first)
        {
            return format!("{module_table}.{}", segments.join("."));
        }

        segments.join(".")
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
            ..
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
}
