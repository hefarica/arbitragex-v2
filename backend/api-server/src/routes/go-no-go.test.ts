/**
 * go-no-go — A.9 formal GO/NO-GO ledger machinery tests (ARBX-RDY-06).
 *
 * Mocks the pg pool (SQL/params recorded, small in-memory state for ledger
 * generations + sign-offs); never touches a real Postgres. Mirrors the
 * readiness-evidence.test.ts harness (vitest + express + supertest + a
 * test-only requireAdminToken mirroring the shared middleware contract).
 *
 * Cases:
 *   (0)  canonicalization: recursive key sort, no whitespace
 *   (0b) hash stability: same facts (any key order) → same hash;
 *        changed facts → different hash; 64-char hex
 *   (0c) deriveGoNoGoState pure state machine
 *   (a)  GET /ledger generates + persists one audit row (parameterized)
 *   (b)  identical facts → deduplicated (no second audit row), same hash
 *   (c)  changed facts → different hash + NEW audit row
 *   (d)  facts builder throws → 503 facts_source_failed
 *   (e)  pool null → 503 (ledger + status + sign-off)
 *   (f)  audit insert fails → 503 ledger_persist_failed
 *   (g)  status before any ledger → no_ledger
 *   (h)  status after generation, no sign-offs → awaiting_first
 *   (i)  go_live_eligible stays false until signed_go (clean facts or not)
 *   (j)  sign-off without admin token → 401, zero DB access
 *   (k)  missing x-arbx-actor → 400
 *   (l)  unknown decision / malformed hash → 400
 *   (m)  no ledger generated yet → 400 no_ledger_generated
 *   (n)  stale ledger_hash (newer generation exists) → 400 + current hash
 *   (o)  happy path: two DISTINCT actors GO+GO → signed_go (+eligible when
 *        blockers 0 + paper safe); first response awaiting_second
 *   (o2) same actor twice → 409 duplicate_signoff with existing decision
 *   (o3) mixed GO+NO_GO → conflicted, eligible false
 *   (o4) signed_go but unresolved blockers > 0 → eligible false
 *   (o5) signed_go but paper_mode_active false → eligible false
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import type pg from "pg";
import {
  mountGoNoGo,
  canonicalJsonStringify,
  computeLedgerHash,
  deriveGoNoGoState,
  LEDGER_SCHEMA_VERSION,
  LEDGER_GENERATED_ACTION,
  type LedgerFacts,
} from "./go-no-go.js";

const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaa";

/** Test-only requireAdminToken — mirrors @arbx/shared contract. */
function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

const logger = { warn: vi.fn() };
const NOW = new Date("2026-08-28T12:00:00.000Z");

// ---------------------------------------------------------------------------
// Mock pool — in-memory ledger generations (audit_log) + sign-offs registry.
// ---------------------------------------------------------------------------

interface RecordedQuery {
  sql: string;
  params: unknown[];
}

interface MockState {
  ledgerRows: { id: string; created_at: Date; action: string; after_state: Record<string, unknown> }[];
  signoffs: { ledger_hash: string; actor: string; decision: string; signed_at: Date }[];
}

