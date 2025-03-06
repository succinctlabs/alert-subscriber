use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(alert_subscriber::seal_layer())
        .init();

    tracing::error!(
        alert = true,
        "test message"
    );

    // Give time for the alert to be sent to Seal
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}