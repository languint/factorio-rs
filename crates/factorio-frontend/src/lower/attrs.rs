use syn::{
    Attribute, Expr, LitInt, Meta, Path, Token,
    parse::{Parse, ParseStream},
};

use factorio_ir::stage::Stage;

pub struct EventAttributeArgs {
    pub event: Option<Path>,
    pub filter: Option<Expr>,
}

impl Parse for EventAttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                event: None,
                filter: None,
            });
        }

        if input.peek(syn::Ident) && input.peek2(Token![=]) {
            let keyword: syn::Ident = input.parse()?;
            if keyword != "filter" {
                return Err(syn::Error::new(
                    keyword.span(),
                    "expected `filter` or an event type such as `OnBuiltEntity`",
                ));
            }
            input.parse::<Token![=]>()?;
            let filter = Some(input.parse::<Expr>()?);
            return Ok(Self {
                event: None,
                filter,
            });
        }

        let event: Path = input.parse()?;
        let filter = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let keyword: syn::Ident = input.parse()?;
            if keyword != "filter" {
                return Err(syn::Error::new(
                    keyword.span(),
                    "expected `filter` after event type",
                ));
            }
            input.parse::<Token![=]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };

        Ok(Self {
            event: Some(event),
            filter,
        })
    }
}

/// Parses optional `#[factorio_rs::event(...)]` attribute arguments.
pub fn parse_factorio_event_attribute_args(attr: &Attribute) -> Option<EventAttributeArgs> {
    let path = attr.path();
    if !is_factorio_path_segment(path, "event") {
        return None;
    }

    match &attr.meta {
        Meta::Path(_) => Some(EventAttributeArgs {
            event: None,
            filter: None,
        }),
        Meta::List(meta_list) => syn::parse2::<EventAttributeArgs>(meta_list.tokens.clone()).ok(),
        Meta::NameValue(_) => None,
    }
}

/// Parses optional `#[factorio_rs::export]` / `#[factorio_rs::export(interface = "...")]`.
pub fn parse_factorio_export_attribute(
    attr: &Attribute,
) -> Option<factorio_ir::function::ExportMeta> {
    let path = attr.path();
    if !is_factorio_path_segment(path, "export") {
        return None;
    }

    match &attr.meta {
        Meta::Path(_) => Some(factorio_ir::function::ExportMeta { interface: None }),
        Meta::List(meta_list) => {
            let args = syn::parse2::<ExportAttributeArgs>(meta_list.tokens.clone()).ok()?;
            // Bare `interface` and omitted interface both mean "use mod name".
            Some(factorio_ir::function::ExportMeta {
                interface: args.interface,
            })
        }
        Meta::NameValue(_) => None,
    }
}

/// Parses `#[factorio_rs::inline]` (shared-stage require hot path; implies export).
pub fn parse_factorio_inline_attribute(attr: &Attribute) -> bool {
    is_factorio_path_segment(attr.path(), "inline")
}

/// Arguments parsed from `#[factorio_rs::bench]` or `#[factorio_rs::bench(iterations = N)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchAttributeArgs {
    /// Number of times the bench body should run per measurement (>= 1, default 1).
    pub iterations: u32,
}

/// Parses `#[factorio_rs::bench]` / `#[factorio_rs::bench(iterations = N)]`.
///
/// Returns `None` if `attr` is not a `factorio_rs::bench` attribute or if its
/// argument list cannot be parsed.
pub fn parse_factorio_bench_attribute(attr: &Attribute) -> Option<BenchAttributeArgs> {
    let path = attr.path();
    if !is_factorio_path_segment(path, "bench") {
        return None;
    }
    match &attr.meta {
        Meta::Path(_) => Some(BenchAttributeArgs { iterations: 1 }),
        Meta::List(meta_list) => syn::parse2::<BenchArgs>(meta_list.tokens.clone())
            .ok()
            .map(|a| BenchAttributeArgs {
                iterations: a.iterations,
            }),
        Meta::NameValue(_) => None,
    }
}

/// Returns `true` when `attrs` contain a `#[factorio_rs::bench]` attribute.
#[must_use]
pub fn is_bench_fn(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| parse_factorio_bench_attribute(attr).is_some())
}

struct BenchArgs {
    iterations: u32,
}

impl Parse for BenchArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { iterations: 1 });
        }
        let keyword: syn::Ident = input.parse()?;
        if keyword != "iterations" {
            return Err(syn::Error::new(
                keyword.span(),
                "expected `iterations = <n>` in #[factorio_rs::bench(...)]",
            ));
        }
        input.parse::<Token![=]>()?;
        let lit: LitInt = input.parse()?;
        let iterations: u32 = lit.base10_parse()?;
        if iterations == 0 {
            return Err(syn::Error::new(
                lit.span(),
                "`iterations` must be at least 1",
            ));
        }
        Ok(Self { iterations })
    }
}