function makeMockPool(opts?: { auditInsertFail?: boolean }): {
  pool: pg.Pool;
  queries: RecordedQuery[];
  state: MockState;
} {
  const queries: RecordedQuery[] = [];
  const state: MockState = { ledgerRows: [], signoffs: [] };
  let seq = 0;
  const pool = {
    query: async (sql: string, params?: unknown[]): Promise<{ rows: Record<string, unknown>[] }> => {
      const s = sql.trim();
      queries.push({ sql: s, params: params ?? [] });

      // Latest ledger generation read — the real SQL filters by action, so
      // sign-off audit rows must NOT surface as "latest ledger".
      if (s.startsWith("SELECT") && s.includes("FROM audit_log")) {
        const wantAction = String(params?.[0]);
        const matching = state.ledgerRows.filter((r) => r.action === wantAction);
        const last = matching[matching.length - 1];
        return { rows: last ? [last as unknown as Record<string, unknown>] : [] };
      }

      // Audit persist (ledger generations AND sign-off entries).
      if (s.startsWith("INSERT INTO audit_log")) {
        if (opts?.auditInsertFail) throw new Error("audit insert refused");
        const after = JSON.parse(String(params?.[5])) as Record<string, unknown>;
        seq += 1;
        state.ledgerRows.push({
          id: `00000000-0000-0000-0000-00000000000${seq}`,
          created_at: NOW,
          action: String(params?.[1]),
          after_state: after,
        });
        return { rows: [] };
      }

      // Same-actor duplicate check.
      if (s.startsWith("SELECT") && s.includes("FROM go_no_go_signoffs") && s.includes("actor = $2")) {
        const [hash, actor] = [String(params?.[0]), String(params?.[1])];
        const prior = state.signoffs.find((r) => r.ledger_hash === hash && r.actor === actor);
        return { rows: prior ? [prior as unknown as Record<string, unknown>] : [] };
      }

      // Sign-offs for one ledger hash.
      if (s.startsWith("SELECT") && s.includes("FROM go_no_go_signoffs")) {
        const hash = String(params?.[0]);
        return {
          rows: state.signoffs
            .filter((r) => r.ledger_hash === hash)
            .sort((a, b) => a.signed_at.getTime() - b.signed_at.getTime())
            .map((r) => r as unknown as Record<string, unknown>),
        };
      }

      // Sign-off insert — enforce the UNIQUE (ledger_hash, actor) constraint.
      if (s.startsWith("INSERT INTO go_no_go_signoffs")) {
        const [hash, actor, decision] = [String(params?.[0]), String(params?.[1]), String(params?.[2])];
        if (state.signoffs.find((r) => r.ledger_hash === hash && r.actor === actor)) {
          throw Object.assign(
            new Error('duplicate key value violates unique constraint "go_no_go_signoffs_ledger_hash_actor_key"'),
            { code: "23505" },
          );
        }
        const row = { ledger_hash: hash, actor, decision, signed_at: NOW };
        state.signoffs.push(row);
        return { rows: [row as unknown as Record<string, unknown>] };
      }

      return { rows: [] };
    },
  };
  return { pool: pool as unknown as pg.Pool, queries, state };
}

// ---------------------------------------------------------------------------
// App factory — injects a MUTABLE facts document so tests can "change the
// world" between ledger generations.
// ---------------------------------------------------------------------------

function makeFacts(overrides?: {
  unresolved?: number;
  paperModeActive?: boolean | null;
}): LedgerFacts {
  return {
    schema_version: LEDGER_SCHEMA_VERSION,
    blockers: {
      unresolved_count: overrides?.unresolved ?? 0,
      critical_count: overrides?.unresolved ?? 0,
      source: "test:verifyAll",
    },
    paper_safety: {
      paper_mode_active: overrides?.paperModeActive ?? true,
      capital_exposure_usd: 0,
      source: "test:resolvePaperModeState",
    },
  };
}

function buildApp(
  pool: pg.Pool | null,
  factsRef: { current: LedgerFacts },
): Express {
  const app = express();
  app.use(express.json());
  mountGoNoGo(app, {
    pool,
    logger,
    requireAdminToken,
    adminToken: ADMIN_TOKEN,
    buildLedgerFacts: async () => factsRef.current,
  });
  return app;
}

async function generateLedger(app: Express): Promise<{ ledger_hash: string; summary: { unresolved_blockers: number | null; paper_safe: boolean | null } }> {
  const res = await request(app).get("/api/v1/go-no-go/ledger");
  expect(res.status).toBe(200);
  return res.body as { ledger_hash: string; summary: { unresolved_blockers: number | null; paper_safe: boolean | null } };
}

function signOff(app: Express, actor: string, decision: string, ledgerHash: string) {
  return request(app)
    .post("/admin/go-no-go/sign-off")
    .set("x-arbx-admin-token", ADMIN_TOKEN)
    .set("x-arbx-actor", actor)
    .send({ decision, ledger_hash: ledgerHash });
}

// ---------------------------------------------------------------------------

