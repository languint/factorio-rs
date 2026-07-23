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
                // `v.push(x)` lowers to an assignment statement, not an expression.
                if let Some(assign) = self.generate_push_assign_stmt(value) {
                    self.write_line(&assign);
                    let type_comment = self.variable_type_comment(source_type.as_deref());
                    let line = match (scope, module_name) {
                        (Scope::Public, Some(module_name)) => {
                            format!("{module_name}.{name}{type_comment} = nil")
                        }
                        _ => format!("local {name}{type_comment} = nil"),
                    };
                    self.write_line(&line);
                    return Ok(());
                }
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
                let value = self.generate_expression(value);
                // `self.field = value` inside impl methods: use rawset so we don't
                // emit invalid `rawget(self, "field") = value` (field reads use rawget).
                if let factorio_ir::expression::Expression::FieldAccess { base, field } = target
                    && self.struct_table_context.is_some()
                    && matches!(
                        base.as_ref(),
                        factorio_ir::expression::Expression::Identifier(name) if name == "self"
                    )
                {
                    let base_lua = self.generate_expression(base);
                    self.write_line(&format!("rawset({base_lua}, \"{field}\", {value})"));
                } else {
                    let target = self.generate_expression(target);
                    self.write_line(&format!("{target} = {value}"));
                }
            }
            Statement::Conditional {
                condition,
                then_block,
                else_block,
            } => {
                self.generate_conditional_chain(
                    condition,
                    then_block,
                    else_block,
                    module,
                    module_name,
                )?;
            }
            Statement::Return(value) => {
                if let Some(value) = value.as_ref()
                    && let Some(assign) = self.generate_push_assign_stmt(value)
                {
                    // Unit-returning `v.push(x)` as a tail expr: assign, then return.
                    self.write_line(&assign);
                    self.write_line("return");
                    return Ok(());
                }
                let line = value.as_ref().map_or_else(
                    || "return".to_string(),
                    |value| format!("return {}", self.generate_expression(value)),
                );
                self.write_line(&line);
            }
            Statement::Expr(expression) => {
                if let Some(assign) = self.generate_push_assign_stmt(expression) {
                    self.write_line(&assign);
                } else {
                    self.write_line(&self.generate_expression(expression));
                }
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

    fn generate_conditional_chain(
        &mut self,
        condition: &factorio_ir::expression::Expression,
        then_block: &[Statement],
        else_block: &[Statement],
        module: Option<&Module>,
        module_name: Option<&str>,
    ) -> LuaGeneratorResult<()> {
        let condition = self.generate_expression(condition);
        self.write_line(&format!("if {condition} then"));
        self.indent_level += 1;
        for statement in then_block {
            self.generate_statement(statement, module, module_name, Scope::Private)?;
        }
        self.indent_level -= 1;

        let mut else_block = else_block;
        loop {
            match else_block {
                [
                    Statement::Conditional {
                        condition,
                        then_block,
                        else_block: nested_else,
                    },
                ] => {
                    let condition = self.generate_expression(condition);
                    self.write_line(&format!("elseif {condition} then"));
                    self.indent_level += 1;
                    for statement in then_block {
                        self.generate_statement(statement, module, module_name, Scope::Private)?;
                    }
                    self.indent_level -= 1;
                    else_block = nested_else;
                }
                [] => break,
                _ => {
                    self.write_line("else");
                    self.indent_level += 1;
                    for statement in else_block {
                        self.generate_statement(statement, module, module_name, Scope::Private)?;
                    }
                    self.indent_level -= 1;
                    break;
                }
            }
        }

        self.write_line("end");
        Ok(())
    }
}
