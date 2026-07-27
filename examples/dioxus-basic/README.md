# Dioxus basic example

A small web app for trying Pixus `html!` compilation and Dioxus template hot reload.

The example is pinned to the Dioxus provider proof commit. Install the matching `dx` and the Pixus provider as described in the [root README](../../README.md), then run:

```sh
export DIOXUS_TEMPLATE_MACROS=html
export DIOXUS_TEMPLATE_PROVIDER="$(command -v pixus-template-provider)"
dx serve --hot-patch false
```

Edit `src/main.rs` while the app is running. A template-only `html!` edit should update through Dioxus without resetting the counter. An ordinary Rust edit should use Dioxus's rebuild path.
