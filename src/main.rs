use std::collections::HashMap;
use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use warp::http::HeaderMap;
use warp::path::FullPath;
use warp::Filter;

use crate::sonarr_handler::SonarrHandler;

mod env;
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
        .and(warp::query::<HashMap<String, String>>())
        .map({
            let sonarr_handler = Arc::clone(&sonarr_handler);

            move |headers: HeaderMap, body: Value, path: FullPath, map: HashMap<String, String>| {
                let sonarr_handler = Arc::clone(&sonarr_handler);
                let path = path.as_str().to_string();

                // if the HOOKBUFFER_USER and HOOKBUFFER_PASS env vars are set, check for basic auth
                let user = std::env::var("HOOKBUFFER_USER");
                let pass = std::env::var("HOOKBUFFER_PASS");
                if user.is_ok() && pass.is_ok() {
                    let user_value = user.unwrap();
                    let pass_value = pass.unwrap();

                    if let Some(auth_header) = headers.get("Authorization") {
                        match auth_header.to_str() {
                            Ok(auth) => {
                                let auth = auth.replace("Basic ", "");
                                let auth = general_purpose::STANDARD.decode(auth);
                                if auth.is_err() {
                                    // bad request
                                    return warp::reply::with_status(
                                        warp::reply::json(&"Invalid Authorization header: couldn't decode base64"),
                                        warp::http::StatusCode::BAD_REQUEST,
                                    );
                                }

                                let auth = auth.unwrap();
                                let auth = String::from_utf8(auth);
                                if auth.is_err() {
                                    // bad request
                                    return warp::reply::with_status(
                                        warp::reply::json(&"Invalid Authorization header: couldn't convert decoded utf8 to string"),
                                        warp::http::StatusCode::BAD_REQUEST,
                                    );
                                }

                                let auth = auth.unwrap();
                                let auth = auth.split(':').collect::<Vec<&str>>();
                                if auth.len() != 2 {
                                    // bad request
                                    return warp::reply::with_status(
                                        warp::reply::json(&"Invalid Authorization header: couldn't split into 2 parts"),
                                        warp::http::StatusCode::BAD_REQUEST,
                                    );
                                }

                                if auth[0] != user_value || auth[1] != pass_value {
                                    // unauthorized
                                    return warp::reply::with_status(
                                        warp::reply::json(&"Invalid Authorization header: incorrect username or password"),
                                        warp::http::StatusCode::UNAUTHORIZED,
                                    );
                                }
                            }

                            Err(_) => {
                                // bad request
                                return warp::reply::with_status(
                                    warp::reply::json(&"Invalid Authorization header"),
                                    warp::http::StatusCode::BAD_REQUEST,
                                );
                            }
                        }
                    } else {
                        // unauthorized
                        return warp::reply::with_status(warp::reply::json(&"No Authorization header"), warp::http::StatusCode::UNAUTHORIZED);
                    }
                }

                if let Some(user_agent) = headers.get("User-Agent") {
                    match user_agent.to_str() {
                        Ok(agent) if agent.to_lowercase().starts_with("sonarr") => {
                            // send the request to the sonarr handler async and move on
                            tokio::spawn(async move {
                                sonarr_handler.handle(path, body, map).await;
                            });

                            warp::reply::with_status(
                                warp::reply::json(&"Processing and fowarding Sonarr episode webhook"),
                                warp::http::StatusCode::OK,
                            )
                        }

                        _ => {
                            // not sonarr, bad request
                            println!("Received unsupported User-Agent");
                            warp::reply::with_status(warp::reply::json(&"Received unsupported User-Agent"), warp::http::StatusCode::BAD_REQUEST)
                        }
                    }
                } else {
                    // no user agent, bad request
                    warp::reply::with_status(warp::reply::json(&"No User-Agent header"), warp::http::StatusCode::BAD_REQUEST)
                }
            }
        });

    let server_port = env::get_server_port();
    println!("Server started at localhost:{}", server_port);
    warp::serve(route).run(([0, 0, 0, 0], server_port)).await;
}
