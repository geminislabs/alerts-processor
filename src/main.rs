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

    // ── Logo ──────────────────────────────────────────────────────────────────
    {
        const LOGO: &str = include_str!("../assets/geminis-labs-logo.txt");
        const GRAY: &str = "\x1b[38;2;180;180;180m";
        const WHITE: &str = "\x1b[97m";
        const RESET: &str = "\x1b[0m";

        println!();
        println!("\t\t{WHITE}GeminiLabs :: Alert Processor{RESET}");
        println!("{GRAY}────────────────────────────────────────────────────────────────{RESET}");
        println!();
        use std::io::Write;
        if let Err(e) = std::io::stdout().write_all(LOGO.as_bytes()) {
            tracing::warn!(error = %e, "failed to print startup logo");
        }
        println!();
        println!("{GRAY}────────────────────────────────────────────────────────────────{RESET}");
        println!("\t\t{GRAY}alert-processor • @geminislabs{RESET}");
        println!();
        println!();
    }

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

    info!("initializing components");

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

    info!("startup complete, processing events");

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
