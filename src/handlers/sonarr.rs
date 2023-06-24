use crate::structs::discord::DiscordWebhook;
use crate::structs::sonarr::RequestBody;
use reqwest::{Error};
use serde_json::{Value};
use std::collections::HashMap;
use std::env;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use twilight_model::channel::message::embed::{Embed};

pub struct SonarrHandler {
    // This will hold the state for each ongoing timer and queue of requests.
    // The HashMap key will be the URL of the request, and the value will be the state for that URL.
    timers: Arc<Mutex<HashMap<String, TimerState>>>,
}

struct TimerState {
    // This will hold the queue of requests for this URL.
    queue: Vec<RequestData>,
    // This will hold when the timer for this URL is set to expire.
    timer_end: Instant,
    // This will hold the current timer ID for this URL.
    timer_id: usize,
}

struct RequestData {
    // This will hold the headers and body of each queued request.
    body: Value,
}

#[derive(Eq, PartialEq, Hash, Debug)]
struct SonarrGroupKey(u64, u64);

#[derive(Debug)]
struct SonarrGroups(HashMap<SonarrGroupKey, Vec<RequestBody>>);

impl Deref for SonarrGroups {
    type Target = HashMap<SonarrGroupKey, Vec<RequestBody>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SonarrGroups {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SonarrHandler {
    pub fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&self, request_path: String, body: Value) -> impl warp::Reply {
        let new_state: Option<TimerState>;

        {
            let mut timers = self.timers.lock().await;

            // Check if there is already a TimerState for this URL.
            if let Some(timer_state) = timers.get_mut(&request_path) {
                // If there is a TimerState, add this request to the queue and update the timer_end Instant.
                timer_state.queue.push(RequestData { body });
                timer_state.timer_end = Instant::now() + Duration::from_secs(15);
                new_state = None;
            } else {
                // If there isn't a TimerState, create one with this request in the queue and a new timer_end Instant.
                let timer_state = TimerState {
                    queue: vec![RequestData { body }],
                    timer_end: Instant::now() + Duration::from_secs(15), // start a new timer for 15 seconds
                    timer_id: 0,
                };
                new_state = Some(timer_state);
            }
        }

        if let Some(timer_state) = new_state {
            let mut timers = self.timers.lock().await;
            timers.insert(request_path.clone(), timer_state);
        }

        // Now that the request has been added to the queue and the timer_end Instant has been updated,
        // we need to start the timer if it's not already running.
        self.start_timer(request_path).await;

        // For now, just return a simple response. We'll modify this later to return a more useful response.
        warp::reply::json(&"Received Sonarr request")
    }

    async fn start_timer(&self, request_path: String) {
        let mut timers = self.timers.lock().await;

        if let Some(timer_state) = timers.get_mut(&request_path) {
            // Increment the timer ID.
            timer_state.timer_id += 1;
            let timer_id = timer_state.timer_id;

            // Start a new timer.
            let timer_end = timer_state.timer_end;
            let duration = timer_end - Instant::now();

            // Drop the lock before starting the timer.
            drop(timers);

            let timers = Arc::clone(&self.timers);
            tokio::spawn(async move {
                tokio::time::sleep(duration).await;

                let mut timers = timers.lock().await;
                if let Some(timer_state) = timers.get_mut(&request_path) {
                    // Only proceed if the timer ID hasn't changed.
                    if timer_state.timer_id == timer_id {
                        // Process the queued requests here.
                        process_and_send_requests(&mut timer_state.queue, &request_path).await;
                    }
                }
            });
        }
    }
}

async fn process_and_send_requests(queue: &mut Vec<RequestData>, request_path: &str) {
    let grouped_requests = group_sonarr_requests(queue);

    let discord_baseurl =
        env::var("DISCORD_WEBHOOK_BASEURL").unwrap_or("https://discord.com".to_string());
    let matrix_baseurl = env::var("MATRIX_WEBHOOK_BASEURL").unwrap_or("matrixlol".to_string());

    let baseurl = if request_path.ends_with("/matrix") {
        matrix_baseurl
    } else if request_path.ends_with("/discord") {
        discord_baseurl
    } else {
        "idklol".to_string() // or handle this case differently
    };

    for (_group_key, requests) in grouped_requests.iter() {
        let description = requests
            .iter()
            .flat_map(|request| {
                request.episodes.iter().map(move |episode| {
                    let quality = request
                        .episode_file
                        .as_ref()
                        .map_or("None", |episode_file| &episode_file.quality);

                    format!(
                        "{}x{:02} - {} [{}]",
                        episode.season_number, episode.episode_number, episode.title, quality
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        
        let embed = Embed {
            title: Some(requests[0].series.title.to_string()),
            color: Some(0xFF0000), // Discord's color for red
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

        let webhook = DiscordWebhook {
            content: "".to_string(),
            embeds: vec![embed],
        };
        
        let url = format!("{}{}", baseurl, request_path);
        if let Err(e) = send_post_request(url, webhook).await {
            eprintln!("Failed to send POST request: {}", e);
        }
    }
}

fn group_sonarr_requests(queue: &mut Vec<RequestData>) -> SonarrGroups {
    let mut grouped_requests: SonarrGroups = SonarrGroups(HashMap::new());

    while let Some(request_data) = queue.pop() {
        let request: RequestBody = serde_json::from_value(request_data.body.clone()).unwrap();
        for episode in request.episodes.iter() {
            let group_key = SonarrGroupKey(episode.series_id, episode.season_number); // Only series_id is used as the key
            grouped_requests
                .entry(group_key)
                .or_insert(Vec::new())
                .push(request.clone());
        }
    }

    grouped_requests
}

async fn send_post_request(url: String, payload: DiscordWebhook) -> Result<(), Error> {
    // let client = Client::new();
    // client.post(url).json(&payload).send().await?;
    let payload_str = serde_json::to_string_pretty(&payload);

    // Print the formatted JSON string
    println!("Sending POST request to {}\n{:#?}", url, payload_str);
    Ok(())
}
