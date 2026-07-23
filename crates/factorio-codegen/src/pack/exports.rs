use factorio_ir::{module::Module, stage::Stage, statement::Statement};

/// A control-stage function published via `#[factorio_rs::export]` for `remote.call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExport {
    pub module: String,
    pub function: String,
    pub interface: String,
    pub params: Vec<(String, Option<String>)>,
}

/// Collect control-stage `#[factorio_rs::export]` functions (skips event handlers).
#[must_use]
pub fn collect_remote_exports(module: &Module, default_interface: &str) -> Vec<RemoteExport> {
    if module.stage != Stage::Control {
        return Vec::new();
    }

    module
        .symbols
        .iter()
        .filter_map(|symbol| {
            let Statement::FunctionDecl(function) = &symbol.statement else {
                return None;
            };
            if function.event.is_some() {
                return None;
            }
            let export = function.export.as_ref()?;
            if function.inline {
                return None;
            }
            Some(RemoteExport {
                module: module.name.clone(),
                function: function.name.clone(),
                interface: export
                    .interface
                    .clone()
                    .unwrap_or_else(|| default_interface.to_string()),
                params: function
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.source_type.clone()))
                    .collect(),
            })
        })
        .collect()
}
