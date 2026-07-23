use std::fmt::Write as _;

use factorio_ir::expression::Expression;

use crate::{
    LuaGenerator, attribute_property_for_setter, is_factorio_attribute_read, is_factorio_method,
    prototype_lua_typename,
};

/// User-struct / metatable methods need `:method(...)` so Lua passes `self`.
/// Factorio `LuaObject` methods use `.method(...)` (colon would pass an extra
/// argument and error at runtime). Unknown names default to `:`.
const USER_COLON_METHODS: &[&str] = &[
    "get",
    "set",
    "caption",
    "name",
    "ensure_name",
    "with_root_name",
    "child",
    "direction",
    "align_horizontal",
    "align_vertical",
    "centered",
    "on_click",
    "mount",
    "use_state",
    "from_frame",
    "from_text",
    "from_button",
    "from_flow",
    "from_line",
    "from_scroll_pane",
    "into",
    "new",
    "horizontal_scroll_policy",
    "vertical_scroll_policy",
    "text",
    "numeric",
    "allow_decimal",
    "allow_negative",
    "is_password",
    "lose_focus_on_confirm",
    "on_text_changed",
    "on_confirmed",
    "resize_to_sprite",
    "clicked_sprite",
    "hovered_sprite",
    "number",
    "selected_index",
    "on_selection_changed",
    "state",
    "on_checked",
    "minimum_value",
    "maximum_value",
    "value",
    "value_step",
    "discrete_values",
    "on_value_changed",
];

