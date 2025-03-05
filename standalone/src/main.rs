use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use tower_http::compression::{
    predicate::{DefaultPredicate, NotForContentType, Predicate},
    CompressionLayer,
};
use tower_http::trace::TraceLayer;

use tracing::{level_filters, Span};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::sonarr_handler::SonarrHandler;

mod env;
mod sonarr_handler;

type SharedAppState = Arc<SonarrHandler>;

#[derive(Debug, Clone)]
struct RequestUri(Uri);

#[tokio::main]
async fn main() {
    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(level_filters::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let predicate = DefaultPredicate::new().and(NotForContentType::new("application/json"));
    let compression_layer = CompressionLayer::new().gzip(true).compress_when(predicate);

    // health check route
    let app = Router::new()
        .layer(axum::middleware::from_fn(
            |request: Request<_>, next: Next| async move {
                let uri = request.uri().clone();

                let mut response = next.run(request).await;

                response.extensions_mut().insert(RequestUri(uri));

                response
            },
        ))
        .layer(TraceLayer::new_for_http().on_response(
            |response: &Response, latency: std::time::Duration, _span: &Span| {
                let url = match response.extensions().get::<RequestUri>().map(|r| &r.0) {
                    Some(uri) => uri.to_string(),
                    None => "unknown".to_string(),
                };
                let status = response.status();
                let latency = {
                    let milliseconds = latency.as_secs_f64() * 1000.0
                        + latency.subsec_nanos() as f64 / 1_000_000.0;
                    // Format the milliseconds to a string with 2 decimal places and add 'ms' postfix
                    format!("{:.2}ms", milliseconds)
                };

                if url == "/healthcheck" {
                    tracing::trace!("{} {} {}", url, status, latency);
                    return;
                }

                tracing::debug!("{} {} {}", url, status, latency);
            },
        ))
        .layer(compression_layer)
        .route("/healthcheck", get(health_check))
        .route("/{*path}", post(handle_post))
        .with_state(SharedAppState::default());

    let server_port = env::get_server_port();
    tracing::info!("Server started at localhost:{}", server_port);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", server_port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_post(
    State(sonarr_handler): State<SharedAppState>,
    Path(path): Path<String>, // Must come before other extractors
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Basic auth check
    if let (Ok(user), Ok(pass)) = (
        std::env::var("HOOKBUFFER_USER"),
        std::env::var("HOOKBUFFER_PASS"),
    ) {
        if let Err(response) = shared_lib::auth::check_auth(user, pass, &headers) {
            return response;
        }
    }

    // User-Agent verification
    match headers.get("User-Agent").and_then(|h| h.to_str().ok()) {
        Some(agent) if agent.starts_with("Sonarr") => {
            let handler = Arc::clone(&sonarr_handler);
            tokio::spawn(async move {
                handler.handle(path.clone(), body).await;
            });

            (
                StatusCode::OK,
                Json("Processing and forwarding Sonarr episode webhook"),
            )
                .into_response()
        }
        _ => {
            tracing::warn!("Received unsupported User-Agent");
            (
                StatusCode::BAD_REQUEST,
                Json("Received unsupported User-Agent"),
            )
                .into_response()
        }
    }
}

async fn health_check() -> &'static str {
    "OK"
}
