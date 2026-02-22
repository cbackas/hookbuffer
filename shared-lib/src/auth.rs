use axum::http::{header::HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use base64::{engine::general_purpose, Engine as _};

#[derive(Debug)]
pub struct AuthError {
    pub status: StatusCode,
    pub message: &'static str,
}

impl AuthError {
    pub fn unauthorized(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }

    pub fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.message)).into_response()
    }
}

pub fn check_auth(
    user_value: String,
    pass_value: String,
    headers: &HeaderMap,
) -> Result<(), AuthError> {
    let auth_header = headers
        .get("Authorization")
        .ok_or(AuthError::unauthorized("No Authorization header"))?;
    let auth_str = auth_header
        .to_str()
        .map_err(|_| AuthError::bad_request("Invalid Authorization header"))?;
    let auth = auth_str
        .strip_prefix("Basic ")
        .ok_or(AuthError::bad_request("Invalid Authorization header"))?;
    let decoded = general_purpose::STANDARD.decode(auth).map_err(|_| {
        AuthError::bad_request("Invalid Authorization header: couldn't decode base64")
    })?;
    let auth = String::from_utf8(decoded).map_err(|_| {
        AuthError::bad_request(
            "Invalid Authorization header: couldn't convert decoded utf8 to string",
        )
    })?;

    let mut auth_parts = auth.splitn(2, ':');
    let (user, pass) = (
        auth_parts.next().unwrap_or("admin"),
        auth_parts.next().unwrap_or(""),
    );

    if user != user_value || pass != pass_value {
        return Err(AuthError::unauthorized(
            "Invalid Authorization header: incorrect username or password",
        ));
    }

    Ok(())
}
