use syn::{
    Attribute, Expr, Meta, Path, Token,
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
