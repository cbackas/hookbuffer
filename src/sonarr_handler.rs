use crate::send::send_post_request;
use crate::structs::discord::DiscordWebhook;
use crate::structs::hookbuffer::SonarrGroupKey;
use crate::structs::sonarr::{SonarrEventType, SonarrRequestBody};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, Instant};
use twilight_model::channel::message::embed::Embed;
use warp::reply::Json;

pub struct SonarrHandler {
    // this will hold the state for each ongoing timer and queue of requests
    // the HashMap key will be the URL of the request, and the value will be the state for that URL
    timers: Arc<Mutex<HashMap<String, TimerState>>>,
}

struct TimerState {
    // this will hold the queue of requests for this URL
    queue: Vec<SonarrRequestBody>,
    // this will hold when the timer for this URL is set to expire
    timer_end: Instant,
    // this will hold the current timer ID for this URL
    timer_id: usize,
}

impl SonarrHandler {
    pub fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&self, request_path: String, body: Value) -> warp::reply::WithStatus<Json> {
        // parse the request body into a SonarrRequestBody
        let mut sonarr_request: SonarrRequestBody = serde_json::from_value(body).unwrap();

        // if the event type is Download, check if it's an upgrade and change the event type to Upgrade if it is
        let event_type = sonarr_request.event_type.unwrap();
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
        // save the event type back to request object
        sonarr_request.event_type = Some(event_type.clone());

        if !(event_type == SonarrEventType::Grab || event_type == SonarrEventType::Download || event_type == SonarrEventType::Upgrade) {
            // if the event type is not Download or Upgrade, return a 400
            return warp::reply::with_status(
                warp::reply::json(&"Unsupported event type, must be Grab or Download"),
                warp::http::StatusCode::BAD_REQUEST,
            );
        }

        for episode in &sonarr_request.episodes {
            println!(
                "[Recieved] {:?} Episode: {} - {:02}x{:02}",
                sonarr_request.event_type.clone().unwrap(),
                sonarr_request.series.title,
                episode.season_number,
                episode.episode_number
            );
        }

        let timer_end = Instant::now() + Duration::from_secs(15);

        {
            let mut timers = self.timers.lock().await;

            // check if there is already a TimerState for this URL
            if let Some(timer_state) = timers.get_mut(&request_path) {
                // if there is a TimerState, add this request to the queue and update the timer_end Instant
                timer_state.queue.push(sonarr_request);
                timer_state.timer_end = timer_end;
            } else {
                // if there isn't a TimerState, create one with this request in the queue and a new timer_end Instant
                println!("[Timer] new timer started for {}", request_path);
                let timer_state = TimerState {
                    queue: vec![sonarr_request],
                    timer_end,
                    timer_id: 0,
                };
                timers.insert(request_path.clone(), timer_state);
            }
        }

        // now that the request has been added to the queue and the timer_end Instant has been updated
        // we need to start the timer if it's not already running
        self.start_timer(request_path).await;

        warp::reply::with_status(warp::reply::json(&"Request added to queue"), warp::http::StatusCode::OK)
    }

    async fn start_timer(&self, request_path: String) {
        // get the needed information first and then release the lock
        let (timer_id, timer_end) = {
            let mut timers = self.timers.lock().await;

            if let Some(timer_state) = timers.get_mut(&request_path) {
                // increment the timer ID
                timer_state.timer_id += 1;
                let timer_id = timer_state.timer_id;

                // start a new timer
                let timer_end = timer_state.timer_end;

                (timer_id, timer_end) // return this information to use later
            } else {
                return; // no timer state found for this request_path
            }
        };

        // now you're free to start the timer without holding the lock
        let timers = Arc::clone(&self.timers);
        tokio::spawn(process_timer(timers, request_path, timer_id, timer_end));
    }
}

