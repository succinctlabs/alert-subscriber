//! A [`tracing_subscriber::Layer`] that forwards alerts to a handler.
//!
//! This is useful for sending alerts to a handler, such as a Slack channel or PagerDuty.
//!
//! Alerts are indicated by the presence of the `alert` field in the event, they're deduplicated
//! (either by message or explicitly by the `dedup_key` field) and filtered by the maximum level of allowed alerts.
//!
//! # Example
//! ```rust,ignore
//! let layer = AlertLayer::new(handler, Level::WARN, dedup_time, buffer);
//! 
//!  tracing_subscriber::registry()
//!     .with(fmt::layer().with_filter(EnvFilter::from_default_env()))
//!     .with(alert_layer)
//!     .init();
//! 
//! tracing::error!(alert = true, "Critical error occured ... ");
//! ```
/// The `handler` module contains the `AlertHandler` trait and its implementations.
pub mod handler;
pub use crate::handler::{AlertHandler, *};

/// The `visitor` module contains the `AlertVisitor` struct.
pub mod visitor;
use crate::visitor::AlertVisitor;

use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

use std::collections::{btree_map::Entry, BTreeMap};
use std::time::{Duration, Instant};

/// Dedup the cache every 60 seconds.
const DEDUP_TIME: Duration = Duration::from_secs(60);

pub struct AlertLayer {
    /// The channel to send alerts to the handler.
    handler_tx: mpsc::Sender<AlertVisitor>,
    /// A set of deduplication keys.
    dedup: Arc<Mutex<BTreeMap<[u8; 32], Instant>>>,
    /// The timeout for deduplication.
    dedup_time: Duration,
    /// The maximum level of allowed alerts.
    max_level: Level,
}

impl AlertLayer {
    pub fn new<H: AlertHandler>(
        handler: H,
        max_level: Level,
        dedup_time: Duration,
        buffer: usize,
    ) -> Self {
        let (handler_tx, handler_rx) = mpsc::channel(buffer);
        let cache = Arc::new(Mutex::new(BTreeMap::new()));

        tokio::spawn(handle_alert_future(handler, handler_rx, cache.clone()));

        Self {
            handler_tx,
            dedup: cache,
            dedup_time,
            max_level,
        }
    }

    fn purge_expired_entries(cache: &mut BTreeMap<[u8; 32], Instant>) {
        cache.retain(|_, expiration| Instant::now() < *expiration);
    }
}

impl<S> Layer<S> for AlertLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let level = event.metadata().level();
        if level > &self.max_level {
            return;
        }

        let mut visitor = AlertVisitor::new(*level);
        event.record(&mut visitor);

        if visitor.is_alert() {
            let dedup_key = visitor.dedup_key();

            let mut dedup = self.dedup.lock();
            match dedup.entry(dedup_key) {
                Entry::Vacant(entry) => {
                    entry.insert(Instant::now() + self.dedup_time);
                }
                Entry::Occupied(mut entry) => {
                    let now = Instant::now();

                    // If the alert previous alert dedup time has expired, send it to the handler.
                    if &now > entry.get() {
                        entry.insert(now + self.dedup_time);
                    } else {
                        return;
                    }
                }
            }

            tokio::spawn({
                let tx = self.handler_tx.clone();

                async move {
                    if let Err(e) = tx.send(visitor).await {
                        eprintln!("error sending alert to handler: {:?}", e.to_string());
                    }
                }
            });
        }
    }
}

/// If the alert reaches passes the cache. check it should be unconditionally sent to the handler.
async fn handle_alert_future<H: AlertHandler>(
    handler: H,
    mut handler_rx: mpsc::Receiver<AlertVisitor>,
    dedup: Arc<Mutex<BTreeMap<[u8; 32], Instant>>>,
) {
    let mut interval = tokio::time::interval(DEDUP_TIME);

    loop {
        tokio::select! {
            Some(visitor) = handler_rx.recv() => {
                let level = visitor.level();
                let msg = visitor.message();

                if let Err(e) = handler.handle_alert(level, msg).await {
                    eprintln!("error handling alert: {:?}", e.to_string());
                }
            }
            _ = interval.tick() => {
                let mut dedup = dedup.lock();
                AlertLayer::purge_expired_entries(&mut dedup);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::convert::Infallible;
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_expected_message() {
        use tokio::sync::mpsc;
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        struct TestHandler {
            tx: mpsc::Sender<String>,
        }

        impl AlertHandler for TestHandler {
            type Error = Infallible;

            fn handle_alert(&self, _: &Level, msg: &str) -> impl AlertFuture<Self::Error> {
                async move {
                    self.tx.send(msg.to_string()).await.unwrap();

                    Ok(())
                }
            }
        }

        let (tx, mut rx) = mpsc::channel(1);
        let handler = TestHandler { tx };

        let alert_layer = AlertLayer::new(handler, Level::WARN, Duration::from_secs(1), 100);

        tracing_subscriber::registry()
            .with(alert_layer)
            .init();

        tracing::error!(alert = true, "test message");

        let msg = rx.recv().await.unwrap();
        assert!(msg == "test message");
    }
}
