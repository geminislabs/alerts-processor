use std::collections::HashMap;

use anyhow::Result;
use sqlx::PgPool;

use crate::db::models::AlertRuleRow;
use crate::domain::Rule;

/// Loads all active rules from the DB, each with its associated unit_ids aggregated.
///
/// Uses the runtime query form (no `DATABASE_URL` needed at compile time).
pub async fn load_rules(pool: &PgPool) -> Result<Vec<Rule>> {
    // `type` is a reserved Rust keyword; aliased as `rule_type` in SQL.
    let rows = sqlx::query_as::<_, AlertRuleRow>(
        r#"
        SELECT
            ar.id,
            ar.organization_id,
            ar.name,
            ar.type           AS rule_type,
            ar.config,
            ar.updated_at,
            aru.unit_id,
            u.name            AS unit_name
        FROM public.alert_rules ar
                INNER JOIN public.organizations org ON org.id = ar.organization_id
        LEFT JOIN public.alert_rule_units aru ON aru.rule_id = ar.id
        LEFT JOIN public.units u ON u.id = aru.unit_id
        WHERE ar.is_active = true
                    AND org.status = 'ACTIVE'
        "#,
    )
    .fetch_all(pool)
    .await?;

    struct RuleAccumulator {
        id: uuid::Uuid,
        organization_id: uuid::Uuid,
        name: String,
        rule_type: String,
        config: serde_json::Value,
        updated_at: chrono::DateTime<chrono::Utc>,
        unit_ids: Vec<uuid::Uuid>,
        unit_names: HashMap<uuid::Uuid, String>,
    }

    let mut grouped: HashMap<uuid::Uuid, RuleAccumulator> = HashMap::new();
    for row in rows {
        let entry = grouped.entry(row.id).or_insert_with(|| RuleAccumulator {
            id: row.id,
            organization_id: row.organization_id,
            name: row.name.clone(),
            rule_type: row.rule_type.clone(),
            config: row.config.clone(),
            updated_at: row.updated_at,
            unit_ids: Vec::new(),
            unit_names: HashMap::new(),
        });

        if let Some(unit_id) = row.unit_id {
            entry.unit_ids.push(unit_id);

            if let Some(unit_name) = row.unit_name {
                entry.unit_names.insert(unit_id, unit_name);
            }
        }
    }

    let rules = grouped
        .into_values()
        .map(|acc| Rule {
            id: acc.id,
            organization_id: acc.organization_id,
            name: acc.name,
            rule_type: acc.rule_type,
            config: acc.config,
            unit_ids: acc.unit_ids,
            unit_names: acc.unit_names,
            updated_at: acc.updated_at,
        })
        .collect();

    Ok(rules)
}
