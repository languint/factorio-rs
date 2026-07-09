use std::collections::HashMap;

use crate::module::Module;

/// Identifies a single transpiled item within a module for reachability tracking.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemKey {
    /// A top-level or nested function by name.
    Function(String),
    /// A struct table by name.
    Struct(String),
    /// An instance method on a struct: `(struct_name, method_name)`.
    StructMethod(String, String),
    /// An associated constant on a struct: `(struct_name, constant_name)`.
    StructConstant(String, String),
}

/// Cross-module index built from lowered modules before pruning.
pub struct ModuleGraph<'a> {
    pub modules: &'a [Module],
    by_name: HashMap<&'a str, &'a Module>,
    children: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> ModuleGraph<'a> {
    /// Build a lookup graph from all modules in a build.
    pub fn new(modules: &'a [Module]) -> Self {
        let by_name = modules
            .iter()
            .map(|module| (module.name.as_str(), module))
            .collect::<HashMap<_, _>>();
        let mut children = HashMap::<&str, Vec<&str>>::new();
        for module in modules {
            for child in &module.submodules {
                if let Some(child_module) = by_name.get(child.as_str()) {
                    children
                        .entry(module.name.as_str())
                        .or_default()
                        .push(child_module.name.as_str());
                }
            }
        }
        Self {
            modules,
            by_name,
            children,
        }
    }

    /// Look up a module by its dotted name (`control`, `shared.player`, etc.).
    pub fn get(&self, name: &str) -> Option<&'a Module> {
        self.by_name.get(name).copied()
    }

    /// Return the direct submodule names declared by `name`.
    pub fn child_modules(&self, name: &str) -> &[&'a str] {
        self.children.get(name).map_or(&[], Vec::as_slice)
    }
}
