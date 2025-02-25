use std::{cmp::Ordering, collections::HashMap, time::Duration};

use worker::*;

use crate::{
    send::send_post_request,
    structs::{
        discord::DiscordWebhook,
        sonarr::{SonarrEventType, SonarrRequestBody},
    },
};

#[durable_object]
pub struct Queue {
    items: Vec<SonarrRequestBody>,
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for Queue {
    fn new(state: State, env: Env) -> Self {
        Self {
            items: Vec::new(),
            state,
            env,
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        self.state.storage().set_alarm(15 * 1000).await?;

        let sonarr_event: SonarrRequestBody = {
            let mut req = req.clone()?;
            req.json().await?
        };
        let path = req.path();

        self.items.push(sonarr_event);
        self.state.storage().put("items", &self.items).await?;
        self.state.storage().put("url", path).await?;

        console_log!("Added item to queue, length: {}", self.items.len());

        Response::from_json(&serde_json::json!({
            "success": true,
            "queue_length": self.items.len()
        }))
    }

    async fn alarm(&mut self) -> Result<Response> {
        let stored_items: Vec<SonarrRequestBody> = self.state.storage().get("items").await?;
        self.state.storage().delete("items").await?;

        console_log!(
            "Alarm triggered, processing {} items in queue",
            stored_items.len()
        );

        let grouped_items: Vec<DiscordWebhook> = group_sonarr_requests(stored_items)
            .iter()
            .map(|group| group.into())
            .collect();
        let url = {
            let path: String = self.state.storage().get("url").await?;
            format!("https://discord.com{}", path)
        };
        for webhook in grouped_items {
            let _status = send_post_request(url.clone(), webhook).await;
            Delay::from(Duration::from_secs(1)).await;
        }

        Response::ok("Queue processed")
    }
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub struct SonarrGroupKey(pub u64, pub SonarrEventType, pub u64);

impl PartialOrd for SonarrGroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SonarrGroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0, &self.1, self.2).cmp(&(other.0, &other.1, other.2))
    }
}

fn group_sonarr_requests(queue: Vec<SonarrRequestBody>) -> Vec<Vec<SonarrRequestBody>> {
    let mut grouped_requests: HashMap<SonarrGroupKey, Vec<SonarrRequestBody>> = HashMap::new();

    let mut queue = queue;
    while let Some(mut sonarr_request) = queue.pop() {
        for episode in sonarr_request.episodes.iter() {
            // pull the event_type out and check if its an import or an upgrade
            let event_type = sonarr_request.event_type.clone().unwrap();
            let event_type = match event_type {
                SonarrEventType::Download => {
                    if sonarr_request.is_upgrade.unwrap_or(false) {
                        SonarrEventType::Upgrade
                    } else {
                        SonarrEventType::Download
                    }
                }
                _ => event_type,
            };
            sonarr_request.event_type = Some(event_type.clone()); // save it back to the request

            // add the request to the appropriate group
            grouped_requests
                .entry(SonarrGroupKey(
                    episode.series_id,
                    event_type,
                    episode.season_number,
                ))
                .or_default()
                .push(sonarr_request.clone());
        }
    }

    grouped_requests.into_values().collect()
}