// this function is spawned when a url timer expires and it processes the queue of requests
async fn process_timer(timers: Arc<Mutex<HashMap<String, TimerState>>>, request_path: String, timer_id: usize, timer_end: Instant) {
    let duration = timer_end - Instant::now();
    tokio::time::sleep(duration).await;

    let timer_state_queue = {
        let mut timers = timers.lock().await;
        if let Some(timer_state) = timers.get_mut(&request_path) {
            // only proceed if the timer ID hasn't changed
            // this is how we know the timer hasn't been reset since this function was spawned
            if timer_state.timer_id == timer_id {
                println!(
                    "[Timer] timer expired for {} with {} requests in queue",
                    request_path,
                    timer_state.queue.len()
                );

                // take ownership of the queue, leaving an empty one in its place
                Some(std::mem::take(&mut timer_state.queue))
            } else {
                None
            }
        } else {
            None
        }
    };

    let destination_url = crate::env::get_destination_url();

    if let Some(mut queue) = timer_state_queue {
        let grouped_requests = group_sonarr_requests(&mut queue);

        let mut sorted_groups: Vec<(&SonarrGroupKey, &Vec<SonarrRequestBody>)> = grouped_requests.iter().collect();
        sorted_groups.sort_unstable_by(|a, b| a.0.cmp(b.0));

        for (group_key, sonarr_data) in sorted_groups {
            let webhook = convert_group_to_webhook(sonarr_data);

            match send_post_request(destination_url.to_string(), request_path.to_string(), webhook).await {
                Ok(_) => {
                    println!("[Forwarded] {:?} sent to discord succesfully successfully", group_key);
                }
                Err(e) => {
                    eprintln!("Failed to send POST request: {:?}", e);
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    }
}

// connvert all the sonarr requests in the queue into aa map of groupings
// groupings based on series, event type, and season
fn group_sonarr_requests(queue: &mut Vec<SonarrRequestBody>) -> HashMap<SonarrGroupKey, Vec<SonarrRequestBody>> {
    let mut grouped_requests: HashMap<SonarrGroupKey, Vec<SonarrRequestBody>> = HashMap::new();

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
                .entry(SonarrGroupKey(episode.series_id, event_type, episode.season_number))
                .or_insert(Vec::new())
                .push(sonarr_request.clone());
        }
    }

    grouped_requests
}

// convert a group of sonarr requests into a discord webhook with embed
fn convert_group_to_webhook(sonarr_data: &[SonarrRequestBody]) -> DiscordWebhook {
    let event_type = sonarr_data[0].event_type.as_ref().unwrap();
    let series_title = &sonarr_data[0].series.title;
    let season_number = sonarr_data[0].episodes[0].season_number;

    let content = match event_type {
        SonarrEventType::Grab => "Grabbed",
        SonarrEventType::Download => "Imported",
        SonarrEventType::Upgrade => "Upgraded",
        SonarrEventType::Rename => "Renamed",
        _ => "Unknown",
    };
    let content = match sonarr_data.len() {
        1 => format!(
            "{}: {} - {:02}x{:02} - {}",
            content, series_title, season_number, sonarr_data[0].episodes[0].episode_number, sonarr_data[0].episodes[0].title
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
            request
                .episodes
                .iter()
                .map(move |episode| (episode.season_number, episode.episode_number, episode.title.clone(), quality.clone()))
        })
        .collect();
    episodes_with_quality.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)).then(a.3.cmp(&b.3)));

    let description = episodes_with_quality
        .into_iter()
        .map(|(season_number, episode_number, title, quality)| format!("{:02}x{:02} - {} [{}]", season_number, episode_number, title, quality))
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

    DiscordWebhook {
        content,
        embeds: vec![embed],
    }
}

#[cfg(test)]
mod tests {
    use crate::structs::sonarr::{SonarrEpisode, SonarrEpisodeFile, SonarrSeries};

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::*;

