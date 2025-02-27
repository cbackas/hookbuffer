use std::{
    hash::{DefaultHasher, Hash, Hasher},
    time::Duration,
};

use crate::send::send_post_request;
use structs::{
    discord::{DiscordWebhook, DiscordWebhookBody},
    sonarr::{SonarrGroupKey, SonarrRequestBody},
};
use wasm_bindgen::JsValue;
use worker::*;

mod send;
mod structs;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .on_async(
            "/api/webhooks/:id/:token",
            async |req: Request, ctx: RouteContext<()>| {
                match req.headers().get("User-Agent") {
                    Ok(Some(user_agent)) if user_agent.starts_with("Sonarr/") => {}
                    _ => return Response::error("Invalid User-Agent", 400),
                };
                let group_id = ctx.param("id").unwrap();

                console_log!("Recieved webhook for group: {}", group_id);

                let namespace = ctx.durable_object("HOOKBUFFER")?;
                let stub = namespace.id_from_name(group_id)?.get_stub()?;
                stub.fetch_with_request(req).await
            },
        )
        .run(req, env)
        .await
}

fn hash_group_key(s: &SonarrGroupKey) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[durable_object]
pub struct ChannelQueue {
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for ChannelQueue {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        self.state.storage().set_alarm(15 * 1000).await?;

        let sonarr_event: SonarrRequestBody = {
            let mut req = req.clone()?;
            req.json().await?
        };
        let group_key = {
            let key: SonarrGroupKey = (&sonarr_event).into();
            format!("groupkey-{}", hash_group_key(&key))
        };

        let group_items = {
            let mut items = self
                .state
                .storage()
                .get::<Vec<SonarrRequestBody>>(&group_key)
                .await
                .unwrap_or_default();
            items.push(sonarr_event);
            self.state.storage().put(&group_key, &items).await?;
            self.state.storage().put("url", req.path()).await?;
            items.len()
        };

        console_log!("Added item to channel queue, group length: {}", group_items);

        Response::from_json(&serde_json::json!({
            "success": true,
            "queue_length": group_items
        }))
    }

    async fn alarm(&mut self) -> Result<Response> {
        let outbound_queue = self.env.queue("outbound_messages")?;

        let list_options = ListOptions::new().prefix("groupkey-");
        let storage_map = self
            .state
            .storage()
            .list_with_options(list_options)
            .await?
            .entries();

        let url = &{
            let path: String = self.state.storage().get("url").await?;
            format!("https://discord.com{}", path)
        };

        for entry in storage_map {
            let (group_key, group_items) = entry
                .and_then(|val| {
                    if val.is_undefined() {
                        Err(JsValue::from("No such value in storage."))
                    } else {
                        serde_wasm_bindgen::from_value::<(String, Vec<SonarrRequestBody>)>(val)
                            .map_err(|e| JsValue::from(e.to_string()))
                    }
                })
                .map_err(Error::from)?;

            let webhook: DiscordWebhookBody = group_items.into();
            self.state.storage().delete(&group_key).await?;
            outbound_queue
                .send(DiscordWebhook::new(url.to_string(), webhook))
                .await?;
        }

        Response::from_json(&serde_json::json!({
            "success": true,
        }))
    }
}

#[event(queue)]
pub async fn consume_webhook_queue(
    message_batch: MessageBatch<DiscordWebhook>,
    _env: Env,
    _ctx: Context,
) -> Result<()> {
    let messages: Vec<Message<DiscordWebhook>> = message_batch.messages()?;

    for message in &messages {
        match send_post_request(message.body()).await {
            Ok(_) => message.ack(),
            Err(_) => message.retry(),
        };
        Delay::from(Duration::from_secs(1)).await;
    }

    Ok(())
}
