use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// Flat row returned by the JOIN query combining alert_rules + alert_rule_units.
/// One row per (rule, unit) pair; unit_ids are aggregated afterwards.
#[derive(Debug, sqlx::FromRow)]
pub struct AlertRuleRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub rule_type: String,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
    /// Array aggregated in the SQL query (array_agg); may be empty for rules with no units.
    pub unit_ids: Vec<Uuid>,
}
