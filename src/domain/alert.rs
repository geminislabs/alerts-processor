use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub unit_id: Uuid,
    pub unit_name: Option<String>,
    pub rule_id: Uuid,
    pub source_type: String,
    pub source_id: Option<String>,
    pub alert_type: String,
    pub alert_name: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}
