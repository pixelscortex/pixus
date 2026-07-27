use std::{
    io::{self, Read},
    net::SocketAddr,
};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{
        OriginalUri, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderName, Method, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::{any, get},
};
use clap::{Args, Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use proc_macro2::TokenStream;
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tracing::{debug, error, info, warn};
use url::Url;

const FULL_RELOAD_COMMAND: &str = r#""FullReloadCommand""#;

#[derive(Debug, Parser)]
#[command(name = "pixus")]
#[command(about = "Pixus development tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Proof proxy for stock Dioxus `dx serve`.
    Proxy(ProxyArgs),
    /// Experimental body projection provider for Dioxus.
    #[command(hide = true)]
    ProjectBody(ProjectBodyArgs),
}

#[derive(Debug, Args)]
struct ProjectBodyArgs {
    /// Source macro name.
    macro_name: String,
}

#[derive(Debug, Args)]
struct ProxyArgs {
    /// Public address Pixus listens on.
    #[arg(long, default_value = "127.0.0.1:8090")]
    listen: SocketAddr,

    /// Upstream stock dx devserver URL.
    #[arg(long, default_value = "http://127.0.0.1:8081")]
    upstream: Url,
}

fn project_body(macro_name: &str, source: &str) -> Result<String> {
    if macro_name != "html" {
        bail!("unsupported macro name `{macro_name}` (only `html` is supported)");
    }
    let input: TokenStream = source
        .parse()
        .map_err(|error| anyhow::anyhow!("failed to parse macro body as token stream: {error}"))?;
    pixus_core::html_to_rsx_body(input)
        .context("failed to project html body")
        .map(|tokens| tokens.to_string())
}

fn run_project_body(args: ProjectBodyArgs) -> Result<()> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("failed to read macro body from stdin")?;
    println!("{}", project_body(&args.macro_name, &source)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pixus_cli=info,pixus=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Proxy(args) => run_proxy(args).await,
        Command::ProjectBody(args) => run_project_body(args),
    }
}

#[derive(Clone)]
struct ProxyState {
    upstream: Url,
    client: reqwest::Client,
    inject_tx: broadcast::Sender<String>,
}

async fn run_proxy(args: ProxyArgs) -> Result<()> {
    let (inject_tx, _) = broadcast::channel(32);
    let state = ProxyState {
        upstream: args.upstream,
        client: reqwest::Client::new(),
        inject_tx,
    };

    let app = Router::new()
        .route("/__pixus/reload", get(inject_reload))
        .route("/_dioxus", get(proxy_ws))
        .route("/", any(proxy_http))
        .route("/{*path}", any(proxy_http))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("failed to bind {}", args.listen))?;

    info!(listen = %args.listen, upstream = %state.upstream, "pixus proxy listening");
    info!("open the app through the pixus URL, then hit /__pixus/reload to prove injection");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn inject_reload(State(state): State<ProxyState>) -> impl IntoResponse {
    let clients = state.inject_tx.receiver_count();
    match state.inject_tx.send(FULL_RELOAD_COMMAND.to_string()) {
        Ok(_) => (
            StatusCode::OK,
            format!("sent Dioxus FullReloadCommand to {clients} websocket client(s)\n"),
        ),
        Err(_) => (
            StatusCode::OK,
            "no websocket clients are currently connected\n".to_string(),
        ),
    }
}

async fn proxy_ws(
    State(state): State<ProxyState>,
    OriginalUri(uri): OriginalUri,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(err) = handle_ws_proxy(state, uri, socket).await {
            error!(?err, "websocket proxy failed");
        }
    })
}

