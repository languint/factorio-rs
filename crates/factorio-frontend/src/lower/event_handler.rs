use syn::{ItemFn, Type};

use factorio_ir::expression::Expression;

use super::{attrs::parse_factorio_event_attribute_args, event_filter::lower_event_filter_list};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEventHandler {
    pub event_name: String,
    pub filter: Option<Expression>,
}

/// Resolves whether a function is a Factorio event handler by reading its
/// `#[factorio_rs::event]` attribute.
pub fn resolve_event_handler(function: &ItemFn) -> Option<ParsedEventHandler> {
    let args = function
        .attrs
        .iter()
        .find_map(parse_factorio_event_attribute_args)?;

    let marker_type = args
        .event
        .as_ref()
        .and_then(|path| path.segments.last().map(|s| s.ident.to_string()))
        .or_else(|| event_marker_from_param(function))?;

    let event_name = factorio_api::event_type_to_name(&marker_type)?.to_string();
    let filter = match &args.filter {
        Some(expression) => Some(lower_event_filter_list(expression).ok()?),
        None => None,
    };

    Some(ParsedEventHandler { event_name, filter })
}

pub fn event_marker_from_type(ty: &Type) -> Option<String> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segments = &type_path.path.segments;
    match segments.len() {
        1 => {
            let ident = segments[0].ident.to_string();
            ident.strip_suffix("Event").map(str::to_string)
        }
        _ => None,
    }
}

pub fn event_marker_from_param(function: &ItemFn) -> Option<String> {
    let syn::FnArg::Typed(pat_type) = function.sig.inputs.first()? else {
        return None;
    };
    event_marker_from_type(&pat_type.ty)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use syn::parse_str;

    use super::resolve_event_handler;

    #[test]
    fn resolves_handler_from_attribute_without_filter() {
        let function: syn::ItemFn = parse_str(
            r"
            #[factorio_rs::event]
            pub fn on_singleplayer_init(_event: OnSingleplayerInitEvent) {}
            ",
        )
        .expect("function");

        let handler = resolve_event_handler(&function).expect("handler");
        assert_eq!(handler.event_name, "on_singleplayer_init");
        assert!(handler.filter.is_none());
    }

    #[test]
    fn resolves_handler_from_attribute_with_filter() {
        let function: syn::ItemFn = parse_str(
            r#"
            #[factorio_rs::event(filter = [OnBuiltEntityFilter::type_("inserter")])]
            pub fn on_built_entity(_event: OnBuiltEntityEvent) {}
            "#,
        )
        .expect("function");

        let handler = resolve_event_handler(&function).expect("handler");
        assert_eq!(handler.event_name, "on_built_entity");
        assert!(handler.filter.is_some());
    }

    #[test]
    fn no_attribute_returns_none() {
        let function: syn::ItemFn =
            parse_str(r"pub fn helper(x: u32) -> u32 { x }").expect("function");

        assert!(resolve_event_handler(&function).is_none());
    }
}
