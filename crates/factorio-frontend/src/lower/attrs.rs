use syn::{Attribute, Meta, Path};

use factorio_ir::stage::Stage;

/// Parses `#[factorio_rs::event(OnInit)]` and returns the Factorio event name (`on_init`).
pub fn extract_factorio_event(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .find_map(parse_factorio_event_attribute)
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
        "control_mod" => Some(Stage::Control),
        "shared_mod" => Some(Stage::Shared),
        "data_mod" => Some(Stage::Data),
        _ => None,
    }
}

fn parse_factorio_event_attribute(attr: &Attribute) -> Option<String> {
    let path = attr.path();
    if !is_factorio_path_segment(path, "event") {
        return None;
    }

    let Meta::List(meta_list) = &attr.meta else {
        return None;
    };

    let path = syn::parse2::<Path>(meta_list.tokens.clone()).ok()?;
    let segment = path.segments.last()?;
    event_type_to_name(&segment.ident.to_string())
}

fn parse_factorio_stage_attribute(attr: &Attribute) -> Option<Stage> {
    let path = attr.path();
    for stage_name in ["control", "shared", "data"] {
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
        "control" => Some(Stage::Control),
        "shared" => Some(Stage::Shared),
        "data" => Some(Stage::Data),
        _ => None,
    }
}

fn event_type_to_name(event_type: &str) -> Option<String> {
    match event_type {
        "OnInit" => Some("on_init".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use syn::parse_str;

    use super::{extract_factorio_event, extract_factorio_stage};

    #[test]
    fn parses_factorio_event_attribute() {
        let Ok(function) = parse_str::<syn::ItemFn>(
            r"
            #[factorio_rs::event(OnInit)]
            pub fn on_init() {}
        ",
        ) else {
            assert_eq!(1, 0, "failed to parse function");
            return;
        };

        assert_eq!(
            extract_factorio_event(&function.attrs).as_deref(),
            Some("on_init")
        );
    }

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
