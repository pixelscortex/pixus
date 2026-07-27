# pixus-cli proxy proof

This binary currently contains a minimal proof that Pixus can proxy a normal Dioxus web devserver and inject devserver websocket messages.

## Run

Terminal 1:

```bash
cd examples/dioxus-basic
dx serve --web --port 8081 --open false
```

Terminal 2:

```bash
cargo run -p pixus-cli -- proxy \
  --upstream http://127.0.0.1:8081 \
  --listen 127.0.0.1:8090
```

Open the app through Pixus:

```text
http://127.0.0.1:8090
```

Then trigger proof injection:

```bash
curl http://127.0.0.1:8090/__pixus/reload
```

Expected result: the app reloads because Pixus injected Dioxus `DevserverMsg::FullReloadCommand` into the proxied `/_dioxus` websocket.

This does not implement `html!` template hot reload yet. It only proves the proxy/interception/injection transport path.
