use serde::{Deserialize, Serialize};
use twilight_model::channel::message::Embed;

use super::sonarr::SonarrEventType;
use super::summary::GroupSummary;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscordWebhook {
    pub url: String,
    pub body: DiscordWebhookBody,
}

impl DiscordWebhook {
    pub fn new(url: String, body: DiscordWebhookBody) -> Self {
        DiscordWebhook { url, body }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscordWebhookBody {
    pub content: String,
    pub embeds: Vec<Embed>,
}

fn event_color(event_type: SonarrEventType) -> u32 {
    match event_type {
        SonarrEventType::Test => 0x0800FF,     // blue
        SonarrEventType::Grab => 0xFFC130,     // yellow
        SonarrEventType::Download => 0x29A44C, // green
        SonarrEventType::Upgrade => 0x3E6800,  // dark green
        SonarrEventType::Rename => 0xFF00FF,   // purple
        _ => 0xFFFFFF,
    }
}

impl From<&GroupSummary> for DiscordWebhookBody {
    fn from(summary: &GroupSummary) -> Self {
        let embed = Embed {
            title: Some(summary.series_title.clone()),
            color: Some(event_color(summary.event_type)),
            fields: Vec::new(),
            kind: "rich".to_string(),
            author: None,
            description: Some(summary.description()),
            footer: None,
            image: None,
            provider: None,
            thumbnail: None,
            timestamp: None,
            url: None,
            video: None,
        };

        DiscordWebhookBody {
            content: summary.headline.clone(),
            embeds: vec![embed],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::summary::GroupSummary;

    fn summary(event_type: SonarrEventType) -> GroupSummary {
        GroupSummary {
            event_type,
            series_title: "Fake Series".into(),
            season_number: 1,
            headline: "Grabbed: Fake Series Season 01".into(),
            episodes: Vec::new(),
        }
    }

    #[test]
    fn maps_summary_onto_embed() {
        let body = DiscordWebhookBody::from(&summary(SonarrEventType::Grab));
        assert_eq!(body.content, "Grabbed: Fake Series Season 01");
        assert_eq!(body.embeds.len(), 1);
        assert_eq!(body.embeds[0].title, Some("Fake Series".to_string()));
    }

    #[test]
    fn colors() {
        assert_eq!(
            DiscordWebhookBody::from(&summary(SonarrEventType::Grab)).embeds[0].color,
            Some(0xFFC130)
        );
        assert_eq!(
            DiscordWebhookBody::from(&summary(SonarrEventType::Download)).embeds[0].color,
            Some(0x29A44C)
        );
        assert_eq!(
            DiscordWebhookBody::from(&summary(SonarrEventType::Upgrade)).embeds[0].color,
            Some(0x3E6800)
        );
    }
}
