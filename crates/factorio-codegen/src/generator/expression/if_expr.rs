use factorio_ir::expression::Expression;

use crate::LuaGenerator;

impl LuaGenerator {
    /// Emit a real Lua if/else inside an IIFE so falsey then-arms stay correct.
    #[must_use]
    pub fn generate_if_expr(
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
}
