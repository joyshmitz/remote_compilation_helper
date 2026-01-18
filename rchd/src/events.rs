//! Event broadcast utilities for daemon telemetry and benchmarking updates.

#![allow(dead_code)] // Events are wired for upcoming beads.

use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::warn;

const DEFAULT_BUFFER: usize = 256;

/// Broadcast channel for daemon events (JSON lines).
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<String>,
}

impl EventBus {
    /// Create a new event bus with the provided buffer size.
    pub fn new(buffer: usize) -> Self {
        let buffer = buffer.max(1).max(DEFAULT_BUFFER);
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    /// Subscribe to the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }

    /// Emit a structured event with payload.
    pub fn emit<T: Serialize>(&self, event: &str, data: &T) {
        let payload = json!({
            "event": event,
            "data": data,
            "timestamp": Utc::now().to_rfc3339(),
        });
        match serde_json::to_string(&payload) {
            Ok(serialized) => {
                let _ = self.sender.send(serialized);
            }
            Err(err) => warn!("Failed to serialize event {}: {}", event, err),
        }
    }
}
