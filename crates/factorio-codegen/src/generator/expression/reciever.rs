use factorio_ir::expression::Expression;

pub fn is_storage_receiver(receiver: &Expression) -> bool {
    match receiver {
        Expression::Identifier(name) => name == "storage",
        Expression::QualifiedPath { segments } => {
            segments.last().is_some_and(|name| name == "storage")
        }
        _ => false,
    }
}

pub fn is_settings_receiver(receiver: &Expression) -> bool {
    match receiver {
        Expression::Identifier(name) => name == "settings",
        Expression::FieldAccess { base, .. } => is_settings_receiver(base),
        Expression::QualifiedPath { segments } => {
            segments.first().is_some_and(|name| name == "settings")
        }
        _ => false,
    }
}

pub fn is_lua_stdlib_receiver(receiver: &Expression) -> bool {
    matches!(
        receiver,
        Expression::Identifier(name)
            if matches!(
                name.as_str(),
                "string"
                    | "table"
                    | "math"
                    | "bit32"
                    | "coroutine"
                    | "os"
                    | "debug"
                    | "package"
                    | "utf8"
            )
    )
}
