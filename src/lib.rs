/// The `handler` module contains the `AlertHandler` trait and its implementations.
pub mod handler;
pub use crate::handler::{AlertHandler, *};

/// The `visitor` module contains the `AlertVisitor` struct.
pub mod visitor;
use crate::visitor::AlertVisitor;

use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
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
}

impl AlertLayer {
    pub fn new<H: AlertHandler>(handler: H, dedup_time: Duration, buffer: usize) -> Self {
        let (handler_tx, handler_rx) = mpsc::channel(buffer);
        let cache = Arc::new(Mutex::new(BTreeMap::new()));

        tokio::spawn(handle_alert_future(handler, handler_rx, cache.clone()));

        Self {
            handler_tx,
            dedup: cache,
            dedup_time,
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
        let mut visitor = AlertVisitor::new(*event.metadata().level());
        event.record(&mut visitor);

        if visitor.is_alert() {
            let dedup_key = visitor.dedup_key();

            let mut dedup = self.dedup.lock();
            match dedup.entry(dedup_key) {
                Entry::Vacant(entry) => {
                    entry.insert(Instant::now() + self.dedup_time);

                    if let Err(e) = self.handler_tx.blocking_send(visitor) {
                        eprintln!("error sending alert to handler: {:?}", e.to_string());
                    }
                }
                Entry::Occupied(_) => {
                    // do nothing, expired entries are already removed.
                }
            }
        }
    }
}

/// If the alert reaches passes the cache. check it should be unconditionally sent to the handler.
async fn handle_alert_future<H: AlertHandler>(
    handler: H,
    mut handler_rx: mpsc::Receiver<AlertVisitor>,
    dedup: Arc<Mutex<BTreeMap<[u8; 32], Instant>>>,
) {
    loop {
        tokio::select! {
            Some(visitor) = handler_rx.recv() => {
                let level = visitor.level();
                let msg = visitor.message();

                if let Err(e) = handler.handle_alert(level, msg).await {
                    eprintln!("error handling alert: {:?}", e.to_string());
                }
            }
            _ = tokio::time::sleep(DEDUP_TIME) => {
                let mut dedup = dedup.lock();
                AlertLayer::purge_expired_entries(&mut dedup);
            }
        }
    }
}
