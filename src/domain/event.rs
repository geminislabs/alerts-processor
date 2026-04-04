use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct IncomingEvent {
    pub event_id: Uuid,
    #[serde(default)]
    pub organization_id: Option<Uuid>,
    #[serde(default)]
    pub unit_id: Option<Uuid>,
    pub schema_version: u32,
    pub event_type: String,
    pub source: EventSource,
    pub unit: EventUnit,
    pub source_epoch: i64,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub payload: Value,
}

impl IncomingEvent {
    pub fn effective_unit_id(&self) -> Uuid {
        self.unit_id.unwrap_or(self.unit.id)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct EventSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub id: String,
    pub message_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventUnit {
    pub id: Uuid,
}
