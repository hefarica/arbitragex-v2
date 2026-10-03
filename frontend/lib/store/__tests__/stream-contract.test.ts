// frontend/lib/store/__tests__/stream-contract.test.ts
//
// STREAM-SEQ-01 (§11.4) — the sequencing/idempotency contract of the card
// stream. This is where the §11 correction actually lives, so it is pinned
// directly (no renderHook, no React):
//
//   · claves estables por estrategia y por plan
//   · un evento ANTIGUO no sobrescribe uno NUEVO
//   · un replay es idempotente (no se aplica dos veces)
//   · una actualización sobre la MISMA detección sí se aplica (migration-107)
//   · una fila sin fecha se acepta y se DECLARA (nunca se descarta en silencio)
//   · las piernas de un plan no se mezclan con el neto de otro (field groups)
//
// The fixtures are synthetic (allowed in tests) and ISOLATED from the stream
// and from the real metrics — nothing here writes to the store.
import { describe, expect, it } from "vitest";

import {
  compareObservationOrder,
  countOutcome,
  decideUpsert,
  EMPTY_LEDGER_COUNTERS,
  observationClockMs,
  parsePublisherProgress,
  parseStreamEnvelope,
  planDigestOf,
  planKeyOf,
  sequenceVerdict,
  strategyKeyOf,
  STREAM_SCHEMA_VERSION,
  UNDECLARED_SIZE,
  type StreamLedgerCounters,
} from "../stream-contract";
import { mapToOmniOpportunity, type OmniOpportunity } from "../types";

function row(over: Record<string, unknown> = {}): OmniOpportunity {
  return mapToOmniOpportunity({
    id: "opp-1",
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-10-03T10:00:00Z",
    status: "detected",
    trace_id: "trace-1",
    dex_a: "uniswap_v2",
    dex_b: "sushiswap",
    token_in: "0xaaa",
    token_out: "0xbbb",
    ...over,
  });
}

describe("stream-contract — stable keys per strategy and per plan", () => {
  it("strategyKeyOf is chain+strategy and is STABLE across re-detections", () => {
    const a = row({ id: "det-1", detected_at: "2026-10-03T10:00:00Z" });
    const b = row({ id: "det-2", detected_at: "2026-10-03T10:05:00Z" });
    expect(strategyKeyOf(a)).toBe("1|dex_arb");
    expect(strategyKeyOf(b)).toBe(strategyKeyOf(a));
  });

  it("strategyKeyOf degrades to a stable bucket for a malformed row (never throws)", () => {
    const malformed = row({ chain_id: null, strategy_kind: null });
    expect(strategyKeyOf(malformed)).toBe("|");
  });

  it("planKeyOf separates plans by route AND by declared notional (§11.4 field groups)", () => {
    const base = row({
      route_metadata: {
        token_addresses: ["0xaaa", "0xbbb"],
        pool_addresses: ["0xp1"],
        dex_adapters: ["uniswap_v2"],
        economics_amount_in_wei: "1000000",
      },
    });
    // Same route, different DECLARED size = a DIFFERENT plan. This is what stops
    // a net computed at size X from landing on legs quoted at size Y.
    const otherSize = row({
      route_metadata: {
        token_addresses: ["0xaaa", "0xbbb"],
        pool_addresses: ["0xp1"],
        dex_adapters: ["uniswap_v2"],
        economics_amount_in_wei: "2000000",
      },
    });
    // Different route, same size.
    const otherRoute = row({
      token_out: "0xccc",
      route_metadata: {
        token_addresses: ["0xaaa", "0xccc"],
        pool_addresses: ["0xp2"],
        dex_adapters: ["uniswap_v2"],
        economics_amount_in_wei: "1000000",
      },
    });
    expect(planKeyOf(base)).not.toBe(planKeyOf(otherSize));
    expect(planKeyOf(base)).not.toBe(planKeyOf(otherRoute));
    expect(planKeyOf(base)).toContain("1000000");
  });

  it("an undeclared notional is a STATE in the key, not a fabricated 0", () => {
    const key = planKeyOf(row());
    expect(key).toContain(`@${UNDECLARED_SIZE}`);
    expect(key).not.toContain("@0");
  });
});

