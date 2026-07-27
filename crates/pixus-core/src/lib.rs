//! Shared HTML-like template parsing/codegen for Pixus.
//!
//! The parser lowers rstml nodes into a Dioxus RSX token stream. Both the
//! compile-time macro and the development-time provider use this module.

use std::collections::HashSet;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use rstml::{
    Parser, ParserConfig,
    node::{
        Infallible, KVAttributeValue, KeyedAttribute, KeyedAttributeValue, Node, NodeAttribute,
        NodeBlock, NodeElement, NodeName,
    },
};
use syn::{Expr, LitStr, Stmt, spanned::Spanned};

/// Parse an `html! { ... }` body and project it into Dioxus `rsx!` body tokens.
///
/// This is intentionally body-only so callers that own the surrounding template
/// invocation can reuse the deterministic projection without duplicating parsing
/// or lowering.
pub fn html_to_rsx_body(input: TokenStream) -> syn::Result<TokenStream> {
    let nodes = parse_html(input)?;
    nodes_to_rsx(&nodes)
}

/// Parse the HTML-like input and produce a Dioxus `rsx! { ... }` call.
pub fn html_to_rsx_call(input: TokenStream) -> syn::Result<TokenStream> {
    let body = html_to_rsx_body(input)?;

    Ok(quote! {
        ::dioxus::prelude::rsx! { #body }
    })
}

/// Parse the HTML-like input with rstml.
pub fn parse_html(input: TokenStream) -> syn::Result<Vec<Node>> {
    let empty_elements: HashSet<_> = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ]
    .into_iter()
    .collect();

    let raw_text_elements = ["script", "style"].into_iter().collect();

    let config = ParserConfig::new()
        .always_self_closed_elements(empty_elements)
        .raw_text_elements(raw_text_elements);

    Parser::new(config).parse_simple(input)
}

fn nodes_to_rsx(nodes: &[Node]) -> syn::Result<TokenStream> {
    let mut output = TokenStream::new();
    for node in nodes {
        output.extend(node_to_rsx(node)?);
    }
    Ok(output)
}

