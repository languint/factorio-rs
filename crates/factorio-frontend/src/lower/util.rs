use syn::ImplItem;
use syn::spanned::Spanned;

use factorio_ir::span::{SourceLoc, SourceSpan};

/// Returns a byte-accurate source location for error reporting.
pub fn location(span: &impl Spanned) -> SourceLoc {
    SourceLoc::new(SourceSpan::from(span.span().byte_range()))
}

/// Returns a short description of a top-level item for error reporting.
pub fn item_name(item: &syn::Item) -> String {
    match item {
        syn::Item::Fn(function) => format!("fn {}", function.sig.ident),
        syn::Item::Mod(module) => format!("mod {}", module.ident),
        syn::Item::Struct(item) => format!("struct {}", item.ident),
        syn::Item::Enum(item) => format!("enum {}", item.ident),
        syn::Item::Const(item) => format!("const {}", item.ident),
        syn::Item::Static(item) => format!("static {}", item.ident),
        syn::Item::Use(_) => "use".to_string(),
        syn::Item::Type(item) => format!("type {}", item.ident),
        syn::Item::Trait(item) => format!("trait {}", item.ident),
        syn::Item::Macro(_) => "macro".to_string(),
        _ => "item".to_string(),
    }
}

pub fn item_name_impl(item: &ImplItem) -> String {
    match item {
        ImplItem::Fn(function) => format!("fn {}", function.sig.ident),
        ImplItem::Const(item) => format!("const {}", item.ident),
        ImplItem::Type(item) => format!("type {}", item.ident),
        ImplItem::Macro(_) => "macro".to_string(),
        _ => "impl item".to_string(),
    }
}
