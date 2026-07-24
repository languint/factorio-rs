use proc_macro::TokenStream;
use syn::{
    ItemFn, LitInt, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Implementation of `#[factorio_rs::bench]`.
///
/// # Arguments
///
/// `#[factorio_rs::bench]` - one iteration (default).
/// `#[factorio_rs::bench(iterations = N)]` - run the bench body N times; N must be >= 1.
///
/// # Expansion
///
/// ```ignore
/// #[factorio_rs::bench(iterations = 10)]
/// pub fn my_bench() {
///     // ...
/// }
/// ```
///
/// expands to:
///
/// ```ignore
/// #[allow(dead_code)]
/// pub fn my_bench() { /* ... */ }
///
/// #[doc(hidden)]
/// #[allow(non_upper_case_globals)]
/// const __factorio_rs_bench__my_bench: u32 = 10;
/// ```
///
/// The hidden const lets the frontend discover benches and their iteration
/// counts without running the proc macro again; the frontend also accepts the
/// bare `#[factorio_rs::bench]` attribute directly from source text.
pub fn bench(args: TokenStream, input: TokenStream) -> TokenStream {
    let bench_args = parse_macro_input!(args as BenchAttributeArgs);
    let function = parse_macro_input!(input as ItemFn);

    let iterations = bench_args.iterations;
    let bench_marker = syn::Ident::new(
        &format!("__factorio_rs_bench__{}", function.sig.ident),
        function.sig.ident.span(),
    );

    TokenStream::from(quote::quote! {
        #[allow(dead_code)]
        #function

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        const #bench_marker: u32 = #iterations;
    })
}

struct BenchAttributeArgs {
    iterations: u32,
}

impl Parse for BenchAttributeArgs {
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
