# Pixus rstml `html!` POC

This is a normal Dioxus app used to try the Pixus inline macro in a real `dx serve` workflow.

Run it with:

```bash
cd examples/dioxus-basic
dx serve --web
```

What this POC proves:

- `pixus_macro::html!` accepts HTML-like syntax.
- The macro uses `rstml` for parsing.
- The macro emits Dioxus `rsx!`, so the app renders through normal Dioxus.

What this POC does **not** prove yet:

- Stock `dx serve` template hot reload for `html!` edits. `dx` still scans raw source for literal `rsx!`/`render!` calls, so edits inside `html!` are expected to rebuild/hotpatch until we add the Pixus wrapper/watcher.
