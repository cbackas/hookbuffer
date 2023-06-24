use std::sync::Arc;

use handlers::sonarr::SonarrHandler;
use serde_json::Value;
use warp::http::HeaderMap;
use warp::path::FullPath;
use warp::Filter;

use crate::structs::hookbuffer::HBQuery;

mod handlers;
mod structs;

#[tokio::main]
async fn main() {
    // SonarrHandler struct manages the state for the sonarr requests
    let sonarr_handler = Arc::new(SonarrHandler::new());

    let route = warp::post()
        .and(warp::header::headers_cloned())
        .and(warp::body::json::<Value>())
        .and(warp::path::full())
        .and(warp::query::<HBQuery>())
        .map({
            let sonarr_handler = Arc::clone(&sonarr_handler);

            move |headers: HeaderMap, body: Value, path: FullPath, query: HBQuery| {
                let sonarr_handler = Arc::clone(&sonarr_handler);
                let path = path.as_str().to_string();

                if let Some(user_agent) = headers.get("User-Agent") {
                    match user_agent.to_str() {
                        Ok(agent) if agent.to_lowercase().starts_with("sonarr") => {
                            println!("Processing Sonarr data");
                            // Call the handle method on the SonarrHandler.
                            let body = body.clone();
                            tokio::spawn(async move {
                                sonarr_handler.handle(path, body).await;
                            });
                            warp::reply::with_status(
                                "Processing Sonarr data",
                                warp::http::StatusCode::OK,
                            )
                        }

                        Ok(agent) if agent.to_lowercase().starts_with("unraid") => {
                            println!("Processing Unraid data");
                            warp::reply::with_status(
                                "Processing Unraid data",
                                warp::http::StatusCode::OK,
                            )
                        }

                        _ => {
                            // Unsupported content type
                            println!("Received unsupported User-Agent");
                            warp::reply::with_status(
                                "Received unsupported User-Agent",
                                warp::http::StatusCode::BAD_REQUEST,
                            )
                        }
                    }
                } else {
                    // If there is no User-Agent header, return a 400 Bad Request.
                    warp::reply::with_status(
                        "No User-Agent header",
                        warp::http::StatusCode::BAD_REQUEST,
                    )
                }
            }
        });

    println!("Server started at localhost:8000");
    warp::serve(route).run(([0, 0, 0, 0], 8000)).await;
}
