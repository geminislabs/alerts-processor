mod app;
mod cache;
mod config;
mod db;
mod domain;
mod engine;
mod kafka;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

use app::Processor;
use cache::RulesCache;
use engine::EvaluatorRegistry;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Logging ──────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // ── Config ────────────────────────────────────────────────────────────────
    let config = config::AppConfig::from_env()?;
    info!(
        brokers = %config.kafka_brokers,
        events_topic = %config.kafka_topic,
        events_group_id = %config.kafka_group_id,
        alerts_topic = %config.kafka_alerts_topic,
        rules_updates_topic = %config.kafka_rules_updates_topic,
        rules_updates_group_id = %config.kafka_rules_updates_group_id,
        auto_offset_reset = %config.kafka_auto_offset_reset,
        db_host = %config.database_host,
        "alert-processor starting"
    );

    // ── Database ──────────────────────────────────────────────────────────────
    let pool = db::create_pool(&config).await?;
    info!("connected to PostgreSQL");

    // ── Rules cache ───────────────────────────────────────────────────────────
    let rules = db::load_rules(&pool).await?;
    info!(count = rules.len(), "rules loaded");

    let cache = RulesCache::build(rules);

    // ── Engine ────────────────────────────────────────────────────────────────
    let registry = EvaluatorRegistry::new();

    // ── Kafka ─────────────────────────────────────────────────────────────────
    let events_consumer = kafka::build_consumer(&config)?;
    let rules_updates_consumer = kafka::build_consumer_for_topic(
        &config,
        &config.kafka_rules_updates_topic,
        &config.kafka_rules_updates_group_id,
    )?;
    let producer = kafka::build_producer(&config)?;
    info!("kafka consumers and producer ready");

    // ── Run ───────────────────────────────────────────────────────────────────
    let processor = Processor::new(
        events_consumer,
        rules_updates_consumer,
        producer,
        config.kafka_alerts_topic.clone(),
        cache,
        registry,
    );
    processor.run().await;

    Ok(())
}
