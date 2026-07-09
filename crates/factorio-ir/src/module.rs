use crate::{block::Block, scope::Scope, stage::Stage, statement::Statement};

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub scope: Scope,
    pub statement: Statement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImport {
    pub module: String,
    pub local: String,
    pub items: Vec<ImportedItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedItem {
    pub name: String,
    pub local: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub stage: Stage,
    pub body: Block,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<ModuleImport>,
    pub submodules: Vec<String>,
}

impl Module {
    #[must_use]
    pub fn imported_item_local(&self, type_name: &str) -> Option<&str> {
        self.imports
            .iter()
            .flat_map(|import| import.items.iter())
            .find_map(|item| {
                if item.name == type_name {
                    Some(item.local.as_str())
                } else {
                    None
                }
            })
    }

    #[must_use]
    pub fn is_imported_type_extension(&self, struct_decl: &crate::structure::Struct) -> bool {
        struct_decl.fields.is_empty()
            && self.imported_item_local(&struct_decl.name).is_some()
    }
}
