use serde::{Deserialize, Serialize};
use twilight_model::channel::message::Embed;

use super::sonarr::{SonarrEventType, SonarrRequestBody};

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

impl From<Vec<SonarrRequestBody>> for DiscordWebhookBody {
    fn from(sonarr_data: Vec<SonarrRequestBody>) -> Self {
        let event_type = sonarr_data[0].event_type.as_ref().unwrap();
        let series_title = &sonarr_data[0].series.title;
        let season_number = sonarr_data[0].episodes[0].season_number;

        let content = match event_type {
            SonarrEventType::Grab => "Grabbed",
            SonarrEventType::Download => {
                if sonarr_data[0].is_upgrade.unwrap_or(false) {
                    "Upgraded"
                } else {
                    "Imported"
                }
            }
            SonarrEventType::Rename => "Renamed",
            _ => "Unknown",
        };
        let content = match sonarr_data.len() {
            1 => format!(
                "{}: {} - {:02}x{:02} - {}",
                content,
                series_title,
                season_number,
                sonarr_data[0].episodes[0].episode_number,
                sonarr_data[0].episodes[0].title
            ),
            _ => format!("{}: {} Season {:02}", content, series_title, season_number),
        };

        let mut episodes_with_quality: Vec<_> = sonarr_data
            .iter()
            .flat_map(|request| {
                let quality = request
                    .episode_file
                    .as_ref()
                    .map(|episode_file| episode_file.quality.clone())
                    .or_else(|| request.release.clone()?.quality)
                    .unwrap_or_else(|| "None".to_string());
                request.episodes.iter().map(move |episode| {
                    (
                        episode.season_number,
                        episode.episode_number,
                        episode.title.clone(),
                        quality.clone(),
                    )
                })
            })
            .fold(
                Vec::new(),
                |mut acc: Vec<(u64, u64, String, String, u64)>, x| {
                    match acc
                        .iter()
                        .position(|(s, e, _, _, _)| *s == x.0 && *e == x.1)
                    {
                        Some(i) => acc[i].4 += 1,
                        None => acc.push((x.0, x.1, x.2, x.3, 1)),
                    };
                    acc
                },
            );
        episodes_with_quality.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(&b.3))
        });

        let description = episodes_with_quality
            .into_iter()
            .map(
                |(season_number, episode_number, title, quality, count)| match count {
                    1 => format!(
                        "{:02}x{:02} - {} [{}]",
                        season_number, episode_number, title, quality
                    ),
                    _ => format!(
                        "{:02}x{:02} - {} [{}] ({}x)",
                        season_number, episode_number, title, quality, count
                    ),
                },
            )
            .collect::<Vec<_>>()
            .join("\n");

        let color = match event_type {
            SonarrEventType::Test => 0x0800FF,     // blue
            SonarrEventType::Grab => 0xFFC130,     // yellow
            SonarrEventType::Download => 0x29A44C, // green
            SonarrEventType::Upgrade => 0x3E6800,  // dark green
            SonarrEventType::Rename => 0xFF00FF,   // purple
            _ => 0xFFFFFF,
        };

        let embed = Embed {
            title: Some(series_title.to_string()),
            color: Some(color),
            fields: Vec::new(),
            kind: "rich".to_string(),
            author: None,
            description: Some(description),
            footer: None,
            image: None,
            provider: None,
            thumbnail: None,
            timestamp: None,
            url: None,
            video: None,
        };

        DiscordWebhookBody {
            content,
            embeds: vec![embed],
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::structs::sonarr::{
//         SonarrEpisode, SonarrEpisodeFile, SonarrEventType, SonarrRequestBody, SonarrSeries,
//     };
//
//     use std::collections::hash_map::DefaultHasher;
//     use std::hash::{Hash, Hasher};
//
//     use super::*;
//
//     fn hash_str(s: &str) -> u64 {
//         let mut hasher = DefaultHasher::new();
//         s.hash(&mut hasher);
//         hasher.finish()
//     }
//
//     fn create_episode_request(
//         series_title: &str,
//         episode_title: &str,
//         season_number: u64,
//         episode_number: u64,
//         event_type: SonarrEventType,
//     ) -> SonarrRequestBody {
//         let series_id = hash_str(series_title);
//         SonarrRequestBody {
//             series: SonarrSeries {
//                 title: series_title.to_string(),
//                 id: Some(series_id),
//                 imdb_id: None,
//                 path: None,
//                 title_slug: None,
//                 tvdb_id: None,
//                 tv_maze_id: None,
//                 year: None,
//                 type_: None,
//             },
//             event_type: Some(event_type.clone()),
//             episodes: vec![SonarrEpisode {
//                 episode_number,
//                 season_number,
//                 title: episode_title.to_string(),
//                 series_id,
//                 air_date: None,
//                 air_date_utc: None,
//                 id: None,
//                 overview: None,
//             }],
//             episode_file: Some(SonarrEpisodeFile {
//                 id: None,
//                 relative_path: None,
//                 path: None,
//                 quality: "Fake Quality".to_string(),
//                 quality_version: None,
//                 release_group: None,
//                 scene_name: None,
//                 size: None,
//                 date_added: None,
//                 media_info: None,
//             }),
//             release: None,
//             is_upgrade: Some(event_type == SonarrEventType::Upgrade),
//             application_url: None,
//             download_client: None,
//             download_id: None,
//             custom_format_info: None,
//             instance_name: None,
//         }
//     }
//
//     mod group_sonarr_requests {
//         use super::*;
//
//         #[test]
//         fn episode_sorting() {
//             let mut queue = vec![
//                 // group 1
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Grab,
//                 ),
//                 // group 2
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Download,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Download,
//                 ),
//                 // group 3
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Upgrade,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Upgrade,
//                 ),
//                 // group 4
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Grab,
//                 ),
//                 // group 5
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Download,
//                 ),
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Download,
//                 ),
//                 // group 6
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Upgrade,
//                 ),
//                 create_episode_request(
//                     "The Fakest Show",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Upgrade,
//                 ),
//             ];
//
//             let result = group_sonarr_requests(&mut queue);
//
//             assert_eq!(result.len(), 6);
//             assert_eq!(queue.len(), 0);
//         }
//     }
//
//     mod convert_group_to_webhook {
//         use super::*;
//         #[test]
//         fn multiple_episodes() {
//             let requests = vec![
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 4",
//                     1,
//                     4,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 3",
//                     1,
//                     3,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 6",
//                     1,
//                     6,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 2",
//                     1,
//                     2,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 5",
//                     1,
//                     5,
//                     SonarrEventType::Grab,
//                 ),
//             ];
//
//             let webhook = convert_group_to_webhook(&requests);
//
//             assert_eq!(webhook.embeds.len(), 1);
//             assert_eq!(webhook.content, "Grabbed: Fake Series 1 Season 01");
//             assert_eq!(webhook.embeds[0].title, Some("Fake Series 1".to_string()));
//             assert_eq!(
//                 webhook.embeds[0].description,
//                 Some("01x01 - Fake Episode 1 [Fake Quality]\n01x02 - Fake Episode 2 [Fake Quality]\n01x03 - Fake Episode 3 [Fake Quality]\n01x04 - Fake Episode 4 [Fake Quality]\n01x05 - Fake Episode 5 [Fake Quality]\n01x06 - Fake Episode 6 [Fake Quality]".to_string())
//             );
//         }
//
//         #[test]
//         fn single_episode() {
//             let requests = vec![create_episode_request(
//                 "Fake Series",
//                 "Fake Episode 1",
//                 1,
//                 1,
//                 SonarrEventType::Grab,
//             )];
//
//             let webhook = convert_group_to_webhook(&requests);
//
//             assert_eq!(webhook.embeds.len(), 1);
//             assert_eq!(
//                 webhook.content,
//                 "Grabbed: Fake Series - 01x01 - Fake Episode 1"
//             );
//             assert_eq!(webhook.embeds[0].title, Some("Fake Series".to_string()));
//             assert_eq!(
//                 webhook.embeds[0].description,
//                 Some("01x01 - Fake Episode 1 [Fake Quality]".to_string())
//             );
//         }
//
//         #[test]
//         fn repeated_episodes() {
//             let requests = vec![
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 4",
//                     1,
//                     4,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 4",
//                     1,
//                     4,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 4",
//                     1,
//                     4,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 1",
//                     1,
//                     1,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 6",
//                     1,
//                     6,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 6",
//                     1,
//                     6,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 6",
//                     1,
//                     6,
//                     SonarrEventType::Grab,
//                 ),
//                 create_episode_request(
//                     "Fake Series 1",
//                     "Fake Episode 6",
//                     1,
//                     6,
//                     SonarrEventType::Grab,
//                 ),
//             ];
//
//             let webhook = convert_group_to_webhook(&requests);
//
//             assert_eq!(webhook.embeds.len(), 1);
//             assert_eq!(webhook.content, "Grabbed: Fake Series 1 Season 01");
//             assert_eq!(webhook.embeds[0].title, Some("Fake Series 1".to_string()));
//             assert_eq!(
//                 webhook.embeds[0].description,
//                 Some("01x01 - Fake Episode 1 [Fake Quality] (3x)\n01x04 - Fake Episode 4 [Fake Quality] (3x)\n01x06 - Fake Episode 6 [Fake Quality] (4x)".to_string())
//             );
//         }
//
//         #[test]
//         fn colors() {
//             let grab_webhook = convert_group_to_webhook(&vec![create_episode_request(
//                 "Fake Series",
//                 "Fake Episode 1",
//                 1,
//                 1,
//                 SonarrEventType::Grab,
//             )]);
//             let download_webhook = convert_group_to_webhook(&vec![create_episode_request(
//                 "Fake Series",
//                 "Fake Episode 1",
//                 1,
//                 1,
//                 SonarrEventType::Download,
//             )]);
//             let upgrade_webhook = convert_group_to_webhook(&vec![create_episode_request(
//                 "Fake Series",
//                 "Fake Episode 1",
//                 1,
//                 1,
//                 SonarrEventType::Upgrade,
//             )]);
//
//             assert_eq!(grab_webhook.embeds[0].color, Some(0xFFC130));
//             assert_eq!(download_webhook.embeds[0].color, Some(0x29A44C));
//             assert_eq!(upgrade_webhook.embeds[0].color, Some(0x3E6800));
//         }
//     }
// }
