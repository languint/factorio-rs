use factorio_ir::{module::Module, scope::Scope, statement::Statement};

use crate::{LuaGenerator, LuaGeneratorResult};

fn body_has_continue(body: &[Statement]) -> bool {
    body.iter().any(|s| match s {
        Statement::Continue => true,
        Statement::Conditional {
            then_block,
            else_block,
            ..
        } => body_has_continue(then_block) || body_has_continue(else_block),
        Statement::ForIn { body, .. }
        | Statement::ForNumeric { body, .. }
        | Statement::While { body, .. } => body_has_continue(body),
        _ => false,
    })
}

impl LuaGenerator {
    /// Generate code for a given [`Statement`].
    #[allow(clippy::too_many_lines)]
    pub(crate) fn generate_statement(
        &mut self,
        statement: &Statement,
        module: Option<&Module>,
        module_name: Option<&str>,
        scope: Scope,
    ) -> LuaGeneratorResult<()> {
        match statement {
            Statement::FunctionDecl(function) => {
                self.generate_function(function, module, scope, module_name)?;
            }
            Statement::StructDecl(struct_decl) => {
                self.generate_struct(struct_decl, module, scope, module_name)?;
            }
            Statement::EnumDecl(enum_decl) => {
                self.generate_enum(enum_decl, module, scope, module_name)?;
            }
            Statement::VariableDecl {
                name,
                value,
                source_type,
                ..
            } => {
                let value = self.generate_expression(value);
                let type_comment = self.variable_type_comment(source_type.as_deref());
                let line = match (scope, module_name) {
                    (Scope::Public, Some(module_name)) => {
                        format!("{module_name}.{name}{type_comment} = {value}")
                    }
                    _ => format!("local {name}{type_comment} = {value}"),
                };
                self.write_line(&line);
            }
            Statement::Assignment { target, value } => {
                let target = self.generate_expression(target);
                let value = self.generate_expression(value);
                self.write_line(&format!("{target} = {value}"));
            }
            Statement::Conditional {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.generate_expression(condition);
                self.write_line(&format!("if {condition} then"));

                self.indent_level += 1;
                for statement in then_block {
                    self.generate_statement(statement, module, module_name, Scope::Private)?;
                }
                self.indent_level -= 1;

                if !else_block.is_empty() {
                    self.write_line("else");
                    self.indent_level += 1;
                    for statement in else_block {
                        self.generate_statement(statement, module, module_name, Scope::Private)?;
                    }
                    self.indent_level -= 1;
                }
                self.write_line("end");
            }
            Statement::Return(value) => {
                let line = value.as_ref().map_or_else(
                    || "return".to_string(),
                    |value| format!("return {}", self.generate_expression(value)),
                );
                self.write_line(&line);
            }
            Statement::Expr(expression) => {
                self.write_line(&self.generate_expression(expression));
            }
            Statement::ForIn {
                var,
                iter,
                body,
                ipairs,
            } => {
                self.loop_depth += 1;
                let depth = self.loop_depth;
                let iter = self.generate_expression(iter);
                let iterator = if *ipairs { "ipairs" } else { "pairs" };
                self.write_line(&format!("for _, {var} in {iterator}({iter}) do"));
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt, module, module_name, Scope::Private)?;
                }
                if body_has_continue(body) {
                    self.write_line(&format!("::__continue_{depth}::"));
                }
                self.indent_level -= 1;
                self.write_line("end");
                self.loop_depth -= 1;
            }
            Statement::ForNumeric {
                var,
                start,
                limit,
                body,
            } => {
                self.loop_depth += 1;
                let depth = self.loop_depth;
                let start = self.generate_expression(start);
                let limit = self.generate_expression(limit);
                self.write_line(&format!("for {var} = {start}, {limit} do"));
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt, module, module_name, Scope::Private)?;
                }
                if body_has_continue(body) {
                    self.write_line(&format!("::__continue_{depth}::"));
                }
                self.indent_level -= 1;
                self.write_line("end");
                self.loop_depth -= 1;
            }
            Statement::While { condition, body } => {
                self.loop_depth += 1;
                let depth = self.loop_depth;
                let condition = self.generate_expression(condition);
                self.write_line(&format!("while {condition} do"));
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt, module, module_name, Scope::Private)?;
                }
                if body_has_continue(body) {
                    self.write_line(&format!("::__continue_{depth}::"));
                }
                self.indent_level -= 1;
                self.write_line("end");
                self.loop_depth -= 1;
            }
            Statement::Continue => {
                self.write_line(&format!("goto __continue_{}", self.loop_depth));
            }
            Statement::Break => {
                self.write_line("break");
            }
        }

        Ok(())
    }
}
