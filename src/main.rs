use std::sync::Arc;

use serde_json::Value;
use warp::http::HeaderMap;
use warp::path::FullPath;
use warp::Filter;

use crate::sonarr_handler::SonarrHandler;

mod send;
mod sonarr_handler;
mod structs;

#[tokio::main]
async fn main() {
    // SonarrHandler struct manages the state for the sonarr requests
    let sonarr_handler = Arc::new(SonarrHandler::new());

    // Warp web server route
    // accept POSTs to all paths, filters for Sonarr requests and passes them to the SonarrHandler
    let route = warp::post()
        .and(warp::header::headers_cloned())
        .and(warp::body::json::<Value>())
        .and(warp::path::full())
        .map({
            let sonarr_handler = Arc::clone(&sonarr_handler);

            move |headers: HeaderMap, body: Value, path: FullPath| {
                let sonarr_handler = Arc::clone(&sonarr_handler);
                let path = path.as_str().to_string();

                if let Some(user_agent) = headers.get("User-Agent") {
                    match user_agent.to_str() {
                        Ok(agent) if agent.to_lowercase().starts_with("sonarr") => {
                            // send the request to the sonarr handler async and move on
                            tokio::spawn(async move {
                                sonarr_handler.handle(path, body).await;
                            });

                            warp::reply::with_status(
                                warp::reply::json(
                                    &"Processing and fowarding Sonarr episode webhook",
                                ),
                                warp::http::StatusCode::OK,
                            )
                        }

                        _ => {
                            // not sonarr, bad request
                            println!("Received unsupported User-Agent");
                            warp::reply::with_status(
                                warp::reply::json(&"Received unsupported User-Agent"),
                                warp::http::StatusCode::BAD_REQUEST,
                            )
                        }
                    }
                } else {
                    // no user agent, bad request
                    warp::reply::with_status(
                        warp::reply::json(&"No User-Agent header"),
                        warp::http::StatusCode::BAD_REQUEST,
                    )
                }
            }
        });

    println!("Server started at localhost:8000");
    warp::serve(route).run(([0, 0, 0, 0], 8000)).await;
}
