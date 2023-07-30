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

        if !(event_type == SonarrEventType::Grab
            || event_type == SonarrEventType::Download
            || event_type == SonarrEventType::Upgrade)
        {
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

        warp::reply::with_status(
            warp::reply::json(&"Request added to queue"),
            warp::http::StatusCode::OK,
        )
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
async fn process_timer(
    timers: Arc<Mutex<HashMap<String, TimerState>>>,
    request_path: String,
    timer_id: usize,
    timer_end: Instant,
) {
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

    if let Some(mut queue) = timer_state_queue {
        let grouped_requests = group_sonarr_requests(&mut queue);

        let mut sorted_groups: Vec<(&SonarrGroupKey, &Vec<SonarrRequestBody>)> =
            grouped_requests.iter().collect();
        sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

        for (group_key, sonarr_data) in sorted_groups {
            let webhook = convert_group_to_webhook(&sonarr_data);

            match send_post_request(
                "https://discord.com/".to_string(),
                request_path.to_string(),
                webhook,
            )
            .await
            {
                Ok(_) => {
                    println!(
                        "[Forwarded] {:?} sent to discord succesfully successfully",
                        group_key
                    );
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
fn group_sonarr_requests(
    queue: &mut Vec<SonarrRequestBody>,
) -> HashMap<SonarrGroupKey, Vec<SonarrRequestBody>> {
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
                .entry(SonarrGroupKey(
                    episode.series_id,
                    event_type,
                    episode.season_number,
                ))
                .or_insert(Vec::new())
                .push(sonarr_request.clone());
        }
    }

    grouped_requests
}

// convert a group of sonarr requests into a discord webhook with embed
fn convert_group_to_webhook(sonarr_data: &Vec<SonarrRequestBody>) -> DiscordWebhook {
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
    let content = format!("{}: {} Season {:02}", content, series_title, season_number);

    let description = sonarr_data
        .iter()
        .flat_map(|request| {
            let quality = request
                .episode_file
                .as_ref()
                .and_then(|episode_file| Some(episode_file.clone().quality))
                .or_else(|| request.release.clone()?.quality)
                .unwrap_or_else(|| "None".to_string());
            request.episodes.iter().map(move |episode| {
                format!(
                    "{:02}x{:02} - {} [{}]",
                    episode.season_number, episode.episode_number, episode.title, quality
                )
            })
        })
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
