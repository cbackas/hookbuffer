use serde::Serialize;
use twilight_model::channel::message::Embed;

#[derive(Serialize, Debug)]
pub struct DiscordWebhook {
    pub content: String,
    pub embeds: Vec<Embed>,
}
