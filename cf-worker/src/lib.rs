use std::{
    hash::{DefaultHasher, Hash, Hasher},
    time::Duration,
};

use shared_lib::send::{Outbound, Target};
use shared_lib::structs::{
    pushover::PushoverConfig,
    sonarr::{SonarrGroupKey, SonarrRequestBody},
    summary::summarize_group,
};
use wasm_bindgen::JsValue;
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let headers = req.headers().into();
    // Basic auth check
    if let Ok(pass) = env.secret("SECRET_KEY") {
        if let Err(err) =
            shared_lib::auth::check_auth("admin".to_string(), pass.to_string(), &headers)
        {
            return Response::error(err.message, err.status.as_u16());
        }
    }

    match req.headers().get("User-Agent") {
        Ok(Some(user_agent)) if user_agent.starts_with("Sonarr/") => {}
        _ => return Response::error("Invalid User-Agent", 400),
    };

    Router::new()
        .on_async(
            "/api/webhooks/:id/:token",
            async |req: Request, ctx: RouteContext<()>| {
                let group_id = ctx.param("id").unwrap();

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

/// Resolve the forwarding target from the worker environment. `HOOKBUFFER_TARGET`
/// is a plain var (`discord` by default); Pushover credentials come from secrets.
/// Falls back to Discord when `pushover` is requested without credentials.
fn get_target(env: &Env) -> Target {
    let target = env
        .var("HOOKBUFFER_TARGET")
        .map(|v| v.to_string())
        .unwrap_or_default();

    if target.eq_ignore_ascii_case("pushover") {
        match pushover_config(env) {
            Some(config) => Target::Pushover(config),
            None => {
                console_error!(
                    "HOOKBUFFER_TARGET=pushover but PUSHOVER_TOKEN and PUSHOVER_USER are not both set; falling back to discord"
                );
                Target::Discord
            }
        }
    } else {
        if !target.is_empty() && !target.eq_ignore_ascii_case("discord") {
            console_warn!(
                "Unknown HOOKBUFFER_TARGET '{}', defaulting to discord",
                target
            );
        }
        Target::Discord
    }
}

fn pushover_config(env: &Env) -> Option<PushoverConfig> {
    let token = secret(env, "PUSHOVER_TOKEN")?;
    let user = secret(env, "PUSHOVER_USER")?;
    let priority = env
        .var("PUSHOVER_PRIORITY")
        .ok()
        .and_then(|v| v.to_string().parse::<i8>().ok())
        .filter(|priority| (-2..=2).contains(priority));
    let sound = env
        .var("PUSHOVER_SOUND")
        .ok()
        .map(|v| v.to_string())
        .filter(|value| !value.is_empty());

    Some(PushoverConfig {
        token,
        user,
        priority,
        sound,
    })
}

fn secret(env: &Env, name: &str) -> Option<String> {
    env.secret(name)
        .ok()
        .map(|secret| secret.to_string())
        .filter(|value| !value.is_empty())
}

#[durable_object]
pub struct ChannelQueue {
    state: State,
    env: Env,
}

impl DurableObject for ChannelQueue {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
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
                .unwrap_or_default()
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

    async fn alarm(&self) -> Result<Response> {
        let outbound_queue = self.env.queue("outbound_messages")?;
        let target = get_target(&self.env);

        let list_options = ListOptions::new().prefix("groupkey-");
        let storage_map = self
            .state
            .storage()
            .list_with_options(list_options)
            .await?
            .entries();

        let url = &{
            let path: String = self
                .state
                .storage()
                .get("url")
                .await?
                .expect("URL should be set if there are items in the queue.");
            format!("https://discord.com{path}")
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

            let message = target.build(&summarize_group(&group_items), url);
            self.state.storage().delete(&group_key).await?;
            outbound_queue.send(message).await?;
        }

        Response::from_json(&serde_json::json!({
            "success": true,
        }))
    }
}

#[event(queue)]
pub async fn consume_webhook_queue(
    message_batch: MessageBatch<Outbound>,
    _env: Env,
    _ctx: Context,
) -> Result<()> {
    let messages: Vec<Message<Outbound>> = message_batch.messages()?;

    for message in messages {
        match shared_lib::send::send_post_request(message.body()).await {
            Ok(_) => message.ack(),
            Err(_) => message.retry(),
        };
        Delay::from(Duration::from_secs(1)).await;
    }

    Ok(())
}
