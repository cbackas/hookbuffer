use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use warp::http::HeaderMap;

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
    headers: HeaderMap,
    body: Value,
}

impl SonarrHandler {
    pub fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&self, url: String, headers: HeaderMap, body: Value) -> impl warp::Reply {
        let new_state: Option<TimerState>;

        {
            let mut timers = self.timers.lock().await;

            // Check if there is already a TimerState for this URL.
            if let Some(timer_state) = timers.get_mut(&url) {
                // If there is a TimerState, add this request to the queue and update the timer_end Instant.
                timer_state.queue.push(RequestData { headers, body });
                timer_state.timer_end = Instant::now() + Duration::from_secs(15);
                new_state = None;
            } else {
                // If there isn't a TimerState, create one with this request in the queue and a new timer_end Instant.
                let timer_state = TimerState {
                    queue: vec![RequestData { headers, body }],
                    timer_end: Instant::now() + Duration::from_secs(15), // start a new timer for 15 seconds
                    timer_id: 0,
                };
                new_state = Some(timer_state);
            }
        }

        if let Some(timer_state) = new_state {
            let mut timers = self.timers.lock().await;
            timers.insert(url.clone(), timer_state);
        }

        // Now that the request has been added to the queue and the timer_end Instant has been updated,
        // we need to start the timer if it's not already running.
        self.start_timer(url).await;

        // For now, just return a simple response. We'll modify this later to return a more useful response.
        warp::reply::json(&"Received Sonarr request")
    }

    async fn start_timer(&self, url: String) {
        let mut timers = self.timers.lock().await;

        if let Some(timer_state) = timers.get_mut(&url) {
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
                if let Some(timer_state) = timers.get_mut(&url) {
                    // Only proceed if the timer ID hasn't changed.
                    if timer_state.timer_id == timer_id {
                        // Process the queued requests here.
                        process_queued_requests(&mut timer_state.queue).await;
                    }
                }
            });
        }
    }
}

// This function will process the queued requests for a given URL.
// It groups up all the sonarr messages and forwards them to the destination with the correct formatting
async fn process_queued_requests(queue: &mut Vec<RequestData>) {
    while let Some(request_data) = queue.pop() {
        println!("Processing request with body: {:?}", request_data.body);
    }
}