struct ExportAttributeArgs {
    /// `None` for bare `#[export]` / `#[export(interface)]` (default at emit).
    /// `Some(name)` for `#[export(interface = "name")]`.
    interface: Option<String>,
}

impl Parse for ExportAttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { interface: None });
        }
        let keyword: syn::Ident = input.parse()?;
        if keyword != "interface" {
            return Err(syn::Error::new(
                keyword.span(),
                "expected `interface` or `interface = \"...\"`",
            ));
        }
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            let lit: syn::LitStr = input.parse()?;
            Ok(Self {
                interface: Some(lit.value()),
            })
        } else {
            Ok(Self { interface: None })
        }
    }
}

/// Parses `#[factorio_rs::control]`, `#[factorio_rs::shared]`, or `#[factorio_rs::data]`.
pub fn extract_factorio_stage(attrs: &[Attribute]) -> Option<Stage> {
    attrs.iter().find_map(parse_factorio_stage_attribute)
}

pub fn is_factorio_stage_bang(path: &Path) -> Option<Stage> {
    let mut segments = path.segments.iter();
    let first = segments.next()?;

    if first.ident == "factorio_rs" {
        let second = segments.next()?;
        if segments.next().is_some() {
            return None;
        }
        return stage_bang_ident_to_stage(&second.ident.to_string());
    }

    if segments.next().is_some() {
        return None;
    }

    stage_bang_ident_to_stage(&first.ident.to_string())
}

fn stage_bang_ident_to_stage(ident: &str) -> Option<Stage> {
    match ident {
        "settings_mod" => Some(Stage::Settings),
        "settings_updates_mod" => Some(Stage::SettingsUpdates),
        "settings_final_fixes_mod" => Some(Stage::SettingsFinalFixes),
        "data_mod" => Some(Stage::Data),
        "data_updates_mod" => Some(Stage::DataUpdates),
        "data_final_fixes_mod" => Some(Stage::DataFinalFixes),
        "control_mod" => Some(Stage::Control),
        "shared_mod" => Some(Stage::Shared),
        _ => None,
    }
}

fn parse_factorio_stage_attribute(attr: &Attribute) -> Option<Stage> {
    let path = attr.path();
    for stage_name in [
        "settings_final_fixes",
        "settings_updates",
        "settings",
        "data_final_fixes",
        "data_updates",
        "data",
        "control",
        "shared",
    ] {
        if is_factorio_path_segment(path, stage_name) {
            return stage_ident_to_stage(stage_name);
        }
    }
    None
}

fn is_factorio_path_segment(path: &Path, segment: &str) -> bool {
    let mut segments = path.segments.iter();
    let Some(first) = segments.next() else {
        return false;
    };
    if first.ident != "factorio_rs" {
        return false;
    }
    segments.next().is_some_and(|next| next.ident == segment)
}

fn stage_ident_to_stage(ident: &str) -> Option<Stage> {
    match ident {
        "settings" => Some(Stage::Settings),
        "settings_updates" => Some(Stage::SettingsUpdates),
        "settings_final_fixes" => Some(Stage::SettingsFinalFixes),
        "data" => Some(Stage::Data),
        "data_updates" => Some(Stage::DataUpdates),
        "data_final_fixes" => Some(Stage::DataFinalFixes),
        "control" => Some(Stage::Control),
        "shared" => Some(Stage::Shared),
        _ => None,
    }
}

/// Returns true when `attrs` include `#[cfg(test)]` (possibly among other cfgs).
#[must_use]
pub fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(attr_is_cfg_test)
}

/// Returns true when `attrs` include a bare `#[test]` attribute.
#[must_use]
pub fn is_test_fn(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        path.is_ident("test")
            || (path.segments.len() == 2
                && path.segments[0].ident == "rust"
                && path.segments[1].ident == "test")
    })
}

fn attr_is_cfg_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let Meta::List(meta_list) = &attr.meta else {
        return false;
    };
    // `#[cfg(test)]` or `#[cfg(all(test, ...))]` / `#[cfg(any(test, ...))]`
    let tokens = meta_list.tokens.to_string();
    tokens
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|part| part == "test")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use syn::parse_str;

    use super::extract_factorio_stage;

    #[test]
    fn parses_factorio_stage_attribute() {
        let Ok(item_mod) = parse_str::<syn::ItemMod>(
            r"
            #[factorio_rs::control]
            mod handlers {}
        ",
        ) else {
            assert_eq!(1, 0, "failed to parse mod");
            return;
        };

        assert_eq!(
            extract_factorio_stage(&item_mod.attrs),
            Some(factorio_ir::stage::Stage::Control)
        );
    }
}
