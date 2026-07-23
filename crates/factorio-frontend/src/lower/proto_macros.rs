//! Shared helpers for data-stage prototype macro expansion.

use crate::error::{FrontendError, FrontendResult};

/// Rewrite a relative icon path to `__{mod}__/...`, or pass through absolute `__...` paths.
pub fn resolve_icon_path(icon: &str, mod_name: Option<&str>) -> FrontendResult<String> {
    if icon.starts_with("__") {
        return Ok(icon.to_string());
    }
    let Some(mod_name) = mod_name else {
        return Err(FrontendError::ItemIconNeedsModName {
            path: icon.to_string(),
        });
    };
    let trimmed = icon.strip_prefix("./").unwrap_or(icon);
    Ok(format!("__{mod_name}__/{trimmed}"))
}

/// Resolve an optional icon, rewriting when present.
pub fn resolve_optional_icon(
    icon: Option<&str>,
    mod_name: Option<&str>,
) -> FrontendResult<Option<String>> {
    match icon {
        Some(path) => Ok(Some(resolve_icon_path(path, mod_name)?)),
        None => Ok(None),
    }
}

pub fn option_i64_src(value: Option<i64>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

pub fn option_f64_src(value: Option<f64>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

pub fn option_bool_src(value: Option<bool>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some({v})"))
}

pub fn option_str_src(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_string(), |v| format!("Some(\"{v}\")"))
}

pub fn option_flags_src(flags: Option<&[String]>) -> String {
    flags.map_or_else(
        || "None".to_string(),
        |flags| {
            let inner = flags
                .iter()
                .map(|f| format!("\"{f}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Some(&[{inner}])")
        },
    )
}

pub fn str_list_src(items: &[String]) -> String {
    let inner = items
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{inner}]")
}

pub fn energy_source_src(energy_type: &str, usage_priority: Option<&str>) -> String {
    let usage_priority = option_str_src(usage_priority);
    format!(
        "EnergySource {{ r#type: \"{energy_type}\", usage_priority: {usage_priority}, ..Default::default() }}"
    )
}

pub fn color_src(r: f64, g: f64, b: f64, a: Option<f64>) -> String {
    let a = option_f64_src(a);
    format!("Color {{ r: {r}, g: {g}, b: {b}, a: {a}, ..Default::default() }}")
}

/// Emit `Names` struct + constants + `register_*` that calls `data.extend`.
pub fn emit_names_module(
    names_struct: &str,
    register_fn: &str,
    const_defs: &str,
    extend_items: &str,
) -> FrontendResult<Vec<syn::Item>> {
    let code = format!(
        "pub struct {names_struct}; \
         impl {names_struct} {{ {const_defs} }} \
         pub fn {register_fn}() {{ data.extend([ {extend_items} ]); }}"
    );
    let file: syn::File = syn::parse_str(&code).map_err(FrontendError::from)?;
    Ok(file.items)
}