describe("stream-contract — observation order (the fallback authority)", () => {
  it("uses the same vigency clock pruneStale evicts on: last_seen_at ?? detected_at", () => {
    const r = row({ detected_at: "2026-10-03T10:00:00Z" });
    expect(observationClockMs(r)).toBe(Date.parse("2026-10-03T10:00:00Z"));
    const grouped = row({
      detected_at: "2026-10-03T10:00:00Z",
      last_seen_at: "2026-10-03T10:07:00Z",
    });
    expect(observationClockMs(grouped)).toBe(Date.parse("2026-10-03T10:07:00Z"));
  });

  it("an undated row has NO clock (absence is not epoch 0)", () => {
    expect(observationClockMs(row({ detected_at: null }))).toBeNull();
    expect(observationClockMs(row({ detected_at: "not-a-date" }))).toBeNull();
  });

  it("classifies newer / same / older / incomparable", () => {
    const t1 = row({ detected_at: "2026-10-03T10:00:00Z" });
    const t2 = row({ detected_at: "2026-10-03T10:05:00Z" });
    expect(compareObservationOrder(t1, t2)).toBe("newer");
    expect(compareObservationOrder(t2, t1)).toBe("older");
    expect(compareObservationOrder(t1, row({ detected_at: "2026-10-03T10:00:00Z" }))).toBe("same");
    expect(compareObservationOrder(t1, row({ detected_at: null }))).toBe("incomparable");
  });
});

describe("stream-contract — §11.4 an OLD event must never overwrite a NEW one", () => {
  it("REJECTS a delayed older observation of the same id", () => {
    const newer = row({ id: "opp-1", detected_at: "2026-10-03T10:05:00Z" });
    const older = row({ id: "opp-1", detected_at: "2026-10-03T10:00:00Z" });
    const decision = decideUpsert(newer, older);
    expect(decision.apply).toBe(false);
    expect(decision.outcome).toBe("stale_rejected");
    expect(decision.reason).toBe("older_observation_clock");
  });

  it("REJECTS an older RE-DETECTION of the same route (different id)", () => {
    // The store's route-group path resolves the group, then asks the SAME
    // question — an older re-detection must not replace newer economics.
    const newer = row({ id: "det-2", detected_at: "2026-10-03T10:05:00Z" });
    const older = row({ id: "det-1", detected_at: "2026-10-03T10:00:00Z" });
    expect(decideUpsert(newer, older).outcome).toBe("stale_rejected");
  });

  it("ACCEPTS a newer observation", () => {
    const older = row({ id: "opp-1", detected_at: "2026-10-03T10:00:00Z" });
    const newer = row({ id: "opp-1", detected_at: "2026-10-03T10:05:00Z" });
    const decision = decideUpsert(older, newer);
    expect(decision.apply).toBe(true);
    expect(decision.outcome).toBe("accepted_update");
  });

  it("ACCEPTS a later computation on the SAME detection (migration-107 economics update)", () => {
    const before = row({ id: "opp-1", status: "detected", economics: null });
    const after = row({
      id: "opp-1",
      status: "validated",
      economics: {
        computation_status: "computed",
        net_profit_usd: -7.91,
        target_net_usd: 50,
      },
    });
    // Identical clock → the update is NOT stale: dropping it is the defect that
    // froze cards until the next full poll.
    expect(compareObservationOrder(before, after)).toBe("same");
    expect(decideUpsert(before, after).apply).toBe(true);
  });

  it("ACCEPTS an undated row and DECLARES the missing order authority (R8)", () => {
    const dated = row({ id: "opp-1", detected_at: "2026-10-03T10:05:00Z" });
    const undated = row({ id: "opp-1", detected_at: null });
    const decision = decideUpsert(dated, undated);
    expect(decision.apply).toBe(true);
    expect(decision.outcome).toBe("accepted_incomparable");
    expect(decision.reason).toBe("undated_no_order_authority");
  });
});

