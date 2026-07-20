use std::collections::{HashMap, HashSet};

use factorio_ir::lint::{Diagnostic, LintConfig, LintId, LintLevel};

use crate::{
    bindings::BindingRegistry,
    error::{FrontendError, FrontendResult},
    paths::{require_local_name, split_crate_path, split_crate_path_for_call},
};

use super::imports::ImportFragment;

pub struct LowerContext<'a> {
    pub imports: &'a mut Vec<ImportFragment>,
    /// Prefix prepended to every generated module local name to avoid shadowing
    /// Factorio built-in globals (e.g. `"ms"` -> `settings` becomes `ms_settings`).
    pub module_prefix: &'a str,
    /// Binding crates that map Rust `use` paths to foreign Factorio mod requires.
    pub bindings: &'a BindingRegistry,
    /// Maps bare module local names -> prefixed names for rewriting path expressions
    /// of the form `module_name::item` that reference bare-imported modules.
    /// Only populated for bare module imports (`use crate::foo`), NOT item imports
    /// (`use crate::foo::Bar`) - this keeps Factorio globals like `settings` safe.
    pub bare_import_renames: HashMap<String, String>,
    /// Local name for a remote stub module (`remote`) -> Factorio interface name.
    pub remote_locals: HashMap<String, String>,
    /// Bare-imported remote function locals (`use binding::greet`) -> `(interface, fn)`.
    pub remote_fn_locals: HashMap<String, (String, String)>,
    /// Binding name -> Rust type key (last path segment, `Option`/`&` peeled) for
    /// compile-time `{:?}` Debug format selection.
    pub binding_types: HashMap<String, String>,
    /// Locally declared enum variants, used to recognize constructors and patterns.
    pub enums: HashMap<String, Vec<EnumVariantInfo>>,
    /// `type` aliases in this module
    pub type_aliases: HashMap<String, super::types::TypeAlias>,
    /// Locals annotated as `Option<_>` (kept even though [`Self::binding_types`] peels Option).
    pub option_bindings: HashSet<String>,
    /// Lint levels from `Factorio.toml` `[lints]` (defaults deny).
    pub lints: &'a LintConfig,
    /// Collected warn/deny diagnostics (allow is skipped). Deny no longer aborts
    /// lowering so multiple findings can be reported together.
    pub diagnostics: &'a mut Vec<Diagnostic>,
    /// Statements hoisted by `?` (`local __try_N = ...; if __try_N.err ~= nil then return __try_N end`).
    /// Callers of [`lower_expression`](super::expressions::lower_expression) must
    /// drain these with [`Self::take_try_hoists_from`] immediately after.
    pub try_hoists: Vec<factorio_ir::statement::Statement>,
    /// Monotonic counter for `__try_N` temporaries.
    pub try_tmp_counter: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: EnumVariantFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumVariantFields {
    Unit,
    Tuple(usize),
    Named,
}

impl LowerContext<'_> {
    #[must_use]
    pub fn enum_variant(&self, enum_name: &str, variant: &str) -> Option<EnumVariantFields> {
        self.enums
            .get(enum_name)?
            .iter()
            .find(|info| info.name == variant)
            .map(|info| info.fields)
    }
}

