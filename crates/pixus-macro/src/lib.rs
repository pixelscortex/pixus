//! Procedural macros for Pixus.
//!
//! `html!` is intentionally tiny in this POC: it delegates all parsing and
//! conversion to `pixus-core`, which uses `rstml` and then emits Dioxus RSX.

use proc_macro::TokenStream;

/// Parse HTML-like syntax with `rstml` and expand it to Dioxus `rsx!`.
///
/// ```ignore
/// html! {
///     <div class="container">"Hello"</div>
/// }
/// ```
#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    match pixus_core::html_to_rsx_call(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
