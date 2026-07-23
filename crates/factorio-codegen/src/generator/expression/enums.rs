use factorio_ir::expression::Expression;

use crate::LuaGenerator;

impl LuaGenerator {
    #[must_use]
    pub fn generate_enum_literal(
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

        if let Some(mt) = self.shared_metatable_locals.get(enum_name) {
            return format!("setmetatable({literal}, {mt})");
        }
        if let Some((ctx_name, table_path, has_methods)) = &self.struct_table_context
            && ctx_name == enum_name
            && *has_methods
        {
            let mt = self.metatable_expr(enum_name, table_path);
            return format!("setmetatable({literal}, {mt})");
        }
        literal
    }
}
