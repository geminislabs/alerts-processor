use anyhow::Result;
use sqlx::PgPool;

use crate::db::models::AlertRuleRow;
use crate::domain::Rule;

/// Loads all active rules from the DB, each with its associated unit_ids aggregated.
///
/// Uses the runtime query form (no `DATABASE_URL` needed at compile time).
pub async fn load_rules(pool: &PgPool) -> Result<Vec<Rule>> {
    // `type` is a reserved Rust keyword; aliased as `rule_type` in SQL.
    // COALESCE ensures array_agg never returns NULL for rules with no units.
    let rows = sqlx::query_as::<_, AlertRuleRow>(
        r#"
        SELECT
            ar.id,
            ar.organization_id,
            ar.type           AS rule_type,
            ar.config,
            COALESCE(
                array_agg(aru.unit_id) FILTER (WHERE aru.unit_id IS NOT NULL),
                '{}'
            )::uuid[]         AS unit_ids
        FROM public.alert_rules ar
                INNER JOIN public.organizations org ON org.id = ar.organization_id
        LEFT JOIN public.alert_rule_units aru ON aru.rule_id = ar.id
        WHERE ar.is_active = true
                    AND org.status = 'ACTIVE'
        GROUP BY ar.id, ar.organization_id, ar.type, ar.config
        "#,
    )
    .fetch_all(pool)
    .await?;

    let rules = rows
        .into_iter()
        .map(|r| Rule {
            id: r.id,
            organization_id: r.organization_id,
            rule_type: r.rule_type,
            config: r.config,
            unit_ids: r.unit_ids,
        })
        .collect();

    Ok(rules)
}
