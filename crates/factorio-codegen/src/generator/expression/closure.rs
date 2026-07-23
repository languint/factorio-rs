use std::fmt::Write;

use crate::LuaGenerator;

impl LuaGenerator {
    #[must_use]
    pub fn generate_closure(&self, params: &[String], body: &factorio_ir::block::Block) -> String {
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
