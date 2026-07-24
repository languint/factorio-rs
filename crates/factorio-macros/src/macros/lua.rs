use proc_macro::TokenStream;

/// Embed a verbatim Lua code block in a Factorio mod function.
///
/// The macro must be used inside an `unsafe fn` or an `unsafe { }` block.
/// The transpiler extracts the raw source between the delimiters and emits
/// it as-is into the generated Lua file, preserving all internal whitespace.
///
/// The Rust side of this macro expands to `()` so the call typechecks.
///
/// # Example
///
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// pub unsafe fn patch_globals() {
///     lua! {
///         local old_print = print
///         print = function(...)
///             old_print("[patched]", ...)
///         end
///     }
/// }
/// ```
pub fn lua(_input: TokenStream) -> TokenStream {
    // The actual code extraction happens in the frontend lowering phase.
    // At the Rust type-checking level we just return `()`.
    "()".parse().unwrap_or_default()
}
