use std::time::Duration;

use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};

use crate::config::AppConfig;
use crate::domain::Alert;

/// Builds an async Kafka producer with the same auth settings used by consumers.
pub fn build_producer(config: &AppConfig) -> Result<FutureProducer> {
    ClientConfig::new()
        .set("bootstrap.servers", &config.kafka_brokers)
        .set("security.protocol", &config.kafka_security_protocol)
        .set("sasl.mechanisms", &config.kafka_sasl_mechanism)
        .set("sasl.username", &config.kafka_sasl_username)
        .set("sasl.password", &config.kafka_sasl_password)
        .create()
        .context("failed to create Kafka producer")
}

/// Serializes and publishes an alert JSON payload to the configured alerts topic.
pub async fn publish_alert(producer: &FutureProducer, topic: &str, alert: &Alert) -> Result<()> {
    let payload = serde_json::to_string(alert).context("failed to serialize alert payload")?;
    let key = alert.unit_id.to_string();

    let delivery = producer
        .send(
            FutureRecord::to(topic).payload(&payload).key(&key),
            Duration::from_secs(5),
        )
        .await;

    match delivery {
        Ok(_) => Ok(()),
        Err((err, _)) => Err(err).context("failed to publish alert to Kafka"),
    }
}
