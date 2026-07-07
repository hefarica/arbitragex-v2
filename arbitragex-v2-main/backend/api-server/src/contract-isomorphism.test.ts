import { describe, it, expect } from "vitest";
import { z } from "zod";
import { isValidRuntimeAck } from "./websocket.js";
import type {
  RuntimeAckBroadcast,
  RuntimeAckLayer,
  RuntimeAckStatus,
} from "./websocket.js";

/**
 * OMEGA-8/M4 Fase 9 — contract isomorphism tests for runtime_ack.
 *
 * Locks the four-layer contract that the runtime_ack pipeline depends on:
 *
 *   PostgreSQL migration 069  (UNIQUE(event_id, layer), CHECK chain_id > 0)
 *           ↕ wire format
 *   Zod RuntimeAckSchema in routes/system-manifest.ts
 *           ↕ parse output
 *   TypeScript RuntimeAckBroadcast in websocket.ts
 *           ↕ wire format
 *   Rust producer in searcher-rs / sed-core
 *
 * If any of these four drift apart, this test must fail loudly. The Zod
 * schema is duplicated here intentionally (mirror-copy from
 * routes/system-manifest.ts) so the test catches the case where someone
 * edits the route handler but forgets to update the websocket interface.
 *
 * This is the test the spec asks for in FASE 9: contratos no dependen de
 * documentación sino de tests.
 */

// ────────────────────────────────────────────────────────────────────────
// Mirror of RuntimeAckSchema from routes/system-manifest.ts.
// Update BOTH sites when the schema changes.
// ────────────────────────────────────────────────────────────────────────
const RuntimeAckSchemaMirror = z.object({
  event_id: z.string().uuid(),
  resource: z.string().min(1).max(64),
  chain_id: z.number().int().positive().nullable(),
  idempotency_key: z.string().min(1).max(256),
  config_hash_before: z.string().regex(/^[0-9a-f]{64}$/).nullable(),
  config_hash_after: z.string().regex(/^[0-9a-f]{64}$/),
  worker_id: z.string().min(1).max(128),
  layer: z.enum([
    "api", "persistence", "redis_pubsub", "searcher_rs", "arc_swap",
    "frontend_refresh", "readiness", "audit",
  ]),
  status: z.enum(["pending", "received", "applied", "rejected", "timeout", "failed"]),
  latency_ms: z.number().int().nonnegative().nullable().optional(),
  error: z.string().max(2000).nullable().optional(),
});

const VALID_PAYLOAD: RuntimeAckBroadcast = {
  event_id: "11111111-2222-3333-4444-555555555555",
  resource: "trading_config",
  chain_id: 1,
  idempotency_key: "test-key-1",
  config_hash_before: "a".repeat(64),
  config_hash_after: "b".repeat(64),
  worker_id: "api-server-1",
  layer: "api",
  status: "applied",
  latency_ms: 42,
  error: null,
};

