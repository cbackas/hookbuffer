use axum::{
    body::Body,
    http::{header::HeaderMap, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose, Engine as _};

pub fn check_auth(
    user_value: String,
    pass_value: String,
    headers: &HeaderMap,
) -> Result<(), Response<Body>> {
    let unauthorized = |msg| (StatusCode::UNAUTHORIZED, Json(msg)).into_response();
    let bad_request = |msg| (StatusCode::BAD_REQUEST, Json(msg)).into_response();

    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| unauthorized("No Authorization header"))?;
    let auth_str = auth_header
        .to_str()
        .map_err(|_| bad_request("Invalid Authorization header"))?;
    let auth = auth_str
        .strip_prefix("Basic ")
        .ok_or_else(|| bad_request("Invalid Authorization header"))?;
    let decoded = general_purpose::STANDARD
        .decode(auth)
        .map_err(|_| bad_request("Invalid Authorization header: couldn't decode base64"))?;
    let auth = String::from_utf8(decoded).map_err(|_| {
        bad_request("Invalid Authorization header: couldn't convert dec/* o */ded utf8 to string")
    })?;

    let mut auth_parts = auth.splitn(2, ':');
    let (user, pass) = (
        auth_parts.next().unwrap_or("admin"),
        auth_parts.next().unwrap_or(""),
    );

    if user != user_value || pass != pass_value {
        return Err(unauthorized(
            "Invalid Authorization header: incorrect username or password",
        ));
    }

    Ok(())
}
