A [`tracing_subscriber::Layer`] that forwards alerts to a handler.

This is useful for sending alerts to an "alert handler", such as a Slack channel or PagerDuty.

Alerts are indicated by the presence of the `alert` field in the event, they're deduplicated
(either by message or explicitly by the `dedup_key` field) and filtered by the maximum level of allowed alerts.

# Example

```rust,ignore
let layer = AlertLayer::new(handler, Level::WARN, dedup_time, buffer);

 tracing_subscriber::registry()
    .with(fmt::layer().with_filter(EnvFilter::from_default_env()))
    .with(alert_layer)
    .init();

tracing::error!(
    alert = true,
    dedup_key = "critical_error" 
    "Critical error occured ... "
);
```