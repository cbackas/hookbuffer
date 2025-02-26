use std::{collections::HashMap, time::Duration};

use crate::{
    send::send_post_request,
    structs::{discord::DiscordWebhook, sonarr::SonarrRequestBody},
};
use structs::sonarr::SonarrGroupKey;
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

#[durable_object]
pub struct ChannelQueue {
    items: HashMap<SonarrGroupKey, Vec<SonarrRequestBody>>,
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for ChannelQueue {
    fn new(state: State, env: Env) -> Self {
        Self {
            items: HashMap::new(),
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
        let group_key: SonarrGroupKey = (&sonarr_event).into();
        self.items.entry(group_key).or_default().push(sonarr_event);
        self.state.storage().put("items", &self.items).await?;
        self.state.storage().put("url", req.path()).await?;

        console_log!("Added item to queue, length: {}", self.items.len());

        Response::from_json(&serde_json::json!({
            "success": true,
            "queue_length": self.items.len()
        }))
    }

    async fn alarm(&mut self) -> Result<Response> {
        let grouped_items: HashMap<SonarrGroupKey, Vec<SonarrRequestBody>> =
            self.state.storage().get("items").await?;
        self.state.storage().delete("items").await?;

        console_log!(
            "Alarm triggered, processing {} items in queue",
            grouped_items.len()
        );

        let webhooks: Vec<DiscordWebhook> =
            grouped_items.iter().map(|group| group.1.into()).collect();
        let url = {
            let path: String = self.state.storage().get("url").await?;
            format!("https://discord.com{}", path)
        };
        for webhook in webhooks {
            let _status = send_post_request(url.clone(), webhook).await;
            Delay::from(Duration::from_secs(1)).await;
        }

        Response::from_json(&serde_json::json!({
            "success": true,
            "processed_items": grouped_items.len()
        }))
    }
}
