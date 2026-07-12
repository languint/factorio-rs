use std::collections::HashMap;

use factorio_ir::lint::{Diagnostic, LintConfig, LintId, LintLevel};

use crate::{
    error::{FrontendError, FrontendResult},
    paths::{require_local_name, split_crate_path},
};

use super::imports::ImportFragment;

pub struct LowerContext<'a> {
    pub imports: &'a mut Vec<ImportFragment>,
    /// Prefix prepended to every generated module local name to avoid shadowing
    /// Factorio built-in globals (e.g. `"ms"` -> `settings` becomes `ms_settings`).
    pub module_prefix: &'a str,
    /// Maps bare module local names -> prefixed names for rewriting path expressions
    /// of the form `module_name::item` that reference bare-imported modules.
    /// Only populated for bare module imports (`use crate::foo`), NOT item imports
    /// (`use crate::foo::Bar`) - this keeps Factorio globals like `settings` safe.
    pub bare_import_renames: HashMap<String, String>,
    /// Binding name -> Rust type key (last path segment, `Option`/`&` peeled) for
    /// compile-time `{:?}` Debug format selection.
    pub binding_types: HashMap<String, String>,
    /// Lint levels from `Factorio.toml` `[lints]` (defaults deny).
    pub lints: &'a LintConfig,
    /// Collected `warn`-level diagnostics (deny fails immediately).
    pub diagnostics: &'a mut Vec<Diagnostic>,
}

impl LowerContext<'_> {
    /// Emit a lint at `location`, or return `Ok(())` when the lint is allowed.
    pub fn emit_lint(
        &mut self,
        id: LintId,
        message: impl Into<String>,
        location: impl Into<String>,
    ) -> FrontendResult<()> {
        let level = self.lints.level(id);
        if matches!(level, LintLevel::Allow) {
            return Ok(());
        }
        let diagnostic = Diagnostic::new(id, level, message, location);
        if diagnostic.is_error() {
            return Err(FrontendError::Lint(diagnostic));
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    pub fn bind_type(&mut self, name: impl Into<String>, type_key: impl Into<String>) {
        self.binding_types.insert(name.into(), type_key.into());
    }

    #[must_use]
    pub fn binding_type(&self, name: &str) -> Option<&str> {
        self.binding_types.get(name).map(String::as_str)
    }

    /// Compute the Lua local name for a module path, with the configured prefix.
    pub fn prefixed_local(&self, module_path: &str) -> String {
        let base = require_local_name(module_path);
        if self.module_prefix.is_empty() {
            base
        } else {
            format!("{}_{}", self.module_prefix, base)
        }
    }

    /// If the first segment of `segments` matches a bare-imported module local,
    /// rewrite it to the prefixed name.
    pub fn normalize_bare_import_path(&self, segments: &mut [String]) {
        if self.bare_import_renames.is_empty() {
            return;
        }
        if let Some(first) = segments.first()
            && let Some(renamed) = self.bare_import_renames.get(first.as_str())
        {
            segments[0].clone_from(renamed);
        }
    }

    fn register_crate_module(&mut self, module: &str) {
        if self
            .imports
            .iter()
            .any(|fragment| fragment.module == module)
        {
            return;
        }

        self.imports.push(ImportFragment {
            module: module.to_string(),
            require_local: self.prefixed_local(module),
            item: None,
        });
    }

    pub fn normalize_crate_path(&mut self, segments: &mut Vec<String>) -> FrontendResult<()> {
        if segments.first().map(String::as_str) != Some("crate") {
            return Ok(());
        }

        segments.remove(0);
        if segments.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: "crate".to_string(),
            });
        }

        let (module_path, rest) = split_crate_path(segments);
        if module_path.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: segments.join("::"),
            });
        }

        self.register_crate_module(&module_path);

        let local = self.prefixed_local(&module_path);
        *segments = if rest.is_empty() {
            vec![local]
        } else {
            let mut rewritten = vec![local];
            rewritten.extend(rest);
            rewritten
        };

        Ok(())
    }
}
