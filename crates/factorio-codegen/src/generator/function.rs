use factorio_ir::{function::Function, module::Module, scope::Scope};

use crate::{LuaGenerator, LuaGeneratorError, LuaGeneratorResult};

impl LuaGenerator {
    /// Generate code for a given [`Function`]
    pub(crate) fn generate_function(
        &mut self,
        func: &Function,
        module: Option<&Module>,
        scope: Scope,
        module_name: Option<&str>,
    ) -> LuaGeneratorResult<()> {
        // We cannot have a private function and export it.
        if scope == Scope::Private && module_name.is_some() {
            return Err(LuaGeneratorError::FunctionLocalAndExported(
                func.name.clone(),
            ));
        }

        let prefix = match scope {
            Scope::Private if self.forward_declared_locals.contains(&func.name) => "",
            Scope::Private => "local ",
            Scope::Public => "",
        };

        let function_uses_self = func
            .params
            .first()
            .is_some_and(|parameter| parameter.name == "self");

        // Skip emitting self parameters since lua automatically passes that with :func()
        let params = if function_uses_self && module_name.is_some() {
            func.params
                .iter()
                .skip(1)
                .map(|parameter| self.format_parameter(parameter))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            func.params
                .iter()
                .map(|parameter| self.format_parameter(parameter))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let return_comment = self.function_return_comment(
            func.debug
                .as_ref()
                .and_then(|debug| debug.return_type.as_deref()),
        );

        let function_name = match module_name {
            Some(module_name) if function_uses_self => {
                format!("{module_name}:{}", func.name)
            }
            Some(module_name) => format!("{module_name}.{}", func.name),
            None => func.name.clone(),
        };

        self.write_doc_comments(func.doc.as_deref());

        if self.debug_level_at_least(0)
            && let Some(debug) = &func.debug
        {
            self.write_line(&format!("-- {}", debug.header_comment));
        }

        self.write_line(&format!(
            "{prefix}function {function_name}({params}){return_comment}"
        ));

        self.indent_level += 1;
        self.generate_block(&func.body, module)?;
        self.indent_level -= 1;

        self.write_line("end");

        Ok(())
    }
}
