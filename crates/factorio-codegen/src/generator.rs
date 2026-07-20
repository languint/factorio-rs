use std::fmt::Write as _;

use factorio_ir::{
    block::Block, literal::Literal, module::Module, scope::Scope, statement::Statement,
};

use crate::generator::error::LuaGeneratorResult;

pub mod comment;
pub mod error;
pub mod expression;
pub mod function;
pub mod operator;
pub mod statement;
pub mod table;

pub struct LuaGenerator {
    output: String,
    indent_level: usize,
    /// Rewrites `StructName.associated` paths while generating struct methods.
    struct_table_context: Option<(String, String)>,
    debug_level: Option<u8>,
    mod_name: String,
    /// Depth of nested `for` / `while` loops, used for `::__continue_N::` labels.
    loop_depth: usize,
    /// Optional prefix prepended to every module's filename and local require variable.
    /// Empty string means no prefix.
    module_prefix: String,
    /// Active transpile profile name (`debug`, `release`, ...), if known.
    profile: Option<String>,
    exported_functions: std::collections::HashSet<String>,
    current_module_table: Option<String>,
    /// Locals forward-declared before vtables so closures capture upvalues.
    forward_declared_locals: std::collections::HashSet<String>,
}

impl Default for LuaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self::with_mod_name("mod")
    }

    #[must_use]
    pub fn with_mod_name(mod_name: impl Into<String>) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            struct_table_context: None,
            debug_level: None,
            mod_name: mod_name.into(),
            loop_depth: 0,
            module_prefix: String::new(),
            profile: None,
            exported_functions: std::collections::HashSet::new(),
            current_module_table: None,
            forward_declared_locals: std::collections::HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_debug_level(debug_level: u8) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            struct_table_context: None,
            debug_level: Some(debug_level),
            mod_name: "mod".to_string(),
            loop_depth: 0,
            module_prefix: String::new(),
            profile: None,
            exported_functions: std::collections::HashSet::new(),
            current_module_table: None,
            forward_declared_locals: std::collections::HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_mod_name_and_debug(mod_name: impl Into<String>, debug_level: u8) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            struct_table_context: None,
            debug_level: Some(debug_level),
            mod_name: mod_name.into(),
            loop_depth: 0,
            module_prefix: String::new(),
            profile: None,
            exported_functions: std::collections::HashSet::new(),
            current_module_table: None,
            forward_declared_locals: std::collections::HashSet::new(),
        }
    }

    /// Set the module prefix applied to all generated module filenames and local names.
    #[must_use]
    pub fn with_module_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.module_prefix = prefix.into();
        self
    }

    /// Record the transpile profile name in generated file headers.
    #[must_use]
    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    const fn debug_level_at_least(&self, level: u8) -> bool {
        matches!(self.debug_level, Some(current) if current >= level)
    }

    /// Clone generator state into a scratch buffer for emitting nested expression forms
    /// (e.g. closures) without touching the primary output.
    fn fork_expr_emitter(&self) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            struct_table_context: self.struct_table_context.clone(),
            debug_level: self.debug_level,
            mod_name: self.mod_name.clone(),
            loop_depth: 0,
            module_prefix: self.module_prefix.clone(),
            profile: self.profile.clone(),
            exported_functions: self.exported_functions.clone(),
            current_module_table: self.current_module_table.clone(),
            forward_declared_locals: self.forward_declared_locals.clone(),
        }
    }

    pub const INDENT: &'static str = "\t";

    /// Returns a string indented to [`LuaGenerator::indent_level`].
    fn indent(&self) -> String {
        Self::INDENT.repeat(self.indent_level)
    }

    /// Writes a line of text to [`LuaGenerator::output`] terminated by a newline `\n`.
    fn write_line(&mut self, line: &str) {
        if !line.is_empty() {
            self.output.push_str(&self.indent());
            self.output.push_str(line);
        }
        self.output.push('\n');
    }

    fn write_doc_comments(&mut self, doc: Option<&str>) {
        let Some(doc) = doc else {
            return;
        };

        for line in doc.lines() {
            if line.is_empty() {
                self.write_line("--");
            } else {
                self.write_line(&format!("-- {line}"));
            }
        }
    }

    pub const MODULE_META_START: &'static str =
        concat!("-- Generated by factorio-rs@", env!("CARGO_PKG_VERSION"));

    fn generate_module_meta(&self, module: &Module) -> String {
        let mut header = format!("{}\n", Self::MODULE_META_START);
        if let Some(profile) = &self.profile {
            let _ = writeln!(header, "-- Profile: {profile}");
        }
        let _ = write!(header, "-- Module: `{}`", module.name);
        header
    }

    /// Returns the module identifier that exported symbols will be attached to, e.g
    /// `bound_detector` -> `boundDetector`, `player.extra_info` -> `playerExtraInfo`.
    fn module_identifier(module_name: &str) -> String {
        heck::AsLowerCamelCase(module_name.replace('.', "_")).to_string()
    }

    fn mod_require_path(&self, module_path: &str) -> String {
        self.require_path(&self.mod_name, "lua", module_path, true)
    }

    fn import_require_path(&self, import: &factorio_ir::module::ModuleImport) -> String {
        let own_mod = import.factorio_mod.is_none();
        let mod_name = import
            .factorio_mod
            .as_deref()
            .unwrap_or(self.mod_name.as_str());
        let module_root = import.module_root.as_deref().unwrap_or("lua");
        self.require_path(mod_name, module_root, &import.module, own_mod)
    }

    fn require_path(
        &self,
        mod_name: &str,
        module_root: &str,
        module_path: &str,
        apply_prefix: bool,
    ) -> String {
        let path = module_path.replace('.', "/");
        let path = if apply_prefix {
            self.apply_path_prefix(&path)
        } else {
            path
        };
        if module_root.is_empty() {
            format!("__{mod_name}__/{path}")
        } else {
            format!("__{mod_name}__/{module_root}/{path}")
        }
    }

    #[must_use]
    pub fn apply_path_prefix(&self, path: &str) -> String {
        if self.module_prefix.is_empty() {
            return path.to_string();
        }
        path.rfind('/').map_or_else(
            || format!("{}_{}", self.module_prefix, path),
            |slash| {
                format!(
                    "{}/{prefix}_{}",
                    &path[..slash],
                    &path[slash + 1..],
                    prefix = self.module_prefix
                )
            },
        )
    }

    /// Return the prefixed local variable name for a module import.
    #[must_use]
    pub fn prefixed_local(&self, local: &str) -> String {
        if self.module_prefix.is_empty() {
            local.to_string()
        } else {
            format!("{}_{}", self.module_prefix, local)
        }
    }

    /// Generate lua code for a single `module`.
    ///
    /// # Errors
    /// Returns `Err` if parsing the AST fails.
    pub fn generate_module(&mut self, module: &Module) -> LuaGeneratorResult<String> {
        self.output.clear();
        self.indent_level = 0;
        self.exported_functions.clear();
        self.forward_declared_locals.clear();

        for symbol in &module.symbols {
            if let Statement::FunctionDecl(function) = &symbol.statement {
                self.exported_functions.insert(function.name.clone());
            }
        }

        let module_name = Self::module_identifier(&module.name);
        // Qualify exported fn references even in private body code; those bodies only
        // run after the module table exists (events / commands), not during load.
        self.current_module_table = Some(module_name.clone());

        let module_header = self.generate_module_meta(module);
        self.output.push_str(&module_header);
        self.output.push('\n');

        self.generate_imports(&module.imports);

        // Forward-declare private concrete type locals so vtable closures capture
        // upvalues when structs are assigned later in the body.
        self.generate_vtables(&module.vtables, Some(&module_name), module);

        for statement in &module.body.statements {
            self.generate_statement(statement, Some(module), None, Scope::Private)?;
        }

        let (exporter_start, exporter_end) = Self::generate_symbol_exporter(&module_name);
        self.write_line(&exporter_start);

        if !module.submodules.is_empty() {
            self.write_line(&format!(
                "package.loaded[\"{}\"] = {module_name}",
                self.mod_require_path(&module.name)
            ));
        }

        for symbol in &module.symbols {
            self.generate_statement(
                &symbol.statement,
                Some(module),
                Some(&module_name),
                symbol.scope,
            )?;
        }

        self.generate_submodules(&module.submodules);

        self.write_line(&exporter_end);
        self.current_module_table = None;
        self.exported_functions.clear();

        Ok(self.output.clone())
    }

    fn generate_imports(&mut self, imports: &[factorio_ir::module::ModuleImport]) {
        for import in imports {
            // `import.local` already has the module prefix baked in at the IR level.
            self.write_line(&format!(
                "local {} = require(\"{}\")",
                import.local,
                self.import_require_path(import)
            ));

            for item in &import.items {
                self.write_line(&format!(
                    "local {} = {}.{}",
                    item.local, import.local, item.name
                ));
            }
        }

        if !imports.is_empty() {
            self.output.push('\n');
        }
    }

    fn generate_vtables(
        &mut self,
        vtables: &[factorio_ir::module::VTable],
        module_name: Option<&str>,
        module: &Module,
    ) {
        let mut forward = std::collections::BTreeSet::new();
        for vtable in vtables {
            if !concrete_type_is_public(module, &vtable.concrete_type) {
                forward.insert(vtable.concrete_type.clone());
            }
        }
        if !forward.is_empty() {
            let names = forward.iter().cloned().collect::<Vec<_>>().join(", ");
            self.write_line(&format!("local {names}"));
            self.forward_declared_locals.extend(forward);
            self.output.push('\n');
        }

        for vtable in vtables {
            let concrete_path =
                resolve_concrete_table_path(module, module_name, &vtable.concrete_type);
            self.write_line(&format!("local {} = {{", vtable.name));
            self.indent_level += 1;
            for method in &vtable.methods {
                self.write_line(&format!(
                    "{method} = function(self, ...) return {concrete_path}.{method}(self._data, ...) end,"
                ));
            }
            self.indent_level -= 1;
            self.write_line("}");
        }
        if !vtables.is_empty() {
            self.output.push('\n');
        }
    }

    fn generate_submodules(&mut self, submodules: &[String]) {
        for submodule in submodules {
            self.write_line(&format!(
                "require(\"{}\")",
                self.mod_require_path(submodule)
            ));
        }

        if !submodules.is_empty() {
            self.output.push('\n');
        }
    }

    /// Return the starting and ending lines for exporting symbols.
    fn generate_symbol_exporter(module_name: &str) -> (String, String) {
        (
            format!("local {module_name} = {{}}"),
            format!("return {module_name}"),
        )
    }

    fn generate_block(&mut self, block: &Block, module: Option<&Module>) -> LuaGeneratorResult<()> {
        for statement in &block.statements {
            self.generate_statement(statement, module, None, Scope::Private)?;
        }
        Ok(())
    }

    fn generate_literal(literal: &Literal) -> String {
        match literal {
            Literal::Int(value) => value.to_string(),
            Literal::Float(value) => value.to_string(),
            Literal::String(value) => format!("\"{}\"", escape_lua_string(value)),
            Literal::Bool(value) => value.to_string(),
            Literal::Nil => "nil".to_string(),
        }
    }

    fn format_parameter(&self, parameter: &factorio_ir::function::Parameter) -> String {
        let type_comment = self.parameter_type_comment(parameter.source_type.as_deref());
        format!("{}{type_comment}", parameter.name)
    }
}

fn escape_lua_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn concrete_type_is_public(module: &Module, concrete_type: &str) -> bool {
    module.symbols.iter().any(|symbol| {
        matches!(
            &symbol.statement,
            Statement::StructDecl(s) if s.name == concrete_type
        ) || matches!(
            &symbol.statement,
            Statement::EnumDecl(e) if e.name == concrete_type
        )
    })
}

fn resolve_concrete_table_path(
    module: &Module,
    module_name: Option<&str>,
    concrete_type: &str,
) -> String {
    if concrete_type_is_public(module, concrete_type)
        && let Some(module_name) = module_name
    {
        format!("{module_name}.{concrete_type}")
    } else {
        concrete_type.to_string()
    }
}
