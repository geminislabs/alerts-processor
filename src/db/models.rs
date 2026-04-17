use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// Flat row returned by the JOIN query combining alert_rules + alert_rule_units.
/// One row per (rule, unit) pair.
#[derive(Debug, sqlx::FromRow)]
pub struct AlertRuleRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub rule_type: String,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
    pub unit_id: Option<Uuid>,
    pub unit_name: Option<String>,
}
