use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::Rule;

/// In-memory cache that maps unit_id → list of applicable rules.
/// Lookup is O(1) by unit_id.
pub struct RulesCache {
    state: Arc<RwLock<CacheState>>,
}

struct CacheState {
    by_unit: HashMap<Uuid, Vec<Rule>>,
    by_rule: HashMap<Uuid, RuleVersion>,
}

#[derive(Debug, Clone)]
struct RuleVersion {
    updated_at: DateTime<Utc>,
    unit_ids: Vec<Uuid>,
    is_deleted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "UPPERCASE")]
enum RulesUpdateMessage {
    Upsert {
        rule: RuleUpsertPayload,
    },
    Delete {
        rule_id: Uuid,
        updated_at: DateTime<Utc>,
    },
}

#[derive(Debug, Deserialize)]
struct RuleUpsertPayload {
    id: Uuid,
    organization_id: Uuid,
    #[serde(rename = "type")]
    rule_type: String,
    config: serde_json::Value,
    unit_ids: Vec<Uuid>,
    is_active: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum RuleUpdateOutcome {
    AppliedUpsert {
        rule_id: Uuid,
        unit_ids_count: usize,
    },
    AppliedDelete {
        rule_id: Uuid,
    },
    SkippedStale {
        rule_id: Uuid,
        incoming_updated_at: DateTime<Utc>,
        current_updated_at: DateTime<Utc>,
    },
}

#[derive(Debug, Error)]
pub enum RulesUpdateError {
    #[error("invalid rules update payload: {0}")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("rules cache lock poisoned")]
    LockPoisoned,
}

impl RulesCache {
    /// Build the cache from a flat list of rules.
    /// Each rule is cloned once per unit_id it covers.
    pub fn build(rules: Vec<Rule>) -> Self {
        let mut by_unit: HashMap<Uuid, Vec<Rule>> = HashMap::new();
        let mut by_rule: HashMap<Uuid, RuleVersion> = HashMap::new();

        for rule in rules {
            for &unit_id in &rule.unit_ids {
                by_unit.entry(unit_id).or_default().push(rule.clone());
            }

            by_rule.insert(
                rule.id,
                RuleVersion {
                    updated_at: rule.updated_at,
                    unit_ids: rule.unit_ids.clone(),
                    is_deleted: false,
                },
            );
        }

        Self {
            state: Arc::new(RwLock::new(CacheState { by_unit, by_rule })),
        }
    }

    /// Returns a cloned list of rules that apply to the given unit.
    pub fn get(&self, unit_id: &Uuid) -> Vec<Rule> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.by_unit.get(unit_id).cloned())
            .unwrap_or_default()
    }

    /// Applies a rules update message from Kafka.
    pub fn apply_update_message(
        &self,
        payload: &serde_json::Value,
    ) -> Result<RuleUpdateOutcome, RulesUpdateError> {
        let message: RulesUpdateMessage = serde_json::from_value(payload.clone())?;

        let mut state = self
            .state
            .write()
            .map_err(|_| RulesUpdateError::LockPoisoned)?;

        let outcome = match message {
            RulesUpdateMessage::Upsert { rule } => {
                if !rule.is_active {
                    apply_delete(&mut state, rule.id, rule.updated_at)
                } else {
                    apply_upsert(&mut state, rule)
                }
            }
            RulesUpdateMessage::Delete {
                rule_id,
                updated_at,
            } => apply_delete(&mut state, rule_id, updated_at),
        };

        Ok(outcome)
    }
}