async fn handle_ws_proxy(state: ProxyState, uri: Uri, socket: WebSocket) -> Result<()> {
    let upstream_url = upstream_ws_url(&state.upstream, &uri)?;
    info!(%upstream_url, "proxying /_dioxus websocket");

    let (upstream, _) = connect_async(upstream_url.as_str())
        .await
        .with_context(|| format!("failed to connect upstream websocket {upstream_url}"))?;

    let (mut client_write, mut client_read) = socket.split();
    let (mut upstream_write, mut upstream_read) = upstream.split();
    let mut inject_rx = state.inject_tx.subscribe();

    let client_to_upstream = async move {
        while let Some(message) = client_read.next().await {
            let message = message.context("failed reading browser websocket message")?;
            if let Some(message) = axum_to_tungstenite(message) {
                if let TungsteniteMessage::Text(text) = &message {
                    debug!(direction = "browser->dx", %text);
                }
                upstream_write
                    .send(message)
                    .await
                    .context("failed forwarding browser websocket message upstream")?;
            }
        }
        Result::<()>::Ok(())
    };

    let upstream_to_client = async move {
        loop {
            tokio::select! {
                message = upstream_read.next() => {
                    let Some(message) = message else {
                        break;
                    };
                    let message = message.context("failed reading upstream websocket message")?;
                    if let TungsteniteMessage::Text(text) = &message {
                        debug!(direction = "dx->browser", %text);
                    }
                    if let Some(message) = tungstenite_to_axum(message) {
                        client_write
                            .send(message)
                            .await
                            .context("failed forwarding upstream websocket message to browser")?;
                    }
                }
                injected = inject_rx.recv() => {
                    match injected {
                        Ok(message) => {
                            info!(%message, "injecting devserver websocket message");
                            client_write
                                .send(Message::Text(message.into()))
                                .await
                                .context("failed injecting websocket message")?;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(skipped, "websocket injection receiver lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
        Result::<()>::Ok(())
    };

    tokio::try_join!(client_to_upstream, upstream_to_client)?;
    Ok(())
}

async fn proxy_http(
    State(state): State<ProxyState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    match proxy_http_inner(state, method, uri, headers, body).await {
        Ok(response) => response,
        Err(err) => {
            error!(?err, "http proxy failed");
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("pixus proxy error: {err}\n")))
                .expect("response builder should be valid")
        }
    }
}

async fn proxy_http_inner(
    state: ProxyState,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response<Body>> {
    let upstream_url = upstream_http_url(&state.upstream, &uri)?;
    debug!(%method, %upstream_url, "proxying http request");

    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())?;
    let mut request = state.client.request(reqwest_method, upstream_url);

    for (name, value) in headers.iter() {
        if should_forward_header(name) {
            request = request.header(name.as_str(), value.as_bytes());
        }
    }

    let response = request
        .body(body)
        .send()
        .await
        .context("upstream http request failed")?;

    let status = StatusCode::from_u16(response.status().as_u16())?;
    let mut builder = Response::builder().status(status);

    for (name, value) in response.headers().iter() {
        if should_forward_header(name) {
            builder = builder.header(name.as_str(), value.as_bytes());
        }
    }

    let bytes = response
        .bytes()
        .await
        .context("failed reading upstream response body")?;

    Ok(builder
        .body(Body::from(bytes))
        .expect("response builder should be valid"))
}

fn upstream_http_url(upstream: &Url, uri: &Uri) -> Result<String> {
    if upstream.scheme() != "http" && upstream.scheme() != "https" {
        bail!("upstream must use http or https, got {}", upstream.scheme());
    }

    Ok(join_base_and_uri(upstream, uri))
}

fn upstream_ws_url(upstream: &Url, uri: &Uri) -> Result<String> {
    let ws_scheme = match upstream.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => bail!("upstream must use http or https, got {other}"),
    };

    let mut ws_base = upstream.clone();
    ws_base
        .set_scheme(ws_scheme)
        .map_err(|_| anyhow::anyhow!("failed to set websocket scheme"))?;

    Ok(join_base_and_uri(&ws_base, uri))
}

fn join_base_and_uri(base: &Url, uri: &Uri) -> String {
    let base = base.as_str().trim_end_matches('/');
    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    format!("{base}{path_and_query}")
}

fn should_forward_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "host"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn axum_to_tungstenite(message: Message) -> Option<TungsteniteMessage> {
    match message {
        Message::Text(text) => Some(TungsteniteMessage::Text(text.to_string().into())),
        Message::Binary(bytes) => Some(TungsteniteMessage::Binary(bytes)),
        Message::Ping(bytes) => Some(TungsteniteMessage::Ping(bytes)),
        Message::Pong(bytes) => Some(TungsteniteMessage::Pong(bytes)),
        Message::Close(_) => Some(TungsteniteMessage::Close(None)),
    }
}

fn tungstenite_to_axum(message: TungsteniteMessage) -> Option<Message> {
    match message {
        TungsteniteMessage::Text(text) => Some(Message::Text(text.to_string().into())),
        TungsteniteMessage::Binary(bytes) => Some(Message::Binary(bytes)),
        TungsteniteMessage::Ping(bytes) => Some(Message::Ping(bytes)),
        TungsteniteMessage::Pong(bytes) => Some(Message::Pong(bytes)),
        TungsteniteMessage::Close(_) => Some(Message::Close(None)),
        TungsteniteMessage::Frame(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::project_body;

    #[test]
    fn projects_supported_html_body() {
        let projected = project_body("html", "<div class=\"greeting\">Hello</div>").unwrap();
        assert_eq!(projected, "div { class : \"greeting\" , \"Hello\" }");
    }

    #[test]
    fn rejects_unsupported_macro_name() {
        let error = project_body("rsx", "<div />").unwrap_err();
        assert!(error.to_string().contains("only `html` is supported"));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(project_body("html", "<div>").is_err());
    }
}