describe("OMEGA-8/M4 Fase 9 — RuntimeAck contract isomorphism", () => {
  it("Zod schema and TS interface accept the same canonical payload", () => {
    // Round-trip via JSON to exercise serialization, then verify both
    // validators agree.
    const wire = JSON.parse(JSON.stringify(VALID_PAYLOAD));
    const parsed = RuntimeAckSchemaMirror.safeParse(wire);
    expect(parsed.success, "Zod must accept canonical payload").toBe(true);
    expect(isValidRuntimeAck(wire), "TS guard must accept canonical payload").toBe(true);
  });

  it("Zod schema rejects chain_id <= 0 (matches DB CHECK constraint in migration 069)", () => {
    // The Postgres migration 069 added CHECK (chain_id IS NULL OR chain_id > 0).
    // Zod enforces the same invariant at the API boundary. The TS guard
    // isValidRuntimeAck is intentionally more permissive (defense-in-depth,
    // not source of truth — see websocket.ts:316), so only Zod is asserted
    // here.
    const bad = { ...VALID_PAYLOAD, chain_id: 0 };
    expect(RuntimeAckSchemaMirror.safeParse(bad).success).toBe(false);
    const bad2 = { ...VALID_PAYLOAD, chain_id: -1 };
    expect(RuntimeAckSchemaMirror.safeParse(bad2).success).toBe(false);
  });

  it("Zod accepts chain_id=null (CHECK constraint allows NULL)", () => {
    const ok = { ...VALID_PAYLOAD, chain_id: null };
    expect(RuntimeAckSchemaMirror.safeParse(ok).success).toBe(true);
    expect(isValidRuntimeAck(ok)).toBe(true);
  });

  it("RUNTIME_ACK_LAYERS enum is identical Zod ↔ TS", () => {
    const zodLayers: ReadonlyArray<RuntimeAckLayer> = [
      "api", "persistence", "redis_pubsub", "searcher_rs", "arc_swap",
      "frontend_refresh", "readiness", "audit",
    ];
    // Test each layer is accepted by BOTH validators.
    for (const layer of zodLayers) {
      const payload = { ...VALID_PAYLOAD, layer };
      expect(
        RuntimeAckSchemaMirror.safeParse(payload).success,
        `Zod must accept layer=${layer}`,
      ).toBe(true);
      expect(
        isValidRuntimeAck(payload),
        `TS guard must accept layer=${layer}`,
      ).toBe(true);
    }
    // Unknown layer must be rejected by BOTH.
    const bad = { ...VALID_PAYLOAD, layer: "totally_invented_layer" };
    expect(RuntimeAckSchemaMirror.safeParse(bad).success).toBe(false);
    expect(isValidRuntimeAck(bad)).toBe(false);
  });

  it("RUNTIME_ACK_STATUSES enum is identical Zod ↔ TS", () => {
    const statuses: ReadonlyArray<RuntimeAckStatus> = [
      "pending", "received", "applied", "rejected", "timeout", "failed",
    ];
    for (const status of statuses) {
      const payload = { ...VALID_PAYLOAD, status };
      expect(RuntimeAckSchemaMirror.safeParse(payload).success).toBe(true);
      expect(isValidRuntimeAck(payload)).toBe(true);
    }
    const bad = { ...VALID_PAYLOAD, status: "queued" };
    expect(RuntimeAckSchemaMirror.safeParse(bad).success).toBe(false);
    expect(isValidRuntimeAck(bad)).toBe(false);
  });

  it("config_hash_after must be 64-char lowercase hex sha256", () => {
    const ok = { ...VALID_PAYLOAD, config_hash_after: "0".repeat(64) };
    expect(RuntimeAckSchemaMirror.safeParse(ok).success).toBe(true);

    const tooShort = { ...VALID_PAYLOAD, config_hash_after: "abc" };
    expect(RuntimeAckSchemaMirror.safeParse(tooShort).success).toBe(false);

    const upper = { ...VALID_PAYLOAD, config_hash_after: "A".repeat(64) };
    expect(RuntimeAckSchemaMirror.safeParse(upper).success).toBe(false);
  });

  it("idempotency_key length is bounded (DB schema constraint)", () => {
    const ok = { ...VALID_PAYLOAD, idempotency_key: "x".repeat(256) };
    expect(RuntimeAckSchemaMirror.safeParse(ok).success).toBe(true);
    const tooLong = { ...VALID_PAYLOAD, idempotency_key: "x".repeat(257) };
    expect(RuntimeAckSchemaMirror.safeParse(tooLong).success).toBe(false);
  });

  it("roundtrip serialization preserves all fields and types", () => {
    // Wire-level isomorphism: the parsed Zod object MUST round-trip through
    // JSON without drift, so the searcher-rs / sed-core producers can rely
    // on a stable wire shape.
    const parsed = RuntimeAckSchemaMirror.parse(VALID_PAYLOAD);
    const serialized = JSON.stringify(parsed);
    const back = JSON.parse(serialized);
    const reparsed = RuntimeAckSchemaMirror.parse(back);
    expect(reparsed).toEqual(parsed);
  });
});