impl Clone for RulesCache {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

fn apply_upsert(state: &mut CacheState, incoming: RuleUpsertPayload) -> RuleUpdateOutcome {
    if let Some(current) = state.by_rule.get(&incoming.id) {
        if current.updated_at > incoming.updated_at {
            return RuleUpdateOutcome::SkippedStale {
                rule_id: incoming.id,
                incoming_updated_at: incoming.updated_at,
                current_updated_at: current.updated_at,
            };
        }
    }

    if let Some(current) = state.by_rule.get(&incoming.id) {
        if !current.is_deleted {
            let old_unit_ids = current.unit_ids.clone();
            remove_rule_from_units(state, incoming.id, &old_unit_ids);
        }
    }

    let rule = Rule {
        id: incoming.id,
        organization_id: incoming.organization_id,
        rule_type: incoming.rule_type,
        config: incoming.config,
        unit_ids: incoming.unit_ids.clone(),
        updated_at: incoming.updated_at,
    };

    for unit_id in &rule.unit_ids {
        state
            .by_unit
            .entry(*unit_id)
            .or_default()
            .push(rule.clone());
    }

    state.by_rule.insert(
        rule.id,
        RuleVersion {
            updated_at: rule.updated_at,
            unit_ids: rule.unit_ids.clone(),
            is_deleted: false,
        },
    );

    RuleUpdateOutcome::AppliedUpsert {
        rule_id: rule.id,
        unit_ids_count: rule.unit_ids.len(),
    }
}

fn apply_delete(
    state: &mut CacheState,
    rule_id: Uuid,
    updated_at: DateTime<Utc>,
) -> RuleUpdateOutcome {
    if let Some(current) = state.by_rule.get(&rule_id) {
        if current.updated_at > updated_at {
            return RuleUpdateOutcome::SkippedStale {
                rule_id,
                incoming_updated_at: updated_at,
                current_updated_at: current.updated_at,
            };
        }
    }

    if let Some(current) = state.by_rule.get(&rule_id) {
        if !current.is_deleted {
            let old_unit_ids = current.unit_ids.clone();
            remove_rule_from_units(state, rule_id, &old_unit_ids);
        }
    }

    state.by_rule.insert(
        rule_id,
        RuleVersion {
            updated_at,
            unit_ids: Vec::new(),
            is_deleted: true,
        },
    );

    RuleUpdateOutcome::AppliedDelete { rule_id }
}

fn remove_rule_from_units(state: &mut CacheState, rule_id: Uuid, unit_ids: &[Uuid]) {
    for unit_id in unit_ids {
        let should_remove_entry = if let Some(rules) = state.by_unit.get_mut(unit_id) {
            rules.retain(|rule| rule.id != rule_id);
            rules.is_empty()
        } else {
            false
        };

        if should_remove_entry {
            state.by_unit.remove(unit_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use uuid::Uuid;

    use super::{RuleUpdateOutcome, RulesCache};

    #[test]
    fn upsert_adds_new_rule() {
        let cache = RulesCache::build(vec![]);
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let payload = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine OFF"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T04:17:54.525884Z"
            }
        });

        let outcome = cache.apply_update_message(&payload).expect("valid upsert");
        assert!(matches!(
            outcome,
            RuleUpdateOutcome::AppliedUpsert { rule_id: id, .. } if id == rule_id
        ));

        let rules = cache.get(&unit_id);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, rule_id);
    }

