use crate::{function::Function, module::Module, statement::Statement};

/// Find a function declaration in a module's body or exported symbols.
pub fn find_function<'a>(module: &'a Module, name: &str) -> Option<&'a Function> {
    module
        .body
        .statements
        .iter()
        .chain(module.symbols.iter().map(|symbol| &symbol.statement))
        .find_map(|statement| match statement {
            Statement::FunctionDecl(function) if function.name == name => Some(function),
            _ => None,
        })
}

/// Returns whether `name` is declared as a function in `module`.
pub fn function_exists(module: &Module, name: &str) -> bool {
    find_function(module, name).is_some()
}
