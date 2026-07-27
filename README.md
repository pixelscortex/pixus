# Pixus

Pixus is an experimental HTML-like `html!` macro for Dioxus.

```rust
use dioxus::prelude::*;
use pixus_macro::html;

#[component]
fn App() -> Element {
    let mut count = use_signal(|| 0);

    html! {
        <main class="container">
            <h1>"Hello from Pixus"</h1>
            <p>"Count: {count}"</p>
            <button onclick={move |_| count += 1}>"Increment"</button>
        </main>
    }
}
```

Pixus lowers the HTML-like body into normal Dioxus RSX. Dioxus remains the renderer and owns development-time source watching, template identity, diffing, caching, transport, replay, and rebuild fallback.

## Status

Pixus is a pre-release proof for testing and experimentation.

Compile-time rendering works through the normal procedural macro. Template-only hot reload requires a Dioxus DX build containing the external-template-provider seam. Until that feature is released upstream, this repository pins its example to the exact Dioxus proof commit:

```text
30f01fe9185127de806f5d82dcb9d63498efa3af
```

Do not combine the example with the older stock Dioxus 0.7.6 CLI: that release does not contain the provider seam.

## Try the current proof

### 1. Install the matching DX build

```sh
git clone https://github.com/JustKira/dioxus.git
cd dioxus
git checkout 30f01fe9185127de806f5d82dcb9d63498efa3af
cargo install --path packages/cli --locked --force
```

### 2. Install the Pixus provider

From this repository:

```sh
cargo install --path crates/pixus-template-provider --force
```

### 3. Run the example

```sh
export DIOXUS_TEMPLATE_MACROS=html
export DIOXUS_TEMPLATE_PROVIDER="$(command -v pixus-template-provider)"

cd examples/dioxus-basic
dx serve --hot-patch false
```

Edit the body of `html!` in `examples/dioxus-basic/src/main.rs`. Supported template-only changes use Dioxus's normal template hot-reload path. Ordinary Rust changes and unsupported projections conservatively use Dioxus's rebuild path.

## Use the macro in another project

Before crates are published, depend on the repository directly:

```toml
[dependencies]
dioxus = { git = "https://github.com/JustKira/dioxus", rev = "30f01fe9185127de806f5d82dcb9d63498efa3af" }
pixus-macro = { git = "https://github.com/pixelscortex/pixus" }
```

Then import the macro:

```rust
use pixus_macro::html;
```

When Dioxus releases the provider seam, use the matching released Dioxus library and DX CLI instead of the temporary fork revision.

## Packages

```text
pixus-core
  Shared rstml parsing and deterministic HTML-to-RSX lowering.

pixus-macro
  Compile-time html! procedural macro.

pixus-template-provider
  Tiny development-time body projector invoked by DX.
```

The provider is not a second CLI control plane. It has one contract:

```text
stdin:  html! body tokens
stdout: Dioxus RSX body tokens
```

It does not contain a watcher, proxy, devserver, cache, build system, or platform launcher.

## Supported syntax

The current proof supports:

- static element names;
- text and nested elements;
- static and expression-valued attributes;
- Dioxus event handlers such as `onclick={...}`;
- Rust expression children;
- spread attributes;
- standard HTML void elements.

It rejects unsupported constructs rather than producing a partial projection. Current rejected cases include doctypes, dynamic tag names, dynamic attribute names, and binding attributes.

## Architecture

```text
compile time
  html! body -> pixus-core -> Dioxus rsx! -> normal Dioxus application

development time
  DX collector -> pixus-template-provider -> Dioxus CallBody
               -> existing Dioxus diff/cache/transport/fallback
```

The compile-time macro and development-time provider share `pixus-core::html_to_rsx_body`, preventing two independent lowering implementations from drifting.

## More detail

- [`docs/pixus-template-provider.md`](docs/pixus-template-provider.md) — implementation and correctness report.
- [`docs/dioxus-external-template-provider-pr-draft.md`](docs/dioxus-external-template-provider-pr-draft.md) — generic Dioxus pull-request draft.