    #[test]
    fn upsert_moves_rule_when_units_change() {
        let old_unit = Uuid::new_v4();
        let new_unit = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let first = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine OFF"},
                "unit_ids": [old_unit],
                "is_active": true,
                "updated_at": "2026-04-06T04:00:00Z"
            }
        });
        cache.apply_update_message(&first).expect("first upsert");

        let second = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [new_unit],
                "is_active": true,
                "updated_at": "2026-04-06T05:00:00Z"
            }
        });
        cache.apply_update_message(&second).expect("second upsert");

        assert!(cache.get(&old_unit).is_empty());
        let new_rules = cache.get(&new_unit);
        assert_eq!(new_rules.len(), 1);
        assert_eq!(new_rules[0].id, rule_id);
        assert_eq!(new_rules[0].config, json!({"event": "Engine ON"}));
    }

    #[test]
    fn stale_upsert_is_skipped() {
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let fresh = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T06:00:00Z"
            }
        });
        cache.apply_update_message(&fresh).expect("fresh upsert");

        let stale = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine OFF"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T05:00:00Z"
            }
        });

        let outcome = cache
            .apply_update_message(&stale)
            .expect("stale upsert processed");
        assert!(matches!(outcome, RuleUpdateOutcome::SkippedStale { .. }));

        let rules = cache.get(&unit_id);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].config, json!({"event": "Engine ON"}));
    }

    #[test]
    fn delete_removes_rule_from_cache() {
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let upsert = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T06:00:00Z"
            }
        });
        cache.apply_update_message(&upsert).expect("upsert");

        let delete = json!({
            "operation": "DELETE",
            "rule_id": rule_id,
            "updated_at": "2026-04-06T06:30:00Z"
        });
        let outcome = cache.apply_update_message(&delete).expect("delete");
        assert!(matches!(
            outcome,
            RuleUpdateOutcome::AppliedDelete { rule_id: id } if id == rule_id
        ));

        assert!(cache.get(&unit_id).is_empty());
    }

    #[test]
    fn stale_delete_is_skipped() {
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let upsert = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T08:00:00Z"
            }
        });
        cache.apply_update_message(&upsert).expect("upsert");

        let stale_delete = json!({
            "operation": "DELETE",
            "rule_id": rule_id,
            "updated_at": "2026-04-06T07:59:59Z"
        });
        let outcome = cache
            .apply_update_message(&stale_delete)
            .expect("stale delete processed");
        assert!(matches!(outcome, RuleUpdateOutcome::SkippedStale { .. }));

        assert_eq!(cache.get(&unit_id).len(), 1);
    }

    #[test]
    fn upsert_with_empty_units_is_not_indexed() {
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);
        let random_unit = Uuid::new_v4();

        let payload = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine OFF"},
                "unit_ids": [],
                "is_active": true,
                "updated_at": "2026-04-06T04:17:54.525884Z"
            }
        });

        let outcome = cache.apply_update_message(&payload).expect("valid upsert");
        assert!(matches!(
            outcome,
            RuleUpdateOutcome::AppliedUpsert {
                rule_id: id,
                unit_ids_count: 0
            } if id == rule_id
        ));
        assert!(cache.get(&random_unit).is_empty());
    }

    #[test]
    fn stale_upsert_after_delete_is_skipped_by_tombstone() {
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let delete = json!({
            "operation": "DELETE",
            "rule_id": rule_id,
            "updated_at": "2026-04-06T10:00:00Z"
        });
        cache.apply_update_message(&delete).expect("delete");

        let stale_upsert = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": "2026-04-06T09:00:00Z"
            }
        });

        let outcome = cache
            .apply_update_message(&stale_upsert)
            .expect("stale upsert processed");
        assert!(matches!(outcome, RuleUpdateOutcome::SkippedStale { .. }));
        assert!(cache.get(&unit_id).is_empty());
    }

    #[test]
    fn parses_business_upsert_example() {
        let cache = RulesCache::build(vec![]);
        let payload = json!({
            "operation": "UPSERT",
            "rule": {
                "id": "e94351f8-7f5e-4ba0-8262-b9bc44f1d939",
                "organization_id": "1de3e794-2555-4f77-9878-67fe2f934535",
                "name": "MyTest",
                "type": "ignition onnn2",
                "config": {"event": "Engine OFF"},
                "unit_ids": [],
                "is_active": true,
                "updated_at": "2026-04-06T04:17:54.525884Z"
            }
        });

        let outcome = cache
            .apply_update_message(&payload)
            .expect("business upsert");
        assert!(matches!(outcome, RuleUpdateOutcome::AppliedUpsert { .. }));
    }

    #[test]
    fn parses_business_delete_example() {
        let cache = RulesCache::build(vec![]);
        let payload = json!({
            "operation": "DELETE",
            "rule_id": "3b6afa2b-0f8d-4ef2-bdbf-bb20c8af9ae6",
            "updated_at": "2026-04-05T23:30:00Z"
        });

        let outcome = cache
            .apply_update_message(&payload)
            .expect("business delete");
        assert!(matches!(outcome, RuleUpdateOutcome::AppliedDelete { .. }));
    }

    #[test]
    fn outcome_contains_expected_stale_timestamps() {
        let unit_id = Uuid::new_v4();
        let rule_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let cache = RulesCache::build(vec![]);

        let fresh_ts = DateTime::parse_from_rfc3339("2026-04-06T08:00:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);
        let stale_ts = DateTime::parse_from_rfc3339("2026-04-06T07:00:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        let upsert_fresh = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine ON"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": fresh_ts.to_rfc3339()
            }
        });
        cache
            .apply_update_message(&upsert_fresh)
            .expect("fresh upsert should apply");

        let upsert_stale = json!({
            "operation": "UPSERT",
            "rule": {
                "id": rule_id,
                "organization_id": org_id,
                "type": "ignition",
                "config": {"event": "Engine OFF"},
                "unit_ids": [unit_id],
                "is_active": true,
                "updated_at": stale_ts.to_rfc3339()
            }
        });

        let outcome = cache
            .apply_update_message(&upsert_stale)
            .expect("stale upsert should be handled");

        match outcome {
            RuleUpdateOutcome::SkippedStale {
                rule_id: skipped_rule,
                incoming_updated_at,
                current_updated_at,
            } => {
                assert_eq!(skipped_rule, rule_id);
                assert_eq!(incoming_updated_at, stale_ts);
                assert_eq!(current_updated_at, fresh_ts);
            }
            _ => panic!("expected stale outcome"),
        }
    }
}
