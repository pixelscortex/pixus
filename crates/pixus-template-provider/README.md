# pixus-template-provider

A small development-time adapter for Dioxus external template providers.

Dioxus invokes:

```text
pixus-template-provider project-body html
```

The provider reads an `html!` body token stream from stdin and writes Dioxus RSX body tokens to stdout. It does not run a watcher, proxy, devserver, build, or platform launcher.

Install it from a Pixus checkout:

```sh
cargo install --path crates/pixus-template-provider
```

Configure a Dioxus DX build containing the external-template-provider seam:

```sh
export DIOXUS_TEMPLATE_MACROS=html
export DIOXUS_TEMPLATE_PROVIDER="$(command -v pixus-template-provider)"
dx serve --hot-patch false
```

The compile-time `html!` macro and this provider both use `pixus-core::html_to_rsx_body`, keeping their projection semantics aligned.
