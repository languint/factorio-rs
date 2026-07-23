use factorio_ir::expression::{Expression, MethodDispatch};

use crate::{
    LuaGenerator, attribute_property_for_setter,
    generator::expression::{
        reciever::{is_lua_stdlib_receiver, is_settings_receiver, is_storage_receiver},
        trim::trim_trailing_nils,
    },
    is_factorio_attribute_read, is_factorio_method,
};

fn method_call_sep(method: &str, receiver: &Expression, dispatch: MethodDispatch) -> &'static str {
    // Lua stdlib tables must use `.` otherwise the whole table would be passed to `self`.
    if is_lua_stdlib_receiver(receiver) {
        return ".";
    }
    match dispatch {
        MethodDispatch::Colon => ":",
        MethodDispatch::Factorio
        | MethodDispatch::StorageGet
        | MethodDispatch::StorageSet
        | MethodDispatch::SettingsGet => ".",
        MethodDispatch::Infer => {
            // Untyped / hand-built IR: Factorio API names use `.`, everything else `:`.
            if is_factorio_method(method) { "." } else { ":" }
        }
    }
}

impl LuaGenerator {
    /// `v.push(x)` -> `v[#v + 1] = x` (statement form; not a Lua expression).
    #[must_use]
    pub(crate) fn generate_push_assign_stmt(&self, expression: &Expression) -> Option<String> {
        let Expression::MethodCall {
            receiver,
            method,
            args,
            ..
        } = expression
        else {
            return None;
        };
        if method != "push" || args.len() != 1 {
            return None;
        }
        let receiver = self.generate_expression(receiver);
        let item = self.generate_expression(&args[0]);
        Some(format!("{receiver}[#{receiver} + 1] = {item}"))
    }

    #[must_use]
    pub fn generate_call(&self, func: &Expression, args: &[Expression]) -> String {
        if let Expression::QualifiedPath { segments } = func
            && args.is_empty()
            && segments
                .last()
                .is_some_and(|s| s == "new" || s == "default")
        {
            match segments[0].as_str() {
                "LuaAny" => return "nil".to_string(),
                "Vec" if segments.last().is_some_and(|s| s == "new") => {
                    return "{}".to_string();
                }
                _ if segments.last().is_some_and(|s| s == "default") => {
                    return "{}".to_string();
                }
                _ => {}
            }
        }

        let func_is_closure = matches!(func, Expression::Closure { .. });
        let func = self.generate_expression(func);
        let args = self.generate_arg_list(args);
        if func_is_closure {
            format!("({func})({args})")
        } else {
            format!("{func}({args})")
        }
    }

    #[must_use]
    pub fn generate_method_call(
        &self,
        receiver: &Expression,
        method: &str,
        args: &[Expression],
        dispatch: MethodDispatch,
    ) -> String {
        // Explicit / inferred storage + settings rewrites.
        let effective = match dispatch {
            MethodDispatch::Infer
                if method == "get" && args.len() == 1 && is_storage_receiver(receiver) =>
            {
                MethodDispatch::StorageGet
            }
            MethodDispatch::Infer
                if method == "set" && args.len() == 2 && is_storage_receiver(receiver) =>
            {
                MethodDispatch::StorageSet
            }
            MethodDispatch::Infer
                if matches!(
                    method,
                    "get" | "get_bool" | "get_int" | "get_double" | "get_string" | "setting"
                ) && args.len() == 1
                    && is_settings_receiver(receiver) =>
            {
                MethodDispatch::SettingsGet
            }
            other => other,
        };

        match effective {
            MethodDispatch::StorageGet if args.len() == 1 => {
                let receiver = self.generate_expression(receiver);
                let key = self.generate_expression(&args[0]);
                return format!("{receiver}[{key}]");
            }
            MethodDispatch::StorageSet if args.len() == 2 => {
                let receiver = self.generate_expression(receiver);
                let key = self.generate_expression(&args[0]);
                let value = self.generate_expression(&args[1]);
                return format!("{receiver}[{key}] = {value}");
            }
            MethodDispatch::SettingsGet if args.len() == 1 => {
                let receiver = self.generate_expression(receiver);
                let key = self.generate_expression(&args[0]);
                if method == "setting" {
                    return format!("{receiver}[{key}]");
                }
                return format!("{receiver}[{key}].value");
            }
            _ => {}
        }

        if method == "len" && args.is_empty() {
            let receiver = self.generate_expression(receiver);
            return format!("#{receiver}");
        }

        // Expression context: `table.insert` is a valid call expression.
        // Statement context uses `generate_push_assign_stmt` (`t[#t+1] = x`).
        if method == "push" && args.len() == 1 {
            let receiver = self.generate_expression(receiver);
            let item = self.generate_expression(&args[0]);
            return format!("table.insert({receiver}, {item})");
        }

        if method == "is_empty" && args.is_empty() {
            let receiver = self.generate_expression(receiver);
            return format!("#{receiver} == 0");
        }

        let trimmed = trim_trailing_nils(args);
        let allow_factorio_attr =
            matches!(effective, MethodDispatch::Factorio | MethodDispatch::Infer);

        if trimmed.is_empty() {
            let sep = method_call_sep(method, receiver, effective);
            let receiver_lua = self.generate_expression(receiver);
            // Zero-arg API *attributes* are property reads (`entity.surface`).
            // Never apply this for known user (`Colon`) receivers.
            if allow_factorio_attr && args.is_empty() && is_factorio_attribute_read(method) {
                return format!("{receiver_lua}.{method}");
            }
            return format!("{receiver_lua}{sep}{method}()");
        }

        // Attribute writers (`set_caption` / `write_driving`) -> property assign.
        if allow_factorio_attr
            && trimmed.len() == 1
            && let Some(property) = attribute_property_for_setter(method)
        {
            let receiver = self.generate_expression(receiver);
            let value = self.generate_expression(&trimmed[0]);
            return format!("{receiver}.{property} = {value}");
        }

        let sep = method_call_sep(method, receiver, effective);
        let receiver = self.generate_expression(receiver);
        let args_lua = self.generate_arg_list(trimmed);
        format!("{receiver}{sep}{method}({args_lua})")
    }

    /// Join call arguments, omitting trailing `nil` so Factorio optional params stay unset.
    #[must_use]
    pub fn generate_arg_list(&self, args: &[Expression]) -> String {
        trim_trailing_nils(args)
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
