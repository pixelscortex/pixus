//! HTML-like template syntax for Dioxus.
//!
//! `html!` delegates parsing and lowering to `pixus-core`, which uses `rstml`
//! and emits a normal Dioxus `rsx!` invocation.

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
