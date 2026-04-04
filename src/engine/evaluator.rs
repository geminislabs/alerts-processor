use crate::domain::{Alert, IncomingEvent, Rule};

/// Every rule evaluator must implement this trait.
/// Implementations are stateless; all context comes from the event and the rule.
pub trait RuleEvaluator: Send + Sync {
    /// Returns `Some(Alert)` if the rule triggered for this event, or `None` otherwise.
    fn evaluate(&self, event: &IncomingEvent, rule: &Rule) -> Option<Alert>;
}
