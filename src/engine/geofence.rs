use std::collections::HashSet;

use serde_json::Value;
use uuid::Uuid;

use crate::domain::{Alert, IncomingEvent, Rule};
use crate::engine::evaluator::RuleEvaluator;

const GEOFENCE_ENTER_EVENT_TYPE_ID: &str = "64f9709b-8d4c-4b2e-b1ab-44b015527ba5";
const GEOFENCE_EXIT_EVENT_TYPE_ID: &str = "23bb5beb-be85-442d-ac5b-c26192b1f86a";

/// Fires when the event geofence and transition match `rule.config`.
///
/// Supported config:
/// {
///   "geofences": ["<geofence_uuid>", ...],
///   "transitions": ["enter", "exit"]
/// }
///
/// `transitions` also accepts raw `event_type_id` UUID values.
pub struct GeofenceEvaluator;

impl RuleEvaluator for GeofenceEvaluator {
    fn evaluate(&self, event: &IncomingEvent, rule: &Rule) -> Option<Alert> {
        let unit_id = event.effective_unit_id()?;
        let geofence_id = parse_event_geofence_id(event)?;
        let configured_geofences = parse_uuid_list(rule.config.get("geofences"))?;

        if !configured_geofences.contains(&geofence_id) {
            return None;
        }

        let transitions = parse_string_set(rule.config.get("transitions"))?;
        if transitions.is_empty() {
            return None;
        }

        let event_type_id = event.event_type_id.map(|value| value.to_string());
        if !transition_matches(&transitions, event_type_id.as_deref()) {
            return None;
        }

        let organization_id = event.organization_id.unwrap_or(rule.organization_id);
        let unit_name = rule.unit_names.get(&unit_id).cloned();

        Some(Alert {
            id: Uuid::new_v4(),
            organization_id,
            unit_id,
            unit_name,
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

fn parse_uuid_list(value: Option<&Value>) -> Option<HashSet<Uuid>> {
    let items = value?.as_array()?;
    let mut out = HashSet::with_capacity(items.len());

    for item in items {
        let as_str = item.as_str()?.trim();
        let parsed = Uuid::parse_str(as_str).ok()?;
        out.insert(parsed);
    }

    Some(out)
}

fn parse_string_set(value: Option<&Value>) -> Option<HashSet<String>> {
    let items = value?.as_array()?;
    let mut out = HashSet::with_capacity(items.len());

    for item in items {
        let as_str = item.as_str()?.trim();
        if as_str.is_empty() {
            continue;
        }

        out.insert(as_str.to_ascii_lowercase());
    }

    Some(out)
}

fn parse_event_geofence_id(event: &IncomingEvent) -> Option<Uuid> {
    let geofence_id = event.payload.get("geofence_id")?.as_str()?.trim();
    Uuid::parse_str(geofence_id).ok()
}

fn transition_matches(transitions: &HashSet<String>, event_type_id: Option<&str>) -> bool {
    let Some(event_type_id) = event_type_id else {
        return false;
    };

    let normalized = event_type_id.trim().to_ascii_lowercase();

    if transitions.contains(&normalized) {
        return true;
    }

    (transitions.contains("enter") && normalized == GEOFENCE_ENTER_EVENT_TYPE_ID)
        || (transitions.contains("exit") && normalized == GEOFENCE_EXIT_EVENT_TYPE_ID)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    use crate::domain::{IncomingEvent, Rule};
    use crate::engine::evaluator::RuleEvaluator;

    use super::GeofenceEvaluator;

    #[test]
    fn fires_alert_for_matching_geofence_enter_transition() {
        let evaluator = GeofenceEvaluator;
        let event = build_geofence_event(
            "64f9709b-8d4c-4b2e-b1ab-44b015527ba5",
            "550e8400-e29b-41d4-a716-446655440001",
        );
        let rule = build_rule(json!({
            "geofences": ["550e8400-e29b-41d4-a716-446655440001"],
            "transitions": ["enter", "exit"]
        }));

        let alert = evaluator.evaluate(&event, &rule);
        assert!(alert.is_some());
    }

    #[test]
    fn does_not_fire_for_non_matching_geofence() {
        let evaluator = GeofenceEvaluator;
        let event = build_geofence_event(
            "64f9709b-8d4c-4b2e-b1ab-44b015527ba5",
            "550e8400-e29b-41d4-a716-446655440001",
        );
        let rule = build_rule(json!({
            "geofences": ["97003dd7-b9c2-4b76-b681-01842c9c0c7e"],
            "transitions": ["enter"]
        }));

        let alert = evaluator.evaluate(&event, &rule);
        assert!(alert.is_none());
    }

    #[test]
    fn supports_direct_event_type_id_transition_matching() {
        let evaluator = GeofenceEvaluator;
        let event = build_geofence_event(
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "550e8400-e29b-41d4-a716-446655440001",
        );
        let rule = build_rule(json!({
            "geofences": ["550e8400-e29b-41d4-a716-446655440001"],
            "transitions": ["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]
        }));

        let alert = evaluator.evaluate(&event, &rule);
        assert!(alert.is_some());
    }

    fn build_geofence_event(event_type_id: &str, geofence_id: &str) -> IncomingEvent {
        serde_json::from_value(json!({
            "id": "aaaabbbb-cccc-dddd-eeee-ffff11112222",
            "unit_id": "99999999-9999-9999-9999-999999999999",
            "event_type_id": event_type_id,
            "payload": {
                "geofence_id": geofence_id
            },
            "occurred_at": "2026-04-16T14:40:10Z"
        }))
        .expect("valid event")
    }

    fn build_rule(config: serde_json::Value) -> Rule {
        Rule {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            name: "Regla Geocerca".to_string(),
            rule_type: "geofence".to_string(),
            config,
            unit_ids: vec![],
            unit_names: HashMap::new(),
            updated_at: DateTime::parse_from_rfc3339("2026-04-16T14:40:10Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        }
    }
}
