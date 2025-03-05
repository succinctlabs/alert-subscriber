use tracing::field::Field;
use tracing::Level;
use tracing_subscriber::field::Visit;

use std::collections::HashMap;
use std::fmt;

use sha2::{Sha256, Digest};

pub struct AlertVisitor {
    level: Level,
    is_alert: bool,
    string_fields: HashMap<&'static str, String>,
}

impl AlertVisitor {
    pub(crate) fn new(level: Level) -> Self {
        Self {
            level,
            is_alert: false,
            string_fields: HashMap::new(),
        }
    }

    /// Inserts a field into the alert.
    fn insert(&mut self, key: &'static str, value: String) {
        self.string_fields.insert(key, value);
    }

    /// Returns true if the alert is enabled.
    pub(crate) fn is_alert(&self) -> bool {
        self.is_alert || self.string_fields.contains_key("alert")
    }

    /// Returns the message for the alert.
    pub(crate) fn message(&self) -> &str {
        self.string_fields
            .get("message")
            .map(|s| s.as_str())
            .unwrap_or("alert missing message")
    }

    /// Returns the deduplication key for the alert.
    ///
    /// This is the value of the `alert_key` field, if it exists.
    /// Otherwise, its the hash of the message.
    pub(crate) fn dedup_key(&self) -> [u8; 32] {
        let raw_key = self
            .string_fields
            .get("dedup_key")
            .map(|s| s.as_str())
            .unwrap_or(self.message());

        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        hasher.finalize().into()
    }

    /// Returns the level of the alert.
    pub(crate) fn level(&self) -> &Level {
        &self.level
    }
}

impl Visit for AlertVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "alert" {
            self.is_alert = value;
        }

        self.insert(field.name(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.insert(field.name(), format!("{:?}", value));
    }
}
