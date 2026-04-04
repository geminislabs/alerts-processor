use std::collections::HashMap;

use crate::engine::evaluator::RuleEvaluator;
use crate::engine::geofence::GeofenceEvaluator;
use crate::engine::ignition::IgnitionEvaluator;

/// Central registry that maps rule_type strings to their evaluator implementations.
pub struct EvaluatorRegistry {
    map: HashMap<String, Box<dyn RuleEvaluator>>,
}

impl EvaluatorRegistry {
    /// Creates the registry with all built-in evaluators registered.
    pub fn new() -> Self {
        let mut registry = Self {
            map: HashMap::new(),
        };

        registry.register("ignition_off", Box::new(IgnitionEvaluator));
        registry.register("geofence", Box::new(GeofenceEvaluator));

        registry
    }

    pub fn register(&mut self, rule_type: &str, evaluator: Box<dyn RuleEvaluator>) {
        self.map
            .insert(normalize_rule_type(rule_type), evaluator);
    }

    pub fn get(&self, rule_type: &str) -> Option<&dyn RuleEvaluator> {
        self.map
            .get(&normalize_rule_type(rule_type))
            .map(Box::as_ref)
    }

}

fn normalize_rule_type(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

impl Default for EvaluatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
