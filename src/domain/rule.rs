use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub rule_type: String,
    pub config: Value,
    pub unit_ids: Vec<Uuid>,
    pub unit_names: HashMap<Uuid, String>,
    pub updated_at: DateTime<Utc>,
}
