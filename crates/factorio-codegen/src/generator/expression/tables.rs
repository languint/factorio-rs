use factorio_ir::expression::Expression;

use crate::{LuaGenerator, prototype_lua_typename};

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

impl LuaGenerator {
    pub fn generate_struct_literal(
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

    /// Array of `{ key, value }` structs -> `{ [key] = value, ... }`.
    #[must_use]
    pub fn generate_string_pair_table(&self, pairs_expr: &Expression) -> String {
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

    #[must_use]
    pub fn metatable_expr(&self, type_name: &str, table_path: &str) -> String {
        self.shared_metatable_locals
            .get(type_name)
            .cloned()
            .unwrap_or_else(|| format!("{{ __index = {table_path} }}"))
    }
}