describe("go-no-go — canonicalization + hashing (pure)", () => {
  it("(0) canonicalJsonStringify sorts keys recursively, no whitespace", () => {
    expect(canonicalJsonStringify({ b: 1, a: { d: 2, c: 3 } })).toBe('{"a":{"c":3,"d":2},"b":1}');
    expect(canonicalJsonStringify([{ z: 1, y: [2, { b: 1, a: 0 }] }])).toBe('[{"y":[2,{"a":0,"b":1}],"z":1}]');
  });

  it("(0b) same facts (any key order) → same hash; changed facts → different hash", () => {
    const a = makeFacts();
    const b: LedgerFacts = JSON.parse(JSON.stringify(makeFacts())) as LedgerFacts;
    // Re-serialize with reversed key order to prove order-independence.
    const reordered = Object.fromEntries(Object.entries(b).reverse()) as unknown as LedgerFacts;
    const h1 = computeLedgerHash(a, LEDGER_SCHEMA_VERSION);
    const h2 = computeLedgerHash(reordered, LEDGER_SCHEMA_VERSION);
    expect(h1).toMatch(/^[0-9a-f]{64}$/);
    expect(h2).toBe(h1);
    const changed = makeFacts({ unresolved: 1 });
    expect(computeLedgerHash(changed, LEDGER_SCHEMA_VERSION)).not.toBe(h1);
    // Schema version is part of the hash input.
    expect(computeLedgerHash(a, LEDGER_SCHEMA_VERSION + 1)).not.toBe(h1);
  });

  it("(0c) deriveGoNoGoState state machine", () => {
    expect(deriveGoNoGoState([])).toBe("awaiting_first");
    expect(deriveGoNoGoState([{ decision: "GO" }])).toBe("awaiting_second");
    expect(deriveGoNoGoState([{ decision: "NO_GO" }])).toBe("awaiting_second");
    expect(deriveGoNoGoState([{ decision: "GO" }, { decision: "GO" }])).toBe("signed_go");
    expect(deriveGoNoGoState([{ decision: "NO_GO" }, { decision: "NO_GO" }])).toBe("signed_no_go");
    expect(deriveGoNoGoState([{ decision: "GO" }, { decision: "NO_GO" }])).toBe("conflicted");
    expect(deriveGoNoGoState([{ decision: "GO" }, { decision: "GO" }, { decision: "GO" }])).toBe("signed_go");
  });
});

describe("go-no-go — GET /api/v1/go-no-go/ledger", () => {
  it("(a) generates + persists ONE audit row, parameterized, summary computed", async () => {
    const { pool, queries } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 2 }) };
    const app = buildApp(pool, factsRef);

    const res = await request(app).get("/api/v1/go-no-go/ledger");
    expect(res.status).toBe(200);
    expect(res.body.ledger_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(res.body.schema_version).toBe(LEDGER_SCHEMA_VERSION);
    expect(res.body.deduplicated).toBe(false);
    expect(res.body.summary).toEqual({ unresolved_blockers: 2, paper_safe: true });
    expect(res.body.facts.blockers.source).toBe("test:verifyAll");

    const inserts = queries.filter((q) => q.sql.startsWith("INSERT INTO audit_log"));
    expect(inserts).toHaveLength(1);
    const ins = inserts[0]!;
    expect(ins.sql).toContain("arbx_anonymize_ip");
    expect(ins.params[0]).toBe("system:go-no-go");
    expect(ins.params[1]).toBe(LEDGER_GENERATED_ACTION);
    expect(ins.params[2]).toBe("go_no_go");
    expect(ins.params[3]).toBe(res.body.ledger_hash);
    const after = JSON.parse(String(ins.params[5])) as { ledger_hash: string };
    expect(after.ledger_hash).toBe(res.body.ledger_hash);
    // Parameterized only — no value interpolated into the SQL text.
    expect(ins.sql).not.toContain("go_no_go.ledger_generated");
    expect(ins.sql).not.toContain(res.body.ledger_hash);
  });

  it("(b) identical facts → deduplicated, same hash, still ONE audit row", async () => {
    const { pool, queries } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);

    const first = await request(app).get("/api/v1/go-no-go/ledger");
    const second = await request(app).get("/api/v1/go-no-go/ledger");
    expect(first.status).toBe(200);
    expect(second.status).toBe(200);
    expect(second.body.ledger_hash).toBe(first.body.ledger_hash);
    expect(second.body.deduplicated).toBe(true);
    expect(queries.filter((q) => q.sql.startsWith("INSERT INTO audit_log"))).toHaveLength(1);
  });

  it("(c) changed facts → different hash + NEW audit row", async () => {
    const { pool, queries } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);

    const first = await request(app).get("/api/v1/go-no-go/ledger");
    factsRef.current = makeFacts({ unresolved: 5 });
    const second = await request(app).get("/api/v1/go-no-go/ledger");

    expect(second.status).toBe(200);
    expect(second.body.ledger_hash).not.toBe(first.body.ledger_hash);
    expect(second.body.deduplicated).toBe(false);
    expect(second.body.summary.unresolved_blockers).toBe(5);
    expect(queries.filter((q) => q.sql.startsWith("INSERT INTO audit_log"))).toHaveLength(2);
  });

  it("(d) facts builder throws → 503 facts_source_failed, nothing persisted", async () => {
    const { pool, queries } = makeMockPool();
    const app = express();
    app.use(express.json());
    mountGoNoGo(app, {
      pool,
      logger,
      requireAdminToken,
      adminToken: ADMIN_TOKEN,
      buildLedgerFacts: async () => {
        throw new Error("readiness verifier exploded");
      },
    });
    const res = await request(app).get("/api/v1/go-no-go/ledger");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("facts_source_failed");
    expect(queries.filter((q) => q.sql.startsWith("INSERT"))).toHaveLength(0);
  });

  it("(e) pool null → 503 db_unavailable", async () => {
    const factsRef = { current: makeFacts() };
    const res = await request(buildApp(null, factsRef)).get("/api/v1/go-no-go/ledger");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
  });

  it("(f) audit insert fails → 503 ledger_persist_failed (load-bearing write)", async () => {
    const { pool } = makeMockPool({ auditInsertFail: true });
    const factsRef = { current: makeFacts() };
    const res = await request(buildApp(pool, factsRef)).get("/api/v1/go-no-go/ledger");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("ledger_persist_failed");
  });
});

