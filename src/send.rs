use std::time::Duration;

use reqwest::{Client, StatusCode};
use tokio::time::sleep;

use crate::structs::discord::DiscordWebhook;

pub async fn send_post_request(base_url: String, path: String, payload: DiscordWebhook) -> Result<(), StatusCode> {
    let base_url = base_url.strip_suffix('/').unwrap_or(&base_url);
    let full_url = format!("{}{}", base_url, path);

    let client = Client::new();

    let mut backoff = Duration::from_secs(4); // start with a 4 second delay
    let backoff_limit = 128;

    loop {
        let response = client.post(&full_url).json(&payload).send().await;
        if let Err(e) = response {
            println!("Failed to send POST request to {}. Error: {:?}, payload: {:?}", full_url, e, payload);
            let status: StatusCode = match e.status() {
                Some(status) => status,
                None => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return Err(status);
        }
        let response = response.unwrap();

        if response.status().is_success() {
            return Ok(());
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
            println!(
                "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {:?}",
                backoff.as_secs(),
                full_url,
                response.status(),
                payload
            );

            sleep(backoff).await;

            if (backoff * 2).as_secs() > backoff_limit {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            } else {
                backoff *= 2;
            }
        } else {
            println!(
                "Failed to send POST request to {}. Status: {}, payload: {:?}",
                full_url,
                response.status(),
                payload
            );
            return Err(response.status());
        }
    }
}
