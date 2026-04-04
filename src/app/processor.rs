use rdkafka::producer::FutureProducer;
use rdkafka::consumer::StreamConsumer;
use rdkafka::message::Message;
use rdkafka::message::Timestamp;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

use crate::cache::RulesCache;
use crate::engine::EvaluatorRegistry;
use crate::kafka::{parse_json_value, parse_message, publish_alert};

pub struct Processor {
    events_consumer: StreamConsumer,
    rules_updates_consumer: StreamConsumer,
    producer: FutureProducer,
    alerts_topic: String,
    cache: RulesCache,
    registry: EvaluatorRegistry,
}

impl Processor {
    pub fn new(
        events_consumer: StreamConsumer,
        rules_updates_consumer: StreamConsumer,
        producer: FutureProducer,
        alerts_topic: String,
        cache: RulesCache,
        registry: EvaluatorRegistry,
    ) -> Self {
        Self {
            events_consumer,
            rules_updates_consumer,
            producer,
            alerts_topic,
            cache,
            registry,
        }
    }

    /// Main processing loop. Runs until an unrecoverable error occurs or the process is interrupted.
    pub async fn run(self) {
        info!("processor started, waiting for events and rules updates");

        let events_consumer = self.events_consumer;
        let rules_updates_consumer = self.rules_updates_consumer;
        let producer = self.producer;
        let alerts_topic = self.alerts_topic;
        let cache_for_events = self.cache.clone();
        let cache_for_updates = self.cache;
        let registry = self.registry;

        let process_events = async move {
            loop {
                match timeout(Duration::from_secs(30), events_consumer.recv()).await {
                    Err(_) => {
                        info!("still waiting for Kafka messages (no event messages in last 30s)");
                    }
                    Ok(Err(e)) => {
                        error!(error = %e, "kafka recv error on events consumer");
                    }
                    Ok(Ok(msg)) => {
                        info!(
                            topic = msg.topic(),
                            partition = msg.partition(),
                            offset = msg.offset(),
                            payload_len = msg.payload().map(|p| p.len()),
                            "kafka event message received"
                        );

                        let event = match parse_message(&msg) {
                            Ok(e) => e,
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    partition = msg.partition(),
                                    offset = msg.offset(),
                                    "failed to parse event message, skipping"
                                );
                                continue;
                            }
                        };

                        let unit_id = event.effective_unit_id();
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        let kafka_lag_ms = match msg.timestamp() {
                            Timestamp::CreateTime(ts_ms) | Timestamp::LogAppendTime(ts_ms) => {
                                Some(now_ms.saturating_sub(ts_ms))
                            }
                            Timestamp::NotAvailable => None,
                        };
                        let received_lag_ms = now_ms.saturating_sub(event.received_at.timestamp_millis());
                        let occurred_lag_ms = now_ms.saturating_sub(event.occurred_at.timestamp_millis());

                        let rules = cache_for_events.get(&unit_id);

                        if rules.is_empty() {
                            warn!(
                                event_id = %event.event_id,
                                event_type = %event.event_type,
                                unit_id = %unit_id,
                                "event skipped: no rules configured for unit"
                            );
                            continue;
                        }

                        for rule in rules {
                            if let Some(evaluator) = registry.get(&rule.rule_type) {
                                if let Some(alert) = evaluator.evaluate(&event, &rule) {
                                    match publish_alert(&producer, &alerts_topic, &alert).await {
                                        Ok(()) => {
                                            info!(
                                                alert_id = %alert.id,
                                                event_id = %event.event_id,
                                                rule_id = %rule.id,
                                                rule_type = %rule.rule_type,
                                                unit_id = %alert.unit_id,
                                                organization = %alert.organization_id,
                                                occurred_at = %alert.occurred_at,
                                                kafka_lag_ms = ?kafka_lag_ms,
                                                received_lag_ms,
                                                occurred_lag_ms,
                                                alerts_topic = %alerts_topic,
                                                "alert generated and published"
                                            );
                                        }
                                        Err(e) => {
                                            error!(
                                                error = %e,
                                                alert_id = %alert.id,
                                                event_id = %event.event_id,
                                                rule_id = %rule.id,
                                                unit_id = %alert.unit_id,
                                                alerts_topic = %alerts_topic,
                                                "failed to publish generated alert"
                                            );
                                        }
                                    }
                                }
                            } else {
                                warn!(
                                    rule_id = %rule.id,
                                    rule_type = %rule.rule_type,
                                    "rule skipped: evaluator not registered for rule_type"
                                );
                            }
                        }
                    }
                }
            }
        };

        let process_rules_updates = async move {
            loop {
                match timeout(Duration::from_secs(30), rules_updates_consumer.recv()).await {
                    Err(_) => {
                        info!("still waiting for Kafka messages (no rules update messages in last 30s)");
                    }
                    Ok(Err(e)) => {
                        error!(error = %e, "kafka recv error on rules updates consumer");
                    }
                    Ok(Ok(msg)) => {
                        let payload = match parse_json_value(&msg) {
                            Ok(value) => value,
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    topic = msg.topic(),
                                    partition = msg.partition(),
                                    offset = msg.offset(),
                                    "failed to parse rules update message, skipping"
                                );
                                continue;
                            }
                        };

                        cache_for_updates.apply_update_message(&payload);
                        info!(
                            topic = msg.topic(),
                            partition = msg.partition(),
                            offset = msg.offset(),
                            "rules update message received (no-op until contract is defined)"
                        );
                    }
                }
            }
        };

        tokio::join!(process_events, process_rules_updates);
    }
}
