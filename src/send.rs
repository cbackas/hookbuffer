use reqwest::{Client, StatusCode};
use std::time::Duration;
use worker::{console_error, console_log, console_warn, Delay};

use crate::structs::discord::DiscordWebhook;

pub async fn send_post_request(
    url: String,
    payload: impl Into<DiscordWebhook>,
) -> Result<StatusCode, StatusCode> {
    let payload = payload.into();

    console_log!(
        "Sending POST request to {} with payload: {:?}",
        url,
        payload
    );

    let client = Client::new();

    let mut backoff = Duration::from_secs(4); // start with a 4 second delay
    let backoff_limit = 128;

    loop {
        let response = client.post(&url).json(&payload).send().await;
        if let Err(e) = response {
            console_error!(
                "Failed to send POST request to {}. Error: {:?}, payload: {:?}",
                url,
                e,
                payload
            );
            let status: StatusCode = match e.status() {
                Some(status) => status,
                None => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return Err(status);
        }
        let response = response.unwrap();

        if response.status().is_success() {
            return Ok(StatusCode::OK);
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
            console_warn!(
                "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {:?}",
                backoff.as_secs(),
                url,
                response.status(),
                payload
            );

            Delay::from(backoff).await;

            if (backoff * 2).as_secs() > backoff_limit {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            } else {
                backoff *= 2;
            }
        } else {
            console_error!(
                "Failed to send POST request to {}. Status: {}, payload: {:?}",
                url,
                response.status(),
                payload
            );
            return Err(response.status());
        }
    }
}