impl LowerContext<'_> {
    /// Emit a lint at `loc` (no-op when the lint is allowed).
    ///
    /// Warn and deny both append to [`Self::diagnostics`]. Callers (and the CLI)
    /// decide whether deny findings fail the build after all files are processed.
    ///
    /// Returns `Ok` always so call sites can use `?` uniformly with other lowers.
    #[allow(clippy::unnecessary_wraps)]
    pub fn emit_lint(
        &mut self,
        id: LintId,
        message: impl Into<String>,
        loc: impl Into<factorio_ir::span::SourceLoc>,
    ) -> FrontendResult<()> {
        let level = self.lints.level(id);
        if matches!(level, LintLevel::Allow) {
            return Ok(());
        }
        self.diagnostics
            .push(Diagnostic::new(id, level, message, loc));
        Ok(())
    }

    pub fn bind_type(&mut self, name: impl Into<String>, type_key: impl Into<String>) {
        self.binding_types.insert(name.into(), type_key.into());
    }

    pub fn bind_option(&mut self, name: impl Into<String>) {
        self.option_bindings.insert(name.into());
    }

    #[must_use]
    pub fn binding_type(&self, name: &str) -> Option<&str> {
        self.binding_types.get(name).map(String::as_str)
    }

    /// Surface type for `?` / `if` / method discrimination (`Option` or `Result`).
    #[must_use]
    pub fn binding_surface_type(&self, name: &str) -> Option<&str> {
        if self.option_bindings.contains(name) {
            return Some("Option");
        }
        match self.binding_type(name) {
            Some("Result") => Some("Result"),
            _ => None,
        }
    }

    /// Snapshot length before lowering an expression that may emit `?` hoists.
    #[must_use]
    pub const fn try_hoist_mark(&self) -> usize {
        self.try_hoists.len()
    }

    /// Drain hoist statements pushed since `mark`.
    pub fn take_try_hoists_from(&mut self, mark: usize) -> Vec<factorio_ir::statement::Statement> {
        self.try_hoists.split_off(mark)
    }

    pub fn alloc_try_tmp(&mut self) -> String {
        self.try_tmp_counter += 1;
        format!("__try_{}", self.try_tmp_counter)
    }

    /// Allocate a temporary name for assertion left/right bindings.
    pub fn alloc_assert_tmp(&mut self, kind: &str) -> String {
        self.try_tmp_counter += 1;
        format!("__assert_{kind}_{}", self.try_tmp_counter)
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

    fn register_crate_module(
        &mut self,
        module: &str,
        factorio_mod: Option<String>,
        module_root: Option<String>,
    ) {
        if self.imports.iter().any(|fragment| {
            fragment.module == module
                && fragment.factorio_mod == factorio_mod
                && fragment.module_root == module_root
        }) {
            return;
        }

        self.imports.push(ImportFragment {
            module: module.to_string(),
            require_local: self.prefixed_local(module),
            item: None,
            factorio_mod,
            module_root,
        });
    }

    pub fn normalize_crate_path(&mut self, segments: &mut Vec<String>) -> FrontendResult<()> {
        self.normalize_crate_path_inner(segments, false)
    }

    /// Like [`Self::normalize_crate_path`] for call callees (last segment is the fn name).
    pub fn normalize_crate_path_for_call(
        &mut self,
        segments: &mut Vec<String>,
    ) -> FrontendResult<()> {
        self.normalize_crate_path_inner(segments, true)
    }

    /// If `segments` is a remote stub call (`provider_api::greet`,
    /// `provider_api::remote::greet`, `remote::greet` after `use ...::remote`, or
    /// a bare `greet` after `use ...::greet`), return `(interface, fn)`.
    pub fn resolve_remote_call(&self, segments: &[String]) -> Option<(String, String)> {
        if segments.len() == 1
            && let Some((interface, fn_name)) = self.remote_fn_locals.get(&segments[0])
        {
            return Some((interface.clone(), fn_name.clone()));
        }
        if segments.len() == 2
            && let Some(interface) = self.remote_locals.get(&segments[0])
        {
            return Some((interface.clone(), segments[1].clone()));
        }
        if segments.len() == 2
            && let Some(binding) = self.bindings.get(&segments[0])
            && let Some(interface) = binding.interface.as_ref()
            && binding.remote_fns.contains(&segments[1])
        {
            return Some((interface.clone(), segments[1].clone()));
        }
        if segments.len() >= 3
            && let Some(binding) = self.bindings.get(&segments[0])
            && let Some(interface) = binding.interface.as_ref()
            && segments[1] == "remote"
        {
            return Some((interface.clone(), segments[2].clone()));
        }
        None
    }

    fn normalize_crate_path_inner(
        &mut self,
        segments: &mut Vec<String>,
        for_call: bool,
    ) -> FrontendResult<()> {
        let Some(first) = segments.first().map(String::as_str) else {
            return Ok(());
        };

        // `remote::greet` after `use provider_api::remote`
        if for_call && segments.len() == 2 && self.remote_locals.contains_key(first) {
            return Ok(());
        }

        // `provider_api::greet` when `greet` is listed in `remote_fns`
        if for_call
            && segments.len() == 2
            && let Some(binding) = self.bindings.get(first)
            && binding.remote_fns.contains(&segments[1])
        {
            return Ok(());
        }

        let binding = if first == "crate" {
            None
        } else if let Some(binding) = self.bindings.get(first) {
            Some(binding.clone())
        } else {
            return Ok(());
        };

        let (factorio_mod, module_root) = binding.as_ref().map_or((None, None), |binding| {
            (
                Some(binding.mod_name.clone()),
                Some(binding.module_root.clone()),
            )
        });

        // `provider_api::remote::greet` - no require; track remote local if bare use.
        if let Some(binding) = &binding
            && binding.interface.is_some()
            && segments.get(1).map(String::as_str) == Some("remote")
        {
            if segments.len() == 2 {
                // bare `use provider_api::remote` handled in imports; path value unused
                *segments = vec!["remote".to_string()];
                return Ok(());
            }
            if for_call && segments.len() >= 3 {
                // Leave segments for resolve_remote_call (`crate, remote, fn`).
                return Ok(());
            }
        }

        segments.remove(0);
        if segments.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: factorio_ir::span::SourceLoc::default().with_note(
                    if factorio_mod.is_some() {
                        "binding crate"
                    } else {
                        "crate"
                    },
                ),
            });
        }

        let (module_path, rest) = if for_call {
            split_crate_path_for_call(segments)
        } else {
            split_crate_path(segments)
        };
        if module_path.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: factorio_ir::span::SourceLoc::default().with_note(segments.join("::")),
            });
        }

        self.register_crate_module(&module_path, factorio_mod, module_root);

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