fn node_to_rsx(node: &Node) -> syn::Result<TokenStream> {
    match node {
        Node::Element(element) => element_to_rsx(element),
        Node::Fragment(fragment) => nodes_to_rsx(&fragment.children),
        Node::Text(text) => {
            let value = &text.value;
            Ok(quote! { #value })
        }
        Node::RawText(raw_text) => {
            // EDGE_CASE: preserve rstml's source span/origin if provider diagnostics
            // later require byte-accurate mapping; call-site span is safe for now.
            let value = LitStr::new(&raw_text.to_string_best(), Span::call_site());
            Ok(quote! { #value })
        }
        Node::Block(block) => {
            let block = block.to_token_stream();
            Ok(quote! { #block })
        }
        Node::Comment(_) => Ok(TokenStream::new()),
        Node::Doctype(doctype) => Err(syn::Error::new_spanned(
            doctype,
            "doctype nodes are not supported in pixus html! yet",
        )),
        Node::Custom(_) => unreachable!("rstml::Infallible custom nodes are never parsed"),
    }
}

fn element_to_rsx(element: &NodeElement<Infallible>) -> syn::Result<TokenStream> {
    let name = element_name_to_rsx(element.name())?;
    let attrs = attributes_to_rsx(element.attributes())?;
    let children = nodes_to_rsx(&element.children)?;

    Ok(quote! {
        #name {
            #attrs
            #children
        }
    })
}

fn attributes_to_rsx(attrs: &[NodeAttribute]) -> syn::Result<TokenStream> {
    let mut output = TokenStream::new();
    for attr in attrs {
        output.extend(attribute_to_rsx(attr)?);
    }
    Ok(output)
}

fn attribute_to_rsx(attr: &NodeAttribute) -> syn::Result<TokenStream> {
    match attr {
        NodeAttribute::Attribute(attr) => keyed_attribute_to_rsx(attr),
        NodeAttribute::Block(block) => spread_attribute_to_rsx(block),
    }
}

fn keyed_attribute_to_rsx(attr: &KeyedAttribute) -> syn::Result<TokenStream> {
    let name = attribute_name_to_rsx(&attr.key)?;

    match &attr.possible_value {
        KeyedAttributeValue::None => Ok(quote! { #name: true, }),
        KeyedAttributeValue::Value(value) => match &value.value {
            KVAttributeValue::Expr(expr) => {
                let value = attribute_expression_to_rsx(expr);
                Ok(quote! { #name: #value, })
            }
            KVAttributeValue::InvalidBraced(value) => Err(syn::Error::new_spanned(
                value,
                "invalid braced attribute value",
            )),
        },
        KeyedAttributeValue::Binding(binding) => Err(syn::Error::new_spanned(
            binding,
            "rstml binding attributes are not supported in pixus html! yet",
        )),
    }
}

fn attribute_expression_to_rsx(expr: &Expr) -> TokenStream {
    let Expr::Block(block) = expr else {
        return quote! { #expr };
    };
    if !block.attrs.is_empty() || block.label.is_some() {
        return quote! { #expr };
    }
    let [Stmt::Expr(inner, None)] = block.block.stmts.as_slice() else {
        return quote! { #expr };
    };

    quote! { #inner }
}

fn spread_attribute_to_rsx(block: &NodeBlock) -> syn::Result<TokenStream> {
    match block {
        NodeBlock::ValidBlock(_) => {
            let block = block.to_token_stream();
            Ok(quote! { ..#block, })
        }
        NodeBlock::Invalid(value) => Err(syn::Error::new_spanned(
            value,
            "invalid braced attribute spread",
        )),
    }
}

fn element_name_to_rsx(name: &NodeName) -> syn::Result<TokenStream> {
    match name {
        NodeName::Block(block) => Err(syn::Error::new_spanned(
            block,
            "dynamic element names are not supported in pixus html! yet",
        )),
        NodeName::Path(_) | NodeName::Punctuated(_) => Ok(name.to_token_stream()),
    }
}

fn attribute_name_to_rsx(name: &NodeName) -> syn::Result<TokenStream> {
    if let Some(ident) = single_segment_ident(name) {
        return Ok(quote! { #ident });
    }

    match name {
        NodeName::Block(block) => Err(syn::Error::new_spanned(
            block,
            "dynamic attribute names are not supported in pixus html! yet",
        )),
        NodeName::Path(_) | NodeName::Punctuated(_) => {
            let value = LitStr::new(&name.to_string(), name.span());
            Ok(quote! { #value })
        }
    }
}

fn single_segment_ident(name: &NodeName) -> Option<&Ident> {
    let NodeName::Path(path) = name else {
        return None;
    };

    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }

    Some(&path.path.segments.first()?.ident)
}

#[cfg(test)]
mod tests {
    use super::html_to_rsx_body;
    use proc_macro2::TokenStream;
    use quote::quote;

    fn project(input: TokenStream) -> String {
        html_to_rsx_body(input)
            .expect("valid html projection")
            .to_string()
    }

    #[test]
    fn projects_text_class_and_static_attributes() {
        let output = project(quote! { <div class="card" aria-label="hello">Hello</div> });
        assert_eq!(
            output,
            "div { class : \"card\" , \"aria-label\" : \"hello\" , \"Hello\" }"
        );
    }

    #[test]
    fn preserves_dynamic_blocks_and_event_handlers() {
        let output = project(quote! {
            <button onclick={handle_click}>{message}</button>
        });
        assert_eq!(output, "button { onclick : handle_click , { message } }");
    }

    #[test]
    fn preserves_multi_statement_attribute_blocks() {
        let output = project(quote! {
            <button onclick={let handler = handle_click; handler}>Run</button>
        });
        assert_eq!(
            output,
            "button { onclick : { let handler = handle_click ; handler } , \"Run\" }"
        );
    }

    #[test]
    fn preserves_spreads_and_nested_elements() {
        let output = project(quote! {
            <section {attrs}><span>child</span></section>
        });
        assert_eq!(output, "section { .. { attrs } , span { \"child\" } }");
    }

    #[test]
    fn conversion_is_deterministic() {
        let input = quote! { <div class="stable">{value}<span /></div> };
        assert_eq!(project(input.clone()), project(input));
    }

    #[test]
    fn rejects_unsupported_nodes_and_names() {
        assert!(html_to_rsx_body(quote! { <!doctype html><div /> }).is_err());
        assert!(html_to_rsx_body(quote! { <{tag} /> }).is_err());
        assert!(html_to_rsx_body(quote! { <div {name}="value" /> }).is_err());
        assert!(html_to_rsx_body(quote! { <input value(current) /> }).is_err());
    }
}
