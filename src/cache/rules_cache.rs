use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::domain::Rule;

/// In-memory cache that maps unit_id → list of applicable rules.
/// Lookup is O(1) by unit_id.
pub struct RulesCache {
    map: Arc<RwLock<HashMap<Uuid, Vec<Rule>>>>,
}

impl RulesCache {
    /// Build the cache from a flat list of rules.
    /// Each rule is cloned once per unit_id it covers.
    pub fn build(rules: Vec<Rule>) -> Self {
        let mut map: HashMap<Uuid, Vec<Rule>> = HashMap::new();

        for rule in rules {
            for &unit_id in &rule.unit_ids {
                map.entry(unit_id).or_default().push(rule.clone());
            }
        }

        Self {
            map: Arc::new(RwLock::new(map)),
        }
    }

    /// Returns a cloned list of rules that apply to the given unit.
    pub fn get(&self, unit_id: &Uuid) -> Vec<Rule> {
        self.map
            .read()
            .ok()
            .and_then(|map| map.get(unit_id).cloned())
            .unwrap_or_default()
    }

    /// Placeholder for future runtime updates from the rules updates topic.
    pub fn apply_update_message(&self, _payload: &serde_json::Value) {
        // Format is not defined yet; keep this method as an explicit extension point.
    }
}

impl Clone for RulesCache {
    fn clone(&self) -> Self {
        Self {
            map: Arc::clone(&self.map),
        }
    }
}
