use std::collections::HashMap;
use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use warp::http::HeaderMap;
use warp::path::FullPath;
use warp::reply::{Json, WithStatus};
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

    // health check route
    let health_check = warp::path!("healthcheck").map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));

    // accept POSTs to all paths, filters for Sonarr requests and passes them to the SonarrHandler
    let catch_all = warp::post()
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
                if let (Ok(user_value), Ok(pass_value)) = (std::env::var("HOOKBUFFER_USER"), std::env::var("HOOKBUFFER_PASS")) {
                    if let Some(reply) = check_auth(user_value, pass_value, headers.clone()) {
                        return reply;
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

    let routes = health_check.or(catch_all);

    let server_port = env::get_server_port();
    println!("Server started at localhost:{}", server_port);
    warp::serve(routes).run(([0, 0, 0, 0], server_port)).await;
}

fn check_auth(user_value: String, pass_value: String, headers: HeaderMap) -> Option<WithStatus<Json>> {
    let auth_header = match headers.get("Authorization") {
        Some(auth) => auth,
        None => {
            return Some(warp::reply::with_status(
                warp::reply::json(&"No Authorization header"),
                warp::http::StatusCode::UNAUTHORIZED,
            ))
        }
    };

    let auth_str = match auth_header.to_str() {
        Ok(auth) => auth,
        Err(_) => {
            return Some(warp::reply::with_status(
                warp::reply::json(&"Invalid Authorization header"),
                warp::http::StatusCode::BAD_REQUEST,
            ))
        }
    };

    let auth = match auth_str.strip_prefix("Basic ") {
        Some(auth) => auth,
        None => {
            return Some(warp::reply::with_status(
                warp::reply::json(&"Invalid Authorization header"),
                warp::http::StatusCode::BAD_REQUEST,
            ))
        }
    };

    let decoded = match general_purpose::STANDARD.decode(auth) {
        Ok(decoded) => decoded,
        Err(_) => {
            return Some(warp::reply::with_status(
                warp::reply::json(&"Invalid Authorization header: couldn't decode base64"),
                warp::http::StatusCode::BAD_REQUEST,
            ))
        }
    };

    let auth = match String::from_utf8(decoded) {
        Ok(auth) => auth,
        Err(_) => {
            return Some(warp::reply::with_status(
                warp::reply::json(&"Invalid Authorization header: couldn't convert decoded utf8 to string"),
                warp::http::StatusCode::BAD_REQUEST,
            ))
        }
    };

    let mut auth_parts = auth.splitn(2, ':');
    let (user, pass) = (auth_parts.next().unwrap(), auth_parts.next().unwrap_or(""));

    if user != user_value || pass != pass_value {
        return Some(warp::reply::with_status(
            warp::reply::json(&"Invalid Authorization header: incorrect username or password"),
            warp::http::StatusCode::UNAUTHORIZED,
        ));
    }

    None
}