fn method_call_sep(method: &str) -> &'static str {
    if USER_COLON_METHODS.contains(&method) || !is_factorio_method(method) {
        ":"
    } else {
        "."
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

        // Typed settings accessors share the same Lua shape as `.get`.
        if matches!(
            method,
            "get_bool" | "get_int" | "get_double" | "get_string" | "setting"
        ) && args.len() == 1
        {
            let receiver = self.generate_expression(receiver);
            let key = self.generate_expression(&args[0]);
            if method == "setting" {
                return format!("{receiver}[{key}]");
            }
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
            // Zero-arg API *attributes* are property reads (`entity.surface`).
            // Everything else is an invocation. Trailing-`None`-only calls stay
            // invocations (`entity.die()`). User / unknown names use `:`.
            if args.is_empty() && is_factorio_attribute_read(method) {
                return format!("{receiver}.{method}");
            }
            let sep = method_call_sep(method);
            return format!("{receiver}{sep}{method}()");
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
        let sep = method_call_sep(method);
        // Factorio LuaObjects: `.method(args)` (engine binds self).
        // User structs / cross-mod builders: `:method(args)` via `__index`.
        format!("{receiver}{sep}{method}({args_lua})")
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
        if let Some(literal) = self.try_special_struct_literal(struct_name, fields) {
            return self.maybe_struct_metatable(literal, struct_name);
        }

        // Recipe ingredients: `type = "item"` or `"fluid"` from the `fluid` bool field.
        let (injected_type, skip_fields): (Option<&str>, &[&str]) =
            if struct_name == Some("RecipeIngredient") {
                let is_fluid = fields.iter().any(|(n, v)| {
                    n == "fluid"
                        && matches!(
                            v,
                            Expression::Literal(factorio_ir::literal::Literal::Bool(true))
                        )
                });
                (Some(if is_fluid { "fluid" } else { "item" }), &["fluid"])
            } else {
                (struct_name.and_then(prototype_lua_typename), &[])
            };

        let type_prefix = injected_type.map(|t| format!("type = \"{t}\", "));
        let field_strs = fields
            .iter()
            .filter(|(name, value)| {
                if skip_fields.contains(&name.as_str()) {
                    return false;
                }
                if matches!(
                    value,
                    Expression::Literal(factorio_ir::literal::Literal::Nil)
                ) {
                    return false;
                }
                // Omit false optional flags that only affect type injection.
                if name == "fluid"
                    && matches!(
                        value,
                        Expression::Literal(factorio_ir::literal::Literal::Bool(false))
                    )
                {
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
        self.maybe_struct_metatable(format!("{{ {inner} }}"), struct_name)
    }

    /// Special Factorio shapes (tech ingredients, flag sets, `Tags`, `BoundingBox`).
    fn try_special_struct_literal(
        &self,
        struct_name: Option<&str>,
        fields: &[(String, Expression)],
    ) -> Option<String> {
        // Research ingredients are Factorio tuples `{ "pack", amount }`, not named tables.
        if struct_name == Some("TechnologyUnitIngredient") {
            let name = fields
                .iter()
                .find_map(|(n, v)| (n == "name").then(|| self.generate_expression(v)))?;
            let amount = fields
                .iter()
                .find_map(|(n, v)| (n == "amount").then(|| self.generate_expression(v)))?;
            return Some(format!("{{ {name}, {amount} }}"));
        }

        // Flag sets: `{ flags = {"left", "right"} }` -> `{ ["left"] = true, ... }`.
        if let Some(name) = struct_name
            && is_flag_set_struct(name)
            && let Some(flags_expr) = fields.iter().find_map(|(n, v)| (n == "flags").then_some(v))
        {
            return Some(generate_flag_set_table(flags_expr));
        }

        // Tags / PropertyExpressionNames: `{ pairs = [...] }` -> `{ [key] = value, ... }`.
        if matches!(struct_name, Some("Tags" | "PropertyExpressionNames"))
            && let Some(pairs_expr) = fields.iter().find_map(|(n, v)| (n == "pairs").then_some(v))
        {
            return Some(self.generate_string_pair_table(pairs_expr));
        }

        // Bounding box: Factorio expects `{{left_top}, {right_bottom}}`.
        if struct_name == Some("BoundingBox") {
            let get = |key: &str| {
                fields
                    .iter()
                    .find_map(|(n, v)| (n == key).then(|| self.generate_expression(v)))
            };
            if let (Some(lx), Some(ly), Some(rx), Some(ry)) = (
                get("left_top_x"),
                get("left_top_y"),
                get("right_bottom_x"),
                get("right_bottom_y"),
            ) {
                return Some(format!("{{{{ {lx}, {ly} }}, {{ {rx}, {ry} }}}}"));
            }
        }

        None
    }

    fn maybe_struct_metatable(&self, literal: String, struct_name: Option<&str>) -> String {
        let Some((ctx_name, table_path, has_methods)) = &self.struct_table_context else {
            return literal;
        };
        if !has_methods {
            return literal;
        }
        // Only `Self { ... }` / `Frame { ... }` inside `impl Frame` get the method
        // table - not unrelated structs like `LuaGuiElementAddParams { ... }`.
        let applies = struct_name.is_none_or(|name| name == ctx_name);
        if applies {
            let mt = self.metatable_expr(ctx_name, table_path);
            format!("setmetatable({literal}, {mt})")
        } else {
            literal
        }
    }

    fn metatable_expr(&self, type_name: &str, table_path: &str) -> String {
        self.shared_metatable_locals
            .get(type_name)
            .cloned()
            .unwrap_or_else(|| format!("{{ __index = {table_path} }}"))
    }

    /// Array of `{ key, value }` structs -> `{ [key] = value, ... }`.
    fn generate_string_pair_table(&self, pairs_expr: &Expression) -> String {
        let entries = match pairs_expr {
            Expression::Array { elements } => elements
                .iter()
                .filter_map(|item| match item {
                    Expression::StructLiteral { fields, .. } => {
                        let key = fields
                            .iter()
                            .find_map(|(n, v)| (n == "key").then(|| self.generate_expression(v)))?;
                        let value = fields.iter().find_map(|(n, v)| {
                            (n == "value").then(|| self.generate_expression(v))
                        })?;
                        Some(format!("[{key}] = {value}"))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        format!("{{ {} }}", entries.join(", "))
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
        if let Some(table_path) = self.enum_method_table_path(enum_name) {
            let mt = self.metatable_expr(enum_name, &table_path);
            format!("setmetatable({literal}, {mt})")
        } else {
            literal
        }
    }

    fn enum_method_table_path(&self, enum_name: &str) -> Option<String> {
        let Some((ctx_name, table_path, _)) = &self.struct_table_context else {
            return None;
        };
        if ctx_name == enum_name {
            return Some(table_path.clone());
        }
        if self.module_type_names.contains(enum_name)
            && let Some(module_table) = &self.current_module_table
        {
            return Some(format!("{module_table}.{enum_name}"));
        }
        None
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

fn is_flag_set_struct(name: &str) -> bool {
    matches!(
        name,
        "MouseButtonFlags"
            | "SelectionModeFlags"
            | "EntityPrototypeFlags"
            | "ItemPrototypeFlags"
            | "TriggerTargetMask"
    )
}

/// `{ "left", "right" }` array -> `{ ["left"] = true, ... }`.
fn generate_flag_set_table(flags_expr: &Expression) -> String {
    let keys = match flags_expr {
        Expression::Array { elements } => elements
            .iter()
            .filter_map(|item| match item {
                Expression::Literal(factorio_ir::literal::Literal::String(s)) => {
                    Some(format!("[\"{s}\"] = true"))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    format!("{{ {} }}", keys.join(", "))
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
