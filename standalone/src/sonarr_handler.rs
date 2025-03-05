use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use shared_lib::structs::discord::DiscordWebhookBody;
use shared_lib::structs::sonarr::{SonarrEventType, SonarrGroupKey, SonarrRequestBody};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, Instant};

#[derive(Default)]
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
    pub async fn handle(&self, request_path: String, body: Value) -> Response<Body> {
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
        sonarr_request.event_type = Some(event_type);

        if !(event_type == SonarrEventType::Grab
            || event_type == SonarrEventType::Download
            || event_type == SonarrEventType::Upgrade)
        {
            // if the event type is not Download or Upgrade, return a 400
            return (
                StatusCode::BAD_REQUEST,
                Json(&"Unsupported event type, must be Grab or Download"),
            )
                .into_response();
        }

        for episode in &sonarr_request.episodes {
            tracing::info!(
                "[Recieved] {:?} Episode: {} - {:02}x{:02}",
                event_type,
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
                tracing::info!("[Timer] new timer started for {}", request_path);
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

        (StatusCode::OK, Json(&"Request added to queue")).into_response()
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
                tracing::info!(
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

    if let Some(queue) = timer_state_queue {
        process_timer_queue(
            format!("{}{}", crate::env::get_destination_url(), request_path),
            queue,
        )
        .await;
    }
}

async fn process_timer_queue(destination: String, queue: Vec<SonarrRequestBody>) {
    let mut queue = queue;
    let webhook_bodies = group_sonarr_requests(&mut queue)
        .values()
        .map(DiscordWebhookBody::from)
        .collect::<Vec<DiscordWebhookBody>>();

    for body in webhook_bodies {
        let _ = shared_lib::send::send_post_request(destination.clone(), body).await;
        sleep(Duration::from_secs(1)).await;
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
            sonarr_request.event_type = Some(event_type); // save it back to the request

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

    grouped_requests
}
