use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use tracing::level_filters;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::sonarr_handler::SonarrHandler;

mod env;
mod sonarr_handler;

type SharedAppState = Arc<SonarrHandler>;

#[tokio::main]
async fn main() {
    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(level_filters::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // health check route
    let app = Router::new()
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
    Query(query): Query<HashMap<String, String>>,
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
                handler.handle(path.clone(), body, query).await;
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
