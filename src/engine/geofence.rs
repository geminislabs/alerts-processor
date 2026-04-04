use crate::domain::{Alert, IncomingEvent, Rule};
use crate::engine::evaluator::RuleEvaluator;

/// Placeholder for a future geofence evaluator.
/// Currently always returns `None` (no alert fired).
pub struct GeofenceEvaluator;

impl RuleEvaluator for GeofenceEvaluator {
    fn evaluate(&self, _event: &IncomingEvent, _rule: &Rule) -> Option<Alert> {
        // TODO: implement geofence logic
        None
    }
}