    fn hash_str(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    fn create_episode_request(
        series_title: &str,
        episode_title: &str,
        season_number: u64,
        episode_number: u64,
        event_type: SonarrEventType,
    ) -> SonarrRequestBody {
        let series_id = hash_str(series_title);
        SonarrRequestBody {
            series: SonarrSeries {
                title: series_title.to_string(),
                id: Some(series_id),
                imdb_id: None,
                path: None,
                title_slug: None,
                tvdb_id: None,
                tv_maze_id: None,
                year: None,
                type_: None,
            },
            event_type: Some(event_type.clone()),
            episodes: vec![SonarrEpisode {
                episode_number: episode_number,
                season_number: season_number,
                title: episode_title.to_string(),
                series_id,
                air_date: None,
                air_date_utc: None,
                id: None,
                overview: None,
            }],
            episode_file: Some(SonarrEpisodeFile {
                id: None,
                relative_path: None,
                path: None,
                quality: "Fake Quality".to_string(),
                quality_version: None,
                release_group: None,
                scene_name: None,
                size: None,
                date_added: None,
                media_info: None,
            }),
            release: None,
            is_upgrade: Some(event_type == SonarrEventType::Upgrade),
            application_url: None,
            download_client: None,
            download_id: None,
            custom_format_info: None,
            instance_name: None,
        }
    }

    mod group_sonarr_requests {
        use super::*;

        #[test]
        fn test_group_sonarr_requests() {
            let mut queue = vec![
                // group 1
                create_episode_request("Fake Series 1", "Fake Episode 1", 1, 1, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 2", 1, 2, SonarrEventType::Grab),
                // group 2
                create_episode_request("Fake Series 1", "Fake Episode 1", 1, 1, SonarrEventType::Download),
                create_episode_request("Fake Series 1", "Fake Episode 2", 1, 2, SonarrEventType::Download),
                // group 3
                create_episode_request("Fake Series 1", "Fake Episode 1", 1, 1, SonarrEventType::Upgrade),
                create_episode_request("Fake Series 1", "Fake Episode 2", 1, 2, SonarrEventType::Upgrade),
                // group 4
                create_episode_request("The Fakest Show", "Fake Episode 1", 1, 1, SonarrEventType::Grab),
                create_episode_request("The Fakest Show", "Fake Episode 2", 1, 2, SonarrEventType::Grab),
                // group 5
                create_episode_request("The Fakest Show", "Fake Episode 1", 1, 1, SonarrEventType::Download),
                create_episode_request("The Fakest Show", "Fake Episode 2", 1, 2, SonarrEventType::Download),
                // group 6
                create_episode_request("The Fakest Show", "Fake Episode 1", 1, 1, SonarrEventType::Upgrade),
                create_episode_request("The Fakest Show", "Fake Episode 2", 1, 2, SonarrEventType::Upgrade),
            ];

            let result = group_sonarr_requests(&mut queue);

            assert_eq!(result.len(), 6);
            assert_eq!(queue.len(), 0);
        }
    }

    mod convert_group_to_webhook {
        use super::*;
        #[test]
        fn multiple_episodes() {
            let requests = vec![
                create_episode_request("Fake Series 1", "Fake Episode 4", 1, 4, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 3", 1, 3, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 6", 1, 6, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 1", 1, 1, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 2", 1, 2, SonarrEventType::Grab),
                create_episode_request("Fake Series 1", "Fake Episode 5", 1, 5, SonarrEventType::Grab),
            ];

            let webhook = convert_group_to_webhook(&requests);

            assert_eq!(webhook.embeds.len(), 1);
            assert_eq!(webhook.content, "Grabbed: Fake Series 1 Season 01");
            assert_eq!(webhook.embeds[0].title, Some("Fake Series 1".to_string()));
            assert_eq!(
                webhook.embeds[0].description,
                Some("01x01 - Fake Episode 1 [Fake Quality]\n01x02 - Fake Episode 2 [Fake Quality]\n01x03 - Fake Episode 3 [Fake Quality]\n01x04 - Fake Episode 4 [Fake Quality]\n01x05 - Fake Episode 5 [Fake Quality]\n01x06 - Fake Episode 6 [Fake Quality]".to_string())
            );
        }

        #[test]
        fn single_episode() {
            let requests = vec![create_episode_request("Fake Series", "Fake Episode 1", 1, 1, SonarrEventType::Grab)];

            let webhook = convert_group_to_webhook(&requests);

            assert_eq!(webhook.embeds.len(), 1);
            assert_eq!(webhook.content, "Grabbed: Fake Series - 01x01 - Fake Episode 1");
            assert_eq!(webhook.embeds[0].title, Some("Fake Series".to_string()));
            assert_eq!(webhook.embeds[0].description, Some("01x01 - Fake Episode 1 [Fake Quality]".to_string()));
        }

        #[test]
        fn colors() {
            let grab_webhook = convert_group_to_webhook(&vec![create_episode_request(
                "Fake Series",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Grab,
            )]);
            let download_webhook = convert_group_to_webhook(&vec![create_episode_request(
                "Fake Series",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Download,
            )]);
            let upgrade_webhook = convert_group_to_webhook(&vec![create_episode_request(
                "Fake Series",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Upgrade,
            )]);

            assert_eq!(grab_webhook.embeds[0].color, Some(0xFFC130));
            assert_eq!(download_webhook.embeds[0].color, Some(0x29A44C));
            assert_eq!(upgrade_webhook.embeds[0].color, Some(0x3E6800));
        }
    }
}
