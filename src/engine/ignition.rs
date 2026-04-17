use uuid::Uuid;

use crate::domain::{Alert, IncomingEvent, Rule};
use crate::engine::evaluator::RuleEvaluator;

/// Fires when `event.event_type` matches the value of `rule.config["event"]`.
pub struct IgnitionEvaluator;

impl RuleEvaluator for IgnitionEvaluator {
    fn evaluate(&self, event: &IncomingEvent, rule: &Rule) -> Option<Alert> {
        let expected_event = rule.config.get("event")?.as_str()?.trim();
        let actual_event = event.event_type_or_empty().trim();

        if !expected_event.eq_ignore_ascii_case(actual_event) {
            return None;
        }

        let organization_id = event.organization_id.unwrap_or(rule.organization_id);
        let unit_id = event.effective_unit_id()?;

        Some(Alert {
            id: Uuid::new_v4(),
            organization_id,
            unit_id,
            rule_id: rule.id,
            source_type: "event".to_string(),
            source_id: Some(event.event_id.to_string()),
            alert_type: event.event_label(),
            alert_name: rule.name.clone(),
            payload: event.payload.clone(),
            occurred_at: event.occurred_at,
        })
    }
}
