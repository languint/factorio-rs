use factorio_ir::{
    enumeration::{Enum, EnumVariantFields},
    expression::Expression,
    function::Function,
    module::Module,
    scope::Scope,
    structure::Struct,
};

use crate::{LuaGenerator, LuaGeneratorError, LuaGeneratorResult};

impl LuaGenerator {
    /// Generate a `struct`.
    pub(crate) fn generate_struct(
        &mut self,
        struct_decl: &Struct,
        module: Option<&Module>,
        scope: Scope,
        module_name: Option<&str>,
    ) -> LuaGeneratorResult<()> {
        if scope == Scope::Private && module_name.is_some() {
            return Err(LuaGeneratorError::StructLocalAndExported(
                struct_decl.name.clone(),
            ));
        }

        if let Some(module) = module
            && module.is_imported_type_extension(struct_decl)
        {
            let table_path = module
                .imported_item_local(&struct_decl.name)
                .ok_or_else(|| {
                    LuaGeneratorError::FailedToGetTablePathForStruct(struct_decl.name.clone())
                })?
                .to_string();

            for (name, value) in &struct_decl.constants {
                let value = self.generate_expression(value);
                self.write_line(&format!("{table_path}.{name} = {value}"));
            }

            for method in &struct_decl.methods {
                self.generate_table_method(method, &struct_decl.name, &table_path)?;
            }

            return Ok(());
        }

        let table_path = match (scope, module_name) {
            (Scope::Public, Some(module_name)) => {
                format!("{module_name}.{}", struct_decl.name)
            }
            (Scope::Private | Scope::Public, None) => struct_decl.name.clone(),
            (Scope::Private, Some(_)) => unreachable!(),
        };

        let prefix = if scope == Scope::Private && module_name.is_none() {
            if self.forward_declared_locals.contains(&struct_decl.name) {
                ""
            } else {
                "local "
            }
        } else {
            ""
        };

        self.write_doc_comments(struct_decl.doc.as_deref());

        if self.debug_level_at_least(0)
            && let Some(debug) = &struct_decl.debug
        {
            self.write_line(&format!("-- {}", debug.header_comment));
        }

        self.write_line(&format!("{prefix}{table_path} = {{}}"));

        for (name, value) in &struct_decl.constants {
            let value = self.generate_expression(value);
            self.write_line(&format!("{table_path}.{name} = {value}"));
        }

        for method in &struct_decl.methods {
            self.generate_table_method(method, &struct_decl.name, &table_path)?;
        }

        Ok(())
    }

    /// Generate a user-defined tagged-table enum and its method table.
    pub(crate) fn generate_enum(
        &mut self,
        enum_decl: &Enum,
        _module: Option<&Module>,
        scope: Scope,
        module_name: Option<&str>,
    ) -> LuaGeneratorResult<()> {
        if scope == Scope::Private && module_name.is_some() {
            return Err(LuaGeneratorError::StructLocalAndExported(
                enum_decl.name.clone(),
            ));
        }

        let table_path = match (scope, module_name) {
            (Scope::Public, Some(module_name)) => format!("{module_name}.{}", enum_decl.name),
            (Scope::Private | Scope::Public, None) => enum_decl.name.clone(),
            (Scope::Private, Some(_)) => unreachable!(),
        };
        let prefix = if scope == Scope::Private && module_name.is_none() {
            if self.forward_declared_locals.contains(&enum_decl.name) {
                ""
            } else {
                "local "
            }
        } else {
            ""
        };

        self.write_doc_comments(enum_decl.doc.as_deref());
        self.write_line(&format!("{prefix}{table_path} = {{}}"));

        for variant in &enum_decl.variants {
            if matches!(variant.fields, EnumVariantFields::Unit) {
                let value = self.generate_expression(&Expression::EnumLiteral {
                    enum_name: enum_decl.name.clone(),
                    variant: variant.name.clone(),
                    fields: vec![],
                });
                self.write_line(&format!("{table_path}.{} = {value}", variant.name));
            }
        }
        for (name, value) in &enum_decl.constants {
            let value = self.generate_expression(value);
            self.write_line(&format!("{table_path}.{name} = {value}"));
        }
        for method in &enum_decl.methods {
            self.generate_table_method(method, &enum_decl.name, &table_path)?;
        }
        Ok(())
    }

    /// Generate a method found on a `struct`
    pub(crate) fn generate_table_method(
        &mut self,
        func: &Function,
        struct_name: &str,
        table_path: &str,
    ) -> LuaGeneratorResult<()> {
        let function_uses_self = func
            .params
            .first()
            .is_some_and(|parameter| parameter.name == "self");

        let params = if function_uses_self {
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
        let separator = if function_uses_self { ":" } else { "." };

        self.write_doc_comments(func.doc.as_deref());

        if self.debug_level_at_least(0)
            && let Some(debug) = &func.debug
        {
            self.write_line(&format!("-- {}", debug.header_comment));
        }

        self.write_line(&format!(
            "function {table_path}{separator}{}({params}){return_comment}",
            func.name
        ));

        self.struct_table_context = Some((struct_name.to_string(), table_path.to_string()));
        self.indent_level += 1;
        self.generate_block(&func.body, None)?;
        self.indent_level -= 1;
        self.struct_table_context = None;

        self.write_line("end");

        Ok(())
    }
}
