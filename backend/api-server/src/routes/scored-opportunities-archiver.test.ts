/**
 * scored-opportunities-archiver tests — pure schema assertions (no Redis, no PG).
 *
 * ARBX-RDY-02 (A.8 scoring wiring): the Rust emitter now labels every Gate C
 * record with `emission_outcome` ("accepted" | "rejected") + `rejection_reason`
 * (null on accepted). The archiver must parse BOTH the new shape AND the
 * pre-RDY-02 stream-backlog shape (fields absent) — the consumer group replays
 * old entries after a deploy.
 */
import { describe, it, expect } from "vitest";
import { ScoredRecordSchema } from "./scored-opportunities-archiver.js";

/** Pre-RDY-02 record shape (exactly what the Rust emitter XADDed before). */
const baseRecord = {
  opportunity_id: "0e0b0a6e-1234-4000-8000-000000000001",
  strategy_key: "MEV-01-001",
  token_pair: "WETH/USDC",
  posterior_prob: 0.51,
  kelly_fraction: 0.02,
  recommended_usd: 120.5,
  net_profit_usd: 1.5,
  bayesian_accepted: true,
  prior_log_odds: 0.0,
  chain_id: 1,
  source_context: "flat_prior",
  scoring_mode: "paper",
  evidence_vector: null,
};

describe("ScoredRecordSchema (ARBX-RDY-02)", () => {
  it("parses a record WITH the new fields (rejected class, verbatim reason)", () => {
    const rec = ScoredRecordSchema.parse({
      ...baseRecord,
      net_profit_usd: null, // rejected path: both profit fields None ⇒ null (R8)
      emission_outcome: "rejected",
      rejection_reason: "NegativeNetProfit:gas_floor_breach",
    });
    expect(rec.emission_outcome).toBe("rejected");
    expect(rec.rejection_reason).toBe("NegativeNetProfit:gas_floor_breach");
    expect(rec.net_profit_usd).toBeNull();
  });

  it("parses a record WITH the new fields (accepted class, explicit null reason)", () => {
    const rec = ScoredRecordSchema.parse({
      ...baseRecord,
      emission_outcome: "accepted",
      rejection_reason: null,
    });
    expect(rec.emission_outcome).toBe("accepted");
    expect(rec.rejection_reason).toBeNull();
  });

  it("parses an OLD record WITHOUT the new fields (stream backlog compat)", () => {
    const rec = ScoredRecordSchema.parse(baseRecord);
    expect(rec.emission_outcome).toBeUndefined();
    expect(rec.rejection_reason).toBeUndefined();
    // Core fields unaffected.
    expect(rec.opportunity_id).toBe(baseRecord.opportunity_id);
    expect(rec.scoring_mode).toBe("paper");
  });
});
