use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub rule_type: String,
    pub config: Value,
    pub unit_ids: Vec<Uuid>,
}
