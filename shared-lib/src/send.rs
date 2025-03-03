use axum::http::StatusCode;
use std::time::Duration;

use crate::structs::discord::DiscordWebhookBody;

pub async fn send_post_request(
    url: String,
    body: DiscordWebhookBody,
) -> Result<StatusCode, StatusCode> {
    #[cfg(feature = "worker")]
    worker::console_log!("Sending POST request to {} with payload: {:?}", url, body);
    #[cfg(feature = "standalone")]
    tracing::info!("Sending POST request to {} with payload: {:?}", url, body);

    let mut backoff = Duration::from_secs(4); // start with a 4 second delay
    let backoff_limit = 128;

    loop {
        match ureq::post(url.clone()).send_json(&body) {
            Err(e) => {
                #[cfg(feature = "worker")]
                worker::console_error!(
                    "Failed to send POST request to {}. Error: {:?}, payload: {:?}",
                    url,
                    e,
                    body
                );
                #[cfg(feature = "standalone")]
                tracing::error!(
                    "Failed to send POST request to {}. Error: {:?}, payload: {:?}",
                    url,
                    e,
                    body
                );

                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(response) if response.status().is_success() => {
                return Ok(StatusCode::OK);
            }
            Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                #[cfg(feature = "worker")]
                worker::console_warn!(
                    "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {:?}",
                    backoff.as_secs(),
                    url,
                    response.status(),
                    body
                );
                #[cfg(feature = "standalone")]
                tracing::warn!(
                    "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {:?}",
                    backoff.as_secs(),
                    url,
                    response.status(),
                    body
                );

                tokio::time::sleep(backoff).await;

                if (backoff * 2).as_secs() > backoff_limit {
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                } else {
                    backoff *= 2;
                }
            }
            Ok(response) => {
                #[cfg(feature = "worker")]
                worker::console_error!(
                    "Failed to send POST request to {}. Status: {}, payload: {:?}",
                    url,
                    response.status(),
                    body
                );
                #[cfg(feature = "standalone")]
                tracing::error!(
                    "Failed to send POST request to {}. Status: {}, payload: {:?}",
                    url,
                    response.status(),
                    body
                );
                return Err(response.status());
            }
        }
    }
}
