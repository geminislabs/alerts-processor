use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::AppConfig;
use crate::domain::IncomingEvent;

/// Builds an async Kafka `StreamConsumer` with SASL/SCRAM authentication.
pub fn build_consumer(config: &AppConfig) -> Result<StreamConsumer> {
    build_consumer_for_topic(config, &config.kafka_topic, &config.kafka_group_id)
}

/// Builds an async Kafka `StreamConsumer` for an explicit topic/group pair.
pub fn build_consumer_for_topic(config: &AppConfig, topic: &str, group_id: &str) -> Result<StreamConsumer> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.kafka_brokers)
        .set("group.id", group_id)
        .set("security.protocol", &config.kafka_security_protocol)
        .set("sasl.mechanisms", &config.kafka_sasl_mechanism)
        .set("sasl.username", &config.kafka_sasl_username)
        .set("sasl.password", &config.kafka_sasl_password)
        // Low-latency fetch settings to avoid waiting for larger broker batches.
        .set("fetch.wait.max.ms", &config.kafka_fetch_wait_max_ms)
        .set("fetch.min.bytes", &config.kafka_fetch_min_bytes)
        // Reduce rebalance/failover delay when another consumer with the same group.id disappears.
        .set("session.timeout.ms", &config.kafka_session_timeout_ms)
        .set("heartbeat.interval.ms", &config.kafka_heartbeat_interval_ms)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", &config.kafka_auto_offset_reset)
        .create()
        .context("failed to create Kafka consumer")?;

    consumer
        .subscribe(&[topic])
        .context("failed to subscribe to Kafka topic")?;

    Ok(consumer)
}

/// Attempts to deserialize raw Kafka message bytes into an `IncomingEvent`.
///
/// The pipeline supports two wire formats:
///   1. A direct JSON object:  `{"event_id": ...}`
///   2. A JSON-encoded string: `"{\"event_id\": ...}"` (double-serialized)
pub fn parse_message(msg: &rdkafka::message::BorrowedMessage<'_>) -> Result<IncomingEvent> {
    parse_payload(msg)
}

/// Attempts to deserialize Kafka payload bytes into a JSON value.
/// Supports direct JSON and double-serialized JSON string payloads.
pub fn parse_json_value(msg: &rdkafka::message::BorrowedMessage<'_>) -> Result<Value> {
    parse_payload(msg)
}

fn parse_payload<T: DeserializeOwned>(msg: &rdkafka::message::BorrowedMessage<'_>) -> Result<T> {
    let payload = msg.payload().context("empty Kafka message payload")?;

    // Try direct JSON object first.
    if let Ok(parsed) = serde_json::from_slice::<T>(payload) {
        return Ok(parsed);
    }

    // Fallback: payload is a JSON string wrapping another JSON object.
    let inner: String =
        serde_json::from_slice(payload).context("Kafka payload is neither a JSON object nor a JSON-encoded string")?;

    serde_json::from_str(&inner).context("failed to deserialize inner JSON string payload")
}
