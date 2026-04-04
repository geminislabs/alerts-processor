use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    // Database
    pub database_host: String,
    pub database_name: String,
    pub database_user: String,
    pub database_password: String,

    // Kafka
    pub kafka_brokers: String,
    pub kafka_topic: String,
    pub kafka_group_id: String,
    pub kafka_alerts_topic: String,
    pub kafka_rules_updates_topic: String,
    pub kafka_rules_updates_group_id: String,
    pub kafka_sasl_username: String,
    pub kafka_sasl_password: String,
    pub kafka_sasl_mechanism: String,
    pub kafka_security_protocol: String,
    pub kafka_fetch_wait_max_ms: String,
    pub kafka_fetch_min_bytes: String,
    pub kafka_session_timeout_ms: String,
    pub kafka_heartbeat_interval_ms: String,
    pub kafka_auto_offset_reset: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            database_host: required("DATABASE_HOST")?,
            database_name: required("DATABASE_NAME")?,
            database_user: required("DATABASE_USER")?,
            database_password: required("DATABASE_PASSWORD")?,
            kafka_brokers: required("KAFKA_BROKERS")?,
            kafka_topic: required("KAFKA_TOPIC")?,
            kafka_group_id: required("KAFKA_GROUP_ID")?,
            kafka_alerts_topic: required("KAFKA_ALERTS_TOPIC")?,
            kafka_rules_updates_topic: required("KAFKA_RULES_UPDATES_TOPIC")?,
            kafka_rules_updates_group_id: required("KAFKA_RULES_UPDATES_GROUP_ID")?,
            kafka_sasl_username: required("KAFKA_SASL_USERNAME")?,
            kafka_sasl_password: required("KAFKA_SASL_PASSWORD")?,
            kafka_sasl_mechanism: required("KAFKA_SASL_MECHANISM")?,
            kafka_security_protocol: required("KAFKA_SECURITY_PROTOCOL")?,
            kafka_fetch_wait_max_ms: optional_with_default("KAFKA_FETCH_WAIT_MAX_MS", "100"),
            kafka_fetch_min_bytes: optional_with_default("KAFKA_FETCH_MIN_BYTES", "1"),
            kafka_session_timeout_ms: optional_with_default("KAFKA_SESSION_TIMEOUT_MS", "6000"),
            kafka_heartbeat_interval_ms: optional_with_default(
                "KAFKA_HEARTBEAT_INTERVAL_MS",
                "2000",
            ),
            kafka_auto_offset_reset: optional_with_default("KAFKA_AUTO_OFFSET_RESET", "latest"),
        })
    }

    /// Returns a PostgreSQL connection string built from the individual fields.
    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}/{}",
            self.database_user, self.database_password, self.database_host, self.database_name,
        )
    }
}

fn required(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}

fn optional_with_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
