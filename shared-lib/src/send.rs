use axum::http::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::structs::discord::DiscordWebhookBody;
use crate::structs::pushover::{PushoverConfig, PushoverMessage};
use crate::structs::summary::GroupSummary;

const PUSHOVER_ENDPOINT: &str = "https://api.pushover.net/1/messages.json";

/// The downstream service that grouped webhooks are forwarded to. Chosen once
/// per instance from the environment; the Pushover variant carries its config.
#[derive(Clone)]
pub enum Target {
    Discord,
    Pushover(PushoverConfig),
}

impl Target {
    /// Build the concrete, ready-to-send message for a group summary.
    /// `discord_url` is the fully-resolved Discord destination and is ignored
    /// for the Pushover target (which posts to a fixed API endpoint).
    pub fn build(&self, summary: &GroupSummary, discord_url: &str) -> Outbound {
        match self {
            Target::Discord => Outbound::Discord {
                url: discord_url.to_string(),
                body: summary.into(),
            },
            Target::Pushover(config) => {
                Outbound::Pushover(PushoverMessage::render(summary, config))
            }
        }
    }
}

/// A message ready to send. This is the concrete, serializable value that
/// crosses the Cloudflare queue in the worker (Pushover credentials live in the
/// rendered message, so they ride along in the queue payload).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Outbound {
    Discord {
        url: String,
        body: DiscordWebhookBody,
    },
    Pushover(PushoverMessage),
}

pub async fn send_post_request(message: &Outbound) -> Result<StatusCode, StatusCode> {
    let url = match message {
        Outbound::Discord { url, .. } => url.as_str(),
        Outbound::Pushover(_) => PUSHOVER_ENDPOINT,
    };
    // Redacted for Pushover via its manual Debug impl.
    let payload = format!("{message:?}");

    #[cfg(feature = "worker")]
    worker::console_log!("Sending POST request to {} with payload: {}", url, payload);
    #[cfg(feature = "standalone")]
    tracing::info!("Sending POST request to {} with payload: {}", url, payload);

    let mut backoff = Duration::from_secs(4); // start with a 4 second delay
    let backoff_limit = 128;

    let client = Client::new();

    loop {
        let request = client.post(url);
        let request = match message {
            Outbound::Discord { body, .. } => request.json(body),
            Outbound::Pushover(msg) => request.form(msg),
        };

        match request.send().await {
            Err(e) => {
                #[cfg(feature = "worker")]
                worker::console_error!(
                    "Failed to send POST request to {}. Error: {:?}, payload: {}",
                    url,
                    e,
                    payload
                );
                #[cfg(feature = "standalone")]
                tracing::error!(
                    "Failed to send POST request to {}. Error: {:?}, payload: {}",
                    url,
                    e,
                    payload
                );

                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(response) if response.status().is_success() => {
                return Ok(StatusCode::OK);
            }
            Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                #[cfg(feature = "worker")]
                worker::console_warn!(
                    "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {}",
                    backoff.as_secs(),
                    url,
                    response.status(),
                    payload
                );
                #[cfg(feature = "standalone")]
                tracing::warn!(
                    "Rate limited. Retrying in {} seconds. Failed to send POST request to {}. Status: {}, payload: {}",
                    backoff.as_secs(),
                    url,
                    response.status(),
                    payload
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
                    "Failed to send POST request to {}. Status: {}, payload: {}",
                    url,
                    response.status(),
                    payload
                );
                #[cfg(feature = "standalone")]
                tracing::error!(
                    "Failed to send POST request to {}. Status: {}, payload: {}",
                    url,
                    response.status(),
                    payload
                );
                return Err(response.status());
            }
        }
    }
}