describe("go-no-go — GET /api/v1/go-no-go/status", () => {
  it("(g) before any ledger → no_ledger, hash null, eligible false", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const res = await request(buildApp(pool, factsRef)).get("/api/v1/go-no-go/status");
    expect(res.status).toBe(200);
    expect(res.body).toEqual({
      ledger_hash: null,
      sign_offs: [],
      state: "no_ledger",
      go_live_eligible: false,
    });
  });

  it("(h) after generation, no sign-offs → awaiting_first; (i) eligible false even with clean facts", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 0, paperModeActive: true }) };
    const app = buildApp(pool, factsRef);
    await generateLedger(app);

    const res = await request(app).get("/api/v1/go-no-go/status");
    expect(res.status).toBe(200);
    expect(res.body.state).toBe("awaiting_first");
    expect(res.body.sign_offs).toEqual([]);
    expect(res.body.ledger_summary).toEqual({ unresolved_blockers: 0, paper_safe: true });
    // Reads recorded state only — no sign-offs means never eligible.
    expect(res.body.go_live_eligible).toBe(false);
  });
});

describe("go-no-go — POST /admin/go-no-go/sign-off", () => {
  it("(j) without admin token → 401, zero DB access", async () => {
    const { pool, queries } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);
    queries.length = 0;

    const res = await request(app)
      .post("/admin/go-no-go/sign-off")
      .set("x-arbx-actor", "operator-a")
      .send({ decision: "GO", ledger_hash: ledger.ledger_hash });
    expect(res.status).toBe(401);
    expect(queries).toHaveLength(0);
  });

  it("(k) missing x-arbx-actor → 400 missing_actor", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    const res = await request(app)
      .post("/admin/go-no-go/sign-off")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ decision: "GO", ledger_hash: ledger.ledger_hash });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("missing_actor");
  });

  it("(l) unknown decision / malformed hash → 400 invalid_request", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    const badDecision = await signOff(app, "operator-a", "MAYBE", ledger.ledger_hash);
    expect(badDecision.status).toBe(400);
    expect(badDecision.body.error).toBe("invalid_request");

    const badHash = await signOff(app, "operator-a", "GO", "not-a-hash");
    expect(badHash.status).toBe(400);
    expect(badHash.body.error).toBe("invalid_request");
  });

  it("(m) no ledger generated yet → 400 no_ledger_generated", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);
    const res = await signOff(app, "operator-a", "GO", "a".repeat(64));
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("no_ledger_generated");
  });

  it("(n) stale ledger_hash → 400 with the current hash surfaced", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);

    const first = await generateLedger(app);
    factsRef.current = makeFacts({ unresolved: 7 });
    const second = await generateLedger(app);
    expect(second.ledger_hash).not.toBe(first.ledger_hash);

    const res = await signOff(app, "operator-a", "GO", first.ledger_hash);
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("stale_ledger_hash");
    expect(res.body.current_ledger_hash).toBe(second.ledger_hash);
    expect(res.body.submitted_ledger_hash).toBe(first.ledger_hash);
  });

  it("(o) happy path: two DISTINCT actors GO+GO → signed_go + go_live_eligible", async () => {
    const { pool, queries } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 0, paperModeActive: true }) };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    const first = await signOff(app, "operator-a", "GO", ledger.ledger_hash);
    expect(first.status).toBe(201);
    expect(first.body.state).toBe("awaiting_second");
    expect(first.body.go_live_eligible).toBe(false);

    const second = await signOff(app, "operator-b", "GO", ledger.ledger_hash);
    expect(second.status).toBe(201);
    expect(second.body.state).toBe("signed_go");
    expect(second.body.go_live_eligible).toBe(true);

    // Sign-off INSERT is parameterized.
    const ins = queries.find((q) => q.sql.startsWith("INSERT INTO go_no_go_signoffs"))!;
    expect(ins.params).toEqual([ledger.ledger_hash, "operator-a", "GO"]);
    // Audit trail got a sign-off entry per accepted POST (ledger gen + 2).
    const auditSignOffs = queries.filter(
      (q) => q.sql.startsWith("INSERT INTO audit_log") && q.params[1] === "go_no_go.sign_off",
    );
    expect(auditSignOffs).toHaveLength(2);

    // Status reads the recorded state back.
    const status = await request(app).get("/api/v1/go-no-go/status");
    expect(status.status).toBe(200);
    expect(status.body.state).toBe("signed_go");
    expect(status.body.go_live_eligible).toBe(true);
    expect(status.body.sign_offs).toEqual([
      { actor: "operator-a", decision: "GO", signed_at: NOW.toISOString() },
      { actor: "operator-b", decision: "GO", signed_at: NOW.toISOString() },
    ]);
  });

  it("(o2) same actor twice → 409 duplicate_signoff with existing decision", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts() };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    const first = await signOff(app, "operator-a", "GO", ledger.ledger_hash);
    expect(first.status).toBe(201);

    const dup = await signOff(app, "operator-a", "NO_GO", ledger.ledger_hash);
    expect(dup.status).toBe(409);
    expect(dup.body.error).toBe("duplicate_signoff");
    expect(dup.body.existing_decision).toBe("GO");
    expect(dup.body.detail).toContain("operator-a");
    expect(dup.body.detail).toContain("DISTINCT");
  });

  it("(o3) mixed GO+NO_GO → conflicted, eligible false", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 0 }) };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    await signOff(app, "operator-a", "GO", ledger.ledger_hash);
    const second = await signOff(app, "operator-b", "NO_GO", ledger.ledger_hash);
    expect(second.status).toBe(201);
    expect(second.body.state).toBe("conflicted");
    expect(second.body.go_live_eligible).toBe(false);

    const status = await request(app).get("/api/v1/go-no-go/status");
    expect(status.body.state).toBe("conflicted");
    expect(status.body.go_live_eligible).toBe(false);
  });

  it("(o4) signed_go but unresolved blockers > 0 → eligible false", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 3, paperModeActive: true }) };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    await signOff(app, "operator-a", "GO", ledger.ledger_hash);
    const second = await signOff(app, "operator-b", "GO", ledger.ledger_hash);
    expect(second.body.state).toBe("signed_go");
    expect(second.body.go_live_eligible).toBe(false);

    const status = await request(app).get("/api/v1/go-no-go/status");
    expect(status.body.ledger_summary.unresolved_blockers).toBe(3);
    expect(status.body.go_live_eligible).toBe(false);
  });

  it("(o5) signed_go but paper_mode_active false → eligible false", async () => {
    const { pool } = makeMockPool();
    const factsRef = { current: makeFacts({ unresolved: 0, paperModeActive: false }) };
    const app = buildApp(pool, factsRef);
    const ledger = await generateLedger(app);

    await signOff(app, "operator-a", "GO", ledger.ledger_hash);
    const second = await signOff(app, "operator-b", "GO", ledger.ledger_hash);
    expect(second.body.state).toBe("signed_go");
    expect(second.body.go_live_eligible).toBe(false);
  });

  it("(e2) pool null → 503 db_unavailable", async () => {
    const factsRef = { current: makeFacts() };
    const res = await signOff(buildApp(null, factsRef), "operator-a", "GO", "a".repeat(64));
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
  });
});
