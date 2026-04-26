//! `ometa mcp` — local MCP server proxying to {host}/mcp.
//!
//! Forwards every request body verbatim to the upstream MCP endpoint with the
//! configured Bearer token injected, and streams the response back. This lets
//! local AI clients (Claude Desktop, Cursor, etc.) talk to OpenMetadata's MCP
//! server without each having to know the JWT.

use crate::config::{resolve, Overrides};
use anyhow::Result;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use clap::Parser;
use colored::Colorize;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser, Debug)]
pub struct Args {
    /// Local port to bind on.
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

#[derive(Clone)]
struct Proxy {
    upstream: String,
    token: String,
    http: reqwest::Client,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    let cfg = resolve(overrides)?;
    let proxy = Arc::new(Proxy {
        upstream: format!("{}/mcp", cfg.host.trim_end_matches('/')),
        token: cfg.token,
        http: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/mcp", post(mcp_post).get(mcp_get))
        .route("/healthz", get(health))
        .with_state(proxy.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    println!(
        "{} MetaTree MCP proxy listening on http://{}/mcp",
        "▶".green().bold(),
        addr
    );
    println!("  Upstream: {}", proxy.upstream);
    println!(
        "  {}\n    {}",
        "Add this to your MCP client config:".dimmed(),
        format!("{{ \"url\": \"http://{addr}/mcp\" }}").bold()
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn mcp_get(State(proxy): State<Arc<Proxy>>) -> impl IntoResponse {
    forward(proxy, reqwest::Method::GET, HeaderMap::new(), Bytes::new()).await
}

async fn mcp_post(
    State(proxy): State<Arc<Proxy>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    forward(proxy, reqwest::Method::POST, headers, body).await
}

async fn forward(
    proxy: Arc<Proxy>,
    method: reqwest::Method,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = proxy
        .http
        .request(method, &proxy.upstream)
        .bearer_auth(&proxy.token)
        .body(body.to_vec());
    if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) {
        req = req.header("content-type", ct);
    }
    if let Some(accept) = headers.get(axum::http::header::ACCEPT) {
        req = req.header("accept", accept);
    }

    match req.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::BAD_GATEWAY);
            let mut out_headers = HeaderMap::new();
            if let Some(ct) = resp.headers().get("content-type").cloned() {
                if let Ok(v) = ct.to_str() {
                    if let Ok(hv) = axum::http::HeaderValue::from_str(v) {
                        out_headers.insert(axum::http::header::CONTENT_TYPE, hv);
                    }
                }
            }
            let bytes = resp.bytes().await.unwrap_or_default().to_vec();
            (status, out_headers, bytes)
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            HeaderMap::new(),
            format!(r#"{{"error":"upstream_unreachable","detail":"{e}"}}"#).into_bytes(),
        ),
    }
}
