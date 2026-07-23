use factorio_ir::{module::Module, stage::Stage, statement::Statement};

/// Control-stage event handler registration for `control.lua`.
#[derive(Debug, Clone, PartialEq)]
pub struct EventRegistration {
    pub module: String,
    pub handler: String,
    pub event: String,
    pub filter: Option<factorio_ir::expression::Expression>,
}

/// Collect control-stage event handlers.
#[must_use]
pub fn collect_event_registrations(module: &Module) -> Vec<EventRegistration> {
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
            let event_name = function.event.as_ref()?;
            Some(EventRegistration {
                module: module.name.clone(),
                handler: function.name.clone(),
                event: event_name.clone(),
                filter: function.event_filter.clone(),
            })
        })
        .collect()
}
