use proc_macro::TokenStream;
use syn::{ItemFn, Path, parse_macro_input, spanned::Spanned};

/// Marks a control-stage function as a Factorio event handler.
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
    let event_path = parse_macro_input!(args as Path);
    let function = parse_macro_input!(input as ItemFn);

    let event_name = match event_path.segments.last() {
        Some(segment) => match segment.ident.to_string().as_str() {
            "OnInit" => "on_init",
            other => {
                return syn::Error::new_spanned(
                    &segment.ident,
                    format!("unsupported event type `{other}`"),
                )
                .to_compile_error()
                .into();
            }
        },
        None => {
            return syn::Error::new(event_path.span(), "expected an event type such as `OnInit`")
                .to_compile_error()
                .into();
        }
    };

    if function.sig.ident != event_name {
        return syn::Error::new_spanned(
            &function.sig.ident,
            format!("event handler must be named `{event_name}`"),
        )
        .to_compile_error()
        .into();
    }

    // Event handlers are invoked from generated Lua, not Rust call sites.
    TokenStream::from(quote::quote! {
        #[allow(dead_code)]
        #function
    })
}

/// Marks a file or inline `mod` as control-stage code for transpilation.
#[proc_macro_attribute]
pub fn control(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Marks a file or inline `mod` as shared-stage code for transpilation.
#[proc_macro_attribute]
pub fn shared(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Marks a file or inline `mod` as data-stage code for transpilation.
#[proc_macro_attribute]
pub fn data(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Declares a control-stage module from a block of items.
#[proc_macro]
pub fn control_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("control", input)
}

/// Declares a shared-stage module from a block of items.
#[proc_macro]
pub fn shared_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("shared", input)
}

/// Declares a data-stage module from a block of items.
#[proc_macro]
pub fn data_mod(input: TokenStream) -> TokenStream {
    wrap_stage_module("data", input)
}

fn wrap_stage_module(stage: &str, input: TokenStream) -> TokenStream {
    let module_name = syn::Ident::new(
        &format!("__factorio_{stage}"),
        proc_macro2::Span::call_site(),
    );
    let items = proc_macro2::TokenStream::from(input);

    TokenStream::from(quote::quote! {
        #[doc(hidden)]
        mod #module_name {
            #items
        }
    })
}