describe("stream-contract — idempotency (a replay is a NO-OP)", () => {
  it("drops a byte-identical application (same digest)", () => {
    const a = row({ id: "opp-1", detected_at: "2026-10-03T10:00:00Z" });
    const b = row({ id: "opp-1", detected_at: "2026-10-03T10:00:00Z" });
    expect(planDigestOf(a)).toBe(planDigestOf(b));
    const decision = decideUpsert(a, b);
    expect(decision.apply).toBe(false);
    expect(decision.outcome).toBe("duplicate");
    expect(decision.reason).toBe("identical_application_digest");
  });

  it("does NOT treat a real change as a duplicate", () => {
    const a = row({
      id: "opp-1",
      economics: { computation_status: "computed", net_profit_usd: -7.91 },
    });
    const b = row({
      id: "opp-1",
      economics: { computation_status: "computed", net_profit_usd: -3.5 },
    });
    expect(planDigestOf(a)).not.toBe(planDigestOf(b));
    expect(decideUpsert(a, b).apply).toBe(true);
  });

  it("same event_id is a duplicate even when the seq is equal (envelope replay)", () => {
    const prev = { seq: 7, event_id: "plan#7" };
    expect(sequenceVerdict(prev, { seq: 7, event_id: "plan#7" })).toBe("duplicate");
  });

  it("a seq that does not ADVANCE is rejected (re-ordered / stale)", () => {
    const prev = { seq: 7, event_id: "plan#7" };
    expect(sequenceVerdict(prev, { seq: 6, event_id: "plan#6" })).toBe("stale_rejected");
    expect(sequenceVerdict(prev, { seq: 8, event_id: "plan#8" })).toBe("accept");
    expect(sequenceVerdict({ seq: null, event_id: null }, { seq: 1, event_id: "p#1" })).toBe(
      "first",
    );
  });

  it("REGRESSION (reconnect after an api-server restart): the high-water mark must be droppable", () => {
    // A restarted producer begins at seq 1 again. With the old mark kept, every
    // legitimate post-restart event is rejected as stale — which is exactly why
    // the hook clears the map on reconnect and resyncs from the snapshot.
    const staleMark = { seq: 500, event_id: "plan#500" };
    expect(sequenceVerdict(staleMark, { seq: 1, event_id: "plan#1" })).toBe("stale_rejected");
    const cleared = { seq: null, event_id: null };
    expect(sequenceVerdict(cleared, { seq: 1, event_id: "plan#1" })).toBe("first");
  });
});

describe("stream-contract — envelope acceptance (fail-closed, never guessed)", () => {
  const valid = {
    schema_version: STREAM_SCHEMA_VERSION,
    event_id: "1|dex_arb#route@1000000#12",
    seq: 12,
    strategy_key: "1|dex_arb",
    plan_key: "1|dex_arb#route@1000000",
    snapshot_id: "snap-9",
    emitted_at: "2026-10-03T10:00:00.000Z",
    kind: "plan.updated",
    payload: { id: "opp-1" },
    progress: { routes_explored: 3, sizes_quoted: 2, quotes_exact: 1 },
  };

  it("accepts a well-formed envelope and keeps the payload intact", () => {
    const parsed = parseStreamEnvelope(valid);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.envelope.seq).toBe(12);
    expect(parsed.envelope.payload).toEqual({ id: "opp-1" });
    expect(parsed.envelope.progress?.routes_explored).toBe(3);
  });

  it.each([
    ["not_an_object", null],
    ["schema_version_mismatch:1", { ...valid, schema_version: 1 }],
    ["seq_not_a_non_negative_integer", { ...valid, seq: -1 }],
    ["seq_not_a_non_negative_integer", { ...valid, seq: 1.5 }],
    ["event_id_absent", { ...valid, event_id: "" }],
    ["strategy_key_absent", { ...valid, strategy_key: "" }],
    ["plan_key_absent", { ...valid, plan_key: "" }],
    ["unknown_kind:plan.exploded", { ...valid, kind: "plan.exploded" }],
    ["payload_not_an_object", { ...valid, payload: [] }],
    ["emitted_at_absent", { ...valid, emitted_at: "" }],
  ])("rejects a malformed envelope with its machine reason (%s)", (reason, raw) => {
    const parsed = parseStreamEnvelope(raw);
    expect(parsed.ok).toBe(false);
    if (parsed.ok) return;
    expect(parsed.reason).toBe(reason);
  });

  it("DROPS a malformed progress count instead of coercing it to 0 (R10 false zero)", () => {
    const parsed = parsePublisherProgress({
      routes_explored: "3", // a string is not a measured count
      sizes_quoted: 2.5, // non-integer
      quotes_exact: 4,
      quotes_incomplete: -1, // impossible
      budget_exhausted: "yes", // not a boolean
    });
    expect(parsed).toEqual({ quotes_exact: 4 });
    expect(parsePublisherProgress({})).toBeNull();
  });

  it("counts outcomes so a rejection is VISIBLE, never silent", () => {
    let c: StreamLedgerCounters = { ...EMPTY_LEDGER_COUNTERS };
    c = countOutcome(c, "accepted_new");
    c = countOutcome(c, "accepted_update");
    c = countOutcome(c, "duplicate");
    c = countOutcome(c, "stale_rejected");
    c = countOutcome(c, "stale_rejected");
    c = countOutcome(c, "accepted_incomparable");
    expect(c).toEqual({ accepted: 3, duplicates: 1, stale_rejected: 2, incomparable: 1 });
  });
});
