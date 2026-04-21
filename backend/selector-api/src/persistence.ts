/**
 * Persistence for selector decisions.
 *
 * updateDecision does BOTH:
 *   - UPDATE opportunities with new status + rejection_reason (if any)
 *   - INSERT risk_events if the rejection severity is warning/critical
 *
 * Wrap in a single transaction so decision + risk_event land atomically.
 */

import type pg from "pg";
import type { Decision } from "./policy/engine.js";

export async function persistDecision(
  pool: pg.Pool,
  opportunityId: string,
  chainId: number,
  traceId: string,
  decision: Decision,
): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");

    const nextStatus = decision.kind === "accept" ? "validated" : "rejected";
    const rejectionReason = decision.kind === "reject" ? decision.reason : null;
    await client.query(
      `UPDATE opportunities
          SET status = $2,
              rejection_reason = $3,
              risk_score = COALESCE($4, risk_score),
              updated_at = NOW()
        WHERE id = $1 AND status IN ('detected','validated','scored')`,
      [opportunityId, nextStatus, rejectionReason, decision.score],
    );

    if (decision.kind === "reject" && (decision.severity === "warning" || decision.severity === "critical")) {
      const eventType = decision.reason === "blacklist_hit" ? "blacklist_hit"
        : decision.reason === "token_safety_circuit_open" ? "circuit_breaker"
        : "degradation";
      await client.query(
        `INSERT INTO risk_events (event_type, severity, source_service, payload, trace_id, opportunity_id)
         VALUES ($1, $2, 'selector-api', $3::jsonb, $4, $5)`,
        [eventType, decision.severity, JSON.stringify({
          reason: decision.reason, score: decision.score, chain_id: chainId,
        }), traceId, opportunityId],
      );
    }

    await client.query("COMMIT");
  } catch (err) {
    await client.query("ROLLBACK");
    throw err;
  } finally {
    client.release();
  }
}
