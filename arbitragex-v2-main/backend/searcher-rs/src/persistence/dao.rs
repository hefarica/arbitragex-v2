use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::sed_engine::SedOpportunity;

pub struct SedOpportunityDao {
    pool: PgPool,
}

impl SedOpportunityDao {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_opportunity(&self, opp: &SedOpportunity) -> Result<Uuid, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO sed_opportunities (
                block_number, tx_trigger_hash, token_in, token_out,
                amount_in, expected_out, price_impact, phase, status,
                latency_detect_us, edge_node_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#
        )
        .bind(opp.block_number as i64)
        .bind(&opp.tx_trigger_hash)
        .bind(&opp.token_in)
        .bind(&opp.token_out)
        .bind(opp.amount_in)
        .bind(opp.expected_out)
        .bind(opp.price_impact)
        .bind(opp.phase as i16)
        .bind("PENDING")
        .bind(opp.latency_detect_us as i64)
        .bind(&opp.edge_node_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn update_phase(&self, id: Uuid, phase: u8, status: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE sed_opportunities SET phase = $1, status = $2 WHERE id = $3"
        )
        .bind(phase as i16)
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
