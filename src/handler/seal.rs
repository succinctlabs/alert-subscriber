use std::time::Duration;

use tracing::Level;

use crate::handler::{AlertFuture, AlertHandler};
use crate::AlertLayer;

/// The default deduplication time for Seal alerts.
///
/// Every 30 minutes, a new alert is sent.
const SEAL_DEFAULT_DEDUP_TIME: Duration = Duration::from_secs(60 * 30);

/// The default buffer size for Seal alerts.
const SEAL_DEFAULT_BUFFER: usize = 100;

/// Creates a new `AlertLayer` for sending Seal alerts.
///
/// Reads from the following environment variables:
///
/// - `SEAL_URL`: The URL of the Seal API.
/// - `SEAL_BEARER_TOKEN`: The Bearer token for the Seal API.
/// * `SEAL_DEDUP_TIME`: Optional deduplication time.
/// * `SEAL_BUFFER`: Optional buffer size for events.
///
/// # Panics:
/// - If any of the environment variables are not set.
/// - If this is called outside of a tokio runtime.
pub fn seal_layer() -> AlertLayer {
    // Get required environment variables.
    let url = std::env::var("SEAL_URL").expect("SEAL_URL is not set");
    let bearer_token = std::env::var("SEAL_BEARER_TOKEN").expect("SEAL_BEARER_TOKEN is not set");

    // Get optional environment variables.
    let dedup_time = match std::env::var("SEAL_DEDUP_TIME") {
        Ok(v) => v
            .parse::<u64>()
            .inspect_err(|e| {
                eprintln!(
                    "SEAL_DEDUP_TIME is not a valid duration: {}, using default (30 mins)",
                    e
                )
            })
            .map(Duration::from_secs)
            .unwrap_or(SEAL_DEFAULT_DEDUP_TIME),
        Err(_) => SEAL_DEFAULT_DEDUP_TIME,
    };

    let buffer = match std::env::var("SEAL_BUFFER") {
        Ok(v) => v
            .parse::<usize>()
            .inspect_err(|e| {
                eprintln!(
                    "SEAL_BUFFER is not a valid usize: {}, using default (100)",
                    e
                )
            })
            .unwrap_or(SEAL_DEFAULT_BUFFER),
        Err(_) => SEAL_DEFAULT_BUFFER,
    };

    let handler = SealHandler {
        client: reqwest::Client::new(),
        url,
        bearer_token,
    };

    AlertLayer::new(handler, Level::WARN, dedup_time, buffer)
}

pub struct SealHandler {
    client: reqwest::Client,
    url: String,
    bearer_token: String,
}

impl AlertHandler for SealHandler {
    type Error = reqwest::Error;

    fn handle_alert(&self, level: &tracing::Level, msg: &str) -> impl AlertFuture<Self::Error> {
        let formatted_msg = format!("{}: {}", level, msg);

        async move {
            self.client
                .post(&self.url)
                .bearer_auth(&self.bearer_token)
                .body(formatted_msg)
                .send()
                .await?
                .error_for_status()?;

            Ok(())
        }
    }
}
