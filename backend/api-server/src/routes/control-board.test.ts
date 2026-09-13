// CB-02 (2026-09-07)
/**
 * control-board — unit tests (WO CB-02, TS/API half).
 *
 * Harness mirrors service-control.test.ts / stubs.test.ts (vitest + express +
 * supertest + a test-only requireAdminToken mirroring the @arbx/shared
 * contract). pool/redis are test doubles — no real PG/Redis is touched and
 * NO production data is fabricated (fixtures are census fixtures, the exact
 * shape CB-01 publishes).
 *
 * Charter + CB-02-DISENO §15 gates pinned here:
 *   (a)  both verbs admin-gated (401 without x-arbx-admin-token)
 *   (b)  census absent ⇒ GET 200 with EMPTY modules (RULE 00 "snapshot
 *        vacío si el censo no publica") and PUT 503 fail-safe deny
 *   (c)  verified data flows; missing verified ⇒ null DESCONOCIDO, never false
 *   (c3) CB-02-DISENO §15-R3: class-A verified is LIVE (worker heartbeat /
 *        killswitch JSON), never the stale census photo; hb absent/expired ⇒
 *        verified_on null KEEPING the census last-known verified_at
 *   (c5) CB-02-DISENO §4.2 step 5: class-A declared = approved ?? census
 *        default — the approval hash WINS over the live toggle key (a
 *        foreign write to the key is drift to expose, never a declaration);
 *        no approval + no census default ⇒ null; garbage hash entry ⇒ null
 *   (d)  PUT reason non-empty AFTER TRIM (server-side validation)
 *   (e)  §34.3: class C ⇒ 403, terminus denylist ⇒ 403 EVEN when the census
 *        mislabels it class A (defense in depth — server is the law)
 *   (f)  class B ⇒ 409 (boot-time env — CB-05 proposal flow)
 *   (g)  CB-02-DISENO §15-R1: audit_log (SINGULAR, migration 011 columns
 *        actor/action/target_kind/target_id/before_state/after_state) INSERT
 *        happens BEFORE the Redis SET (invocation order); audit failure ⇒ 500
 *        with NO Redis write
 *   (g2) CB-02-DISENO §4.3: the happy path writes BOTH the idempotent toggle
 *        SET and the approval-registry HSET `arbx:controlboard:approved`
 *        ({on,reason,actor,updated_at}); the response snapshot's declared_on
 *        comes from the just-written approval
 *   (h)  Redis SET failure ⇒ COMPENSATORY INSERT control_board.toggle_failed
 *        with applied:false — and NEVER an UPDATE (011:21 append-only); the
 *        approval HSET never runs after a failed SET
 *   (i)  CB-02-DISENO §1.3-4/INV-CB02-4 (cross-exam G4): a class-A
 *        control_key outside the board namespace `arbx:controlboard:*`
 *        (killswitch JSON, api: link) is refused — no audit, no write
 *   (n)  CB-02-DISENO §4.3/GATE-CB02-1: route_scanner bound to the scanner
 *        toggle key with census declared_on:false (worker NOT spawned) ⇒
 *        409 module_not_spawned, no audit, no writes; declared_on:true (or
 *        absent) does NOT trigger the gate
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import type pg from "pg";
import type { Redis } from "ioredis";
import {
  mountControlBoard,
  CONTROL_BOARD_CENSUS_REDIS_KEY,
  CONTROL_BOARD_TOGGLE_AUDIT_ACTION,
  CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION,
  CONTROL_BOARD_APPROVED_HASH,
} from "./control-board.js";

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

// ─── Census fixture (the exact shape the CB-01 publisher will write) ────────

// CB-02-DISENO §1.2/§8: the board's ONLY writable class-A dialect is a raw
// key inside `arbx:controlboard:*` (this board is the single writer).
const CLASS_A_KEY = "arbx:controlboard:route_scanner";
const CLASS_A_HB_KEY = `${CLASS_A_KEY}:hb`;
// R5 link row in the registry dialect (`redis:`-prefixed — resolver strips it).
const KILLSWITCH_KEY = "arbx:killswitch";

const CENSUS = {
  modules: [
    {
      id: "route_scanner_multihop",
      name: "route_scanner multihop (RU-3)",
      description: "Scanner multihop hops 2-6",
      module_class: "A",
      control_key: CLASS_A_KEY,
      telemetry_href: "/operations",
      // census photo (CB-01 observed 12:13Z) — R3: GET must NOT serve this
      // as the live verified state once the module carries a board key.
      verified_on: true,
      verified_source: "log:route_scanner.mode",
      verified_at: "2026-09-07T12:13:15Z",
    },
    {
      // R5: killswitch link row — live LED from arbx:killswitch, toggle
      // lives at /admin/killswitch, never on this board.
      id: "kill_switch",
      name: "Kill-switch (link)",
      module_class: "A",
      control_key: `redis:${KILLSWITCH_KEY}`,
      verified_on: true,
      verified_source: "redis:GET arbx:killswitch enabled=false (disarmado → detección ACTIVA)",
      verified_at: "2026-09-07T13:05:00Z",
    },
    {
      // A-link row: an external runtime surface the board ENLACES — no live
      // Redis signal for the api-server to read ⇒ census verbatim.
      id: "cartridges_runtime",
      name: "Cartridge runtime (link)",
      module_class: "A",
      control_key: "api:/api/cartridges/runtime",
      verified_on: true,
      verified_source: "api:/api/cartridges/runtime",
      verified_at: "2026-09-07T13:05:00Z",
    },
    {
      id: "scoring_hard_gate",
      name: "Scoring hard gate",
      module_class: "B",
      control_key: "ARBX_SCORING_HARD_GATE",
      declared_on: false,
      verified_on: false,
      verified_source: "env:absent-default-false",
    },
    {
      // class C WITHOUT terminus vocabulary → exercises the class-C branch
      id: "terminus_paper_live",
      name: "Terminus paper/live §34.3",
      module_class: "C",
      control_key: "operator-gate:§34.3",
    },
    {
      // census MISTAKE: terminus env var labeled class A → terminus denylist
      // must 403 it anyway (defense in depth)
      id: "exec_flipping",
      name: "Census mistake: terminus as A",
      module_class: "A",
      control_key: "ARBX_LIVE_EXEC_ENABLED",
    },
    {
      // census DEFECT: class A without control_key → refuse to write
      id: "broken_class_a",
      name: "Census defect: A sin control_key",
      module_class: "A",
    },
    {
      // census DEFECT (G4): class A pointing at a FOREIGN redis key (the
      // killswitch JSON) → namespace guard refuses — the board can never be
      // made to corrupt a key it does not own
      id: "foreign_key_class_a",
      name: "Census defect: A con control_key ajeno",
      module_class: "A",
      control_key: "arbx:killswitch",
    },
  ],
};

// ─── Harness ────────────────────────────────────────────────────────────────

interface Harness {
  app: Express;
  poolQuery: ReturnType<typeof vi.fn>;
  redisGet: ReturnType<typeof vi.fn>;
  redisSet: ReturnType<typeof vi.fn>;
  redisHset: ReturnType<typeof vi.fn>;
  redisHgetall: ReturnType<typeof vi.fn>;
}

function buildHarness(opts: {
  census?: string | null; // string (raw JSON, possibly corrupted) | null (absent)
  declaredRaw?: string | null; // live value at CLASS_A_KEY
  hbRaw?: string | null; // live heartbeat JSON at CLASS_A_HB_KEY
  killswitchRaw?: string | null; // live JSON at arbx:killswitch
  approved?: Record<string, string>; // pre-seeded arbx:controlboard:approved hash
  poolImpl?: (text: string, values: unknown[]) => { rows: unknown[]; rowCount: number };
  poolNull?: boolean;
  failRedisGet?: boolean;
  failRedisSet?: boolean;
}): Harness {
  const store = new Map<string, string>();
  if (opts.census !== null && opts.census !== undefined) {
    store.set(CONTROL_BOARD_CENSUS_REDIS_KEY, opts.census);
  } else if (opts.census === undefined) {
    store.set(CONTROL_BOARD_CENSUS_REDIS_KEY, JSON.stringify(CENSUS));
  }
  if (opts.declaredRaw !== undefined && opts.declaredRaw !== null) {
    store.set(CLASS_A_KEY, opts.declaredRaw);
  }
  if (opts.hbRaw !== undefined && opts.hbRaw !== null) {
    store.set(CLASS_A_HB_KEY, opts.hbRaw);
  }
  if (opts.killswitchRaw !== undefined && opts.killswitchRaw !== null) {
    store.set(KILLSWITCH_KEY, opts.killswitchRaw);
  }
  // CB-02 (2026-09-07) — approval-registry hash (§4.2 step 5 / §4.3 HSET).
  const hashStore = new Map<string, Record<string, string>>();
  if (opts.approved) hashStore.set(CONTROL_BOARD_APPROVED_HASH, { ...opts.approved });

  const redisGet = vi.fn(async (k: string) => {
    if (opts.failRedisGet) throw new Error("redis down");
    return store.get(k) ?? null;
  });
  const redisSet = vi.fn(async (k: string, v: string) => {
    if (opts.failRedisSet) throw new Error("redis write refused");
    store.set(k, v);
    return "OK";
  });
  const redisHgetall = vi.fn(async (k: string) => {
    if (opts.failRedisGet) throw new Error("redis down");
    const h = hashStore.get(k);
    return h ? { ...h } : {};
  });
  const redisHset = vi.fn(async (k: string, field: string, v: string) => {
    if (opts.failRedisSet) throw new Error("redis write refused");
    const h = hashStore.get(k) ?? {};
    h[field] = v;
    hashStore.set(k, h);
    return 1;
  });
  const redis = {
    get: redisGet,
    set: redisSet,
    hgetall: redisHgetall,
    hset: redisHset,
  } as unknown as Redis;

  const defaultPoolImpl = async (text: string): Promise<{ rows: unknown[]; rowCount: number }> => {
    if (text.startsWith("INSERT INTO audit_log")) return { rows: [], rowCount: 1 };
    if (text.startsWith("SELECT DISTINCT ON")) return { rows: [], rowCount: 0 };
    throw new Error(`unexpected sql in test: ${text.slice(0, 48)}`);
  };
  const poolQuery = vi.fn(opts.poolImpl ? opts.poolImpl : defaultPoolImpl);
  const pool = opts.poolNull ? null : ({ query: poolQuery } as unknown as pg.Pool);

  const app = express();
  app.use(express.json());
  mountControlBoard(app, {
    pool,
    redis,
    requireAdminToken,
    adminToken: ADMIN_TOKEN,
    logger: { warn: vi.fn(), info: vi.fn() },
  });
  return { app, poolQuery, redisGet, redisSet, redisHset, redisHgetall };
}

const AUTH = ["x-arbx-admin-token", ADMIN_TOKEN] as const;

// ─── GET /api/v1/control-board ──────────────────────────────────────────────

describe("CB-02 control-board GET", () => {
  it("(a) without admin token → 401", async () => {
    const res = await request(buildHarness({}).app).get("/api/v1/control-board");
    expect(res.status).toBe(401);
  });

  it("(b) census absent → 200 with EMPTY modules (RULE 00 snapshot vacío, never fabricated LEDs)", async () => {
    const res = await request(buildHarness({ census: null }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    expect(res.body.modules).toEqual([]);
    expect(typeof res.body.generated_at).toBe("string");
    expect(res.body.generated_at.length).toBeGreaterThan(0);
  });

  it("(c) full census → wire shape: class A declared from Redis, class B declared from census, missing verified stays null", async () => {
    const lastToggleAt = new Date("2026-09-07T12:13:15Z");
    const h = buildHarness({
      declaredRaw: "false",
      poolImpl: async (text) => {
        if (text.startsWith("SELECT DISTINCT ON")) {
          return {
            rows: [
              {
                target_id: "route_scanner_multihop",
                actor: "admin",
                after_state: { reason: "encender RU-3" },
                created_at: lastToggleAt,
              },
            ],
            rowCount: 1,
          };
        }
        if (text.startsWith("INSERT INTO audit_log")) return { rows: [], rowCount: 1 };
        throw new Error("unexpected sql");
      },
    });
    const res = await request(h.app).get("/api/v1/control-board").set(...AUTH);
    expect(res.status).toBe(200);
    const mods = res.body.modules as Array<Record<string, unknown>>;
    expect(mods).toHaveLength(8);

    const a = mods.find((m) => m["id"] === "route_scanner_multihop")!;
    expect(a["module_class"]).toBe("A");
    // (c5) §4.2 step 5: declared = approved ?? census_default — NO approval
    // on record and the census fixture carries no default ⇒ null. The LIVE
    // key holds "false" here and is deliberately NOT served as declared
    // (a foreign write is drift to expose, never the operator's word).
    expect(a["declared_on"]).toBeNull();
    // (c3) R3: NO heartbeat live ⇒ verified_on null DESCONOCIDO — the census
    // photo (verified_on:true @12:13Z) must NOT be served as current truth;
    // verified_at keeps the last-known ts ("última verificación, ahora
    // DESCONOCIDO").
    expect(a["verified_on"]).toBeNull();
    expect(a["verified_at"]).toBe("2026-09-07T12:13:15Z");
    expect(a["last_reason"]).toBe("encender RU-3"); // audit enrichment
    expect(a["last_actor"]).toBe("admin");
    expect(a["updated_at"]).toBe(lastToggleAt.toISOString());

    const b = mods.find((m) => m["id"] === "scoring_hard_gate")!;
    expect(b["module_class"]).toBe("B");
    expect(b["declared_on"]).toBe(false); // census-carried for boot-env
    expect(b["verified_on"]).toBe(false);

    const c = mods.find((m) => m["id"] === "terminus_paper_live")!;
    expect(c["module_class"]).toBe("C");
    expect(c["verified_on"]).toBeNull(); // DESCONOCIDO — never false

    // A-link row (api: surface): no live signal ⇒ census observation verbatim.
    const link = mods.find((m) => m["id"] === "cartridges_runtime")!;
    expect(link["declared_on"]).toBeNull(); // foreign key never read/written
    expect(link["verified_on"]).toBe(true);
    expect(link["verified_source"]).toBe("api:/api/cartridges/runtime");
  });

  it("(c2) foreign value at the control key (not written by the board) → declared_on null, never guessed", async () => {
    const res = await request(buildHarness({ declaredRaw: "maybe" }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["declared_on"]).toBeNull();
  });

  it("(c5) APPROVED hash wins over a tampered live key — foreign write is drift to expose, never the declaration (§4.2 step 5, sovereignty)", async () => {
    const res = await request(
      buildHarness({
        // operator approved OFF through the board…
        approved: {
          route_scanner_multihop: JSON.stringify({
            on: false,
            reason: "apagar RU-3 para mantenimiento",
            actor: "admin",
            updated_at: "2026-09-07T15:30:00Z",
          }),
        },
        // …but an EXTERNAL actor later SET the toggle key "true". The board
        // must show declared=false (the operator's approval) while verified
        // comes from the live heartbeat (run) — the drift badge fires
        // client-side (declared≠verified), the tamper is EXPOSED not adopted.
        declaredRaw: "true",
        hbRaw: JSON.stringify({ state: "run", block: 13001, ts: "2026-09-07T15:31:00Z" }),
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["declared_on"]).toBe(false); // operator approval — NOT the live "true"
    expect(a["verified_on"]).toBe(true); // live heartbeat (the tampered runtime IS running)
    expect(a["verified_source"]).toBe(`redis:${CLASS_A_HB_KEY}`);
  });

  it("(c6) garbage entry in the approval hash → declared_on null (foreign value never interpreted)", async () => {
    const res = await request(
      buildHarness({ approved: { route_scanner_multihop: "{not json" } }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["declared_on"]).toBeNull();
  });

  it("(c3) LIVE heartbeat run → verified_on true from the hb (R3 — LED = realidad, no foto)", async () => {
    const res = await request(
      buildHarness({
        declaredRaw: "true",
        hbRaw: JSON.stringify({ state: "run", block: 12345, ts: "2026-09-07T15:00:00Z" }),
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["verified_on"]).toBe(true);
    expect(a["verified_source"]).toBe(`redis:${CLASS_A_HB_KEY}`);
    expect(a["verified_at"]).toBe("2026-09-07T15:00:00Z");
  });

  it("(c3b) LIVE heartbeat halted → verified_on false (halt verificado, not unknown)", async () => {
    const res = await request(
      buildHarness({
        declaredRaw: "false",
        hbRaw: JSON.stringify({ state: "halted", block: 12346, ts: "2026-09-07T15:00:12Z" }),
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["verified_on"]).toBe(false);
    expect(a["verified_at"]).toBe("2026-09-07T15:00:12Z");
  });

  it("(c3c) garbage heartbeat JSON → verified_on null (foreign value never interpreted) + last-known verified_at kept", async () => {
    const res = await request(buildHarness({ hbRaw: "{not json" }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["verified_on"]).toBeNull();
    expect(a["verified_at"]).toBe("2026-09-07T12:13:15Z"); // census last-known
  });

  it("(c4) killswitch link row LIVE: enabled=false → verified_on true (detección ACTIVA, inverted projection — R3/R5)", async () => {
    const res = await request(
      buildHarness({
        killswitchRaw: JSON.stringify({
          enabled: false,
          reason: "VER",
          triggered_by: "admin",
          updated_at: "2026-09-07T12:53:29.152Z",
        }),
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    const ks = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "kill_switch",
    )!;
    expect(ks["declared_on"]).toBeNull(); // JSON key ≠ board dialect — R5: null forever
    expect(ks["verified_on"]).toBe(true); // disarmado ⇒ detección ACTIVA
    expect(ks["verified_source"]).toBe("redis:arbx:killswitch");
    expect(ks["verified_at"]).toBe("2026-09-07T12:53:29.152Z");
  });

  it("(c4b) killswitch ARMED (enabled=true) → verified_on false (detención) — live, not the census photo", async () => {
    const res = await request(
      buildHarness({
        killswitchRaw: JSON.stringify({ enabled: true, reason: "HALT", updated_at: "2026-09-07T16:00:00Z" }),
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    const ks = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "kill_switch",
    )!;
    expect(ks["verified_on"]).toBe(false); // census photo said true — OVERRIDDEN by live
    expect(ks["verified_at"]).toBe("2026-09-07T16:00:00Z");
  });

  it("(d) Redis read error → 503 redis_unavailable", async () => {
    const res = await request(buildHarness({ failRedisGet: true }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("(e) corrupted census JSON → loud 503 census_corrupted", async () => {
    const res = await request(buildHarness({ census: "{not json" }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("census_corrupted");
  });

  it("(e2) schema-invalid census (module without module_class) → 503 census_schema_invalid", async () => {
    const bad = JSON.stringify({ modules: [{ id: "x", name: "no class" }] });
    const res = await request(buildHarness({ census: bad }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("census_schema_invalid");
  });

  it("(f) PG unavailable → snapshot still 200 with null audit enrichment (state does not degrade, only quién/por qué)", async () => {
    const res = await request(buildHarness({ poolNull: true }).app)
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["last_reason"]).toBeNull();
    expect(a["last_actor"]).toBeNull();
    expect(a["updated_at"]).toBeNull();
  });

  it("(f2) audit SELECT error → 200 with nulls (fail-honest enrichment)", async () => {
    const res = await request(
      buildHarness({
        poolImpl: async () => {
          throw new Error("pg down");
        },
      }).app,
    )
      .get("/api/v1/control-board")
      .set(...AUTH);
    expect(res.status).toBe(200);
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["last_actor"]).toBeNull();
  });
});

// ─── PUT /api/v1/control-board ──────────────────────────────────────────────

describe("CB-02 control-board PUT (sovereignty gates)", () => {
  it("(a) without admin token → 401", async () => {
    const res = await request(buildHarness({}).app)
      .put("/api/v1/control-board")
      .send({ id: "route_scanner_multihop", on: true, reason: "x" });
    expect(res.status).toBe(401);
  });

  it("(d) invalid body — missing `on` → 400", async () => {
    const res = await request(buildHarness({}).app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", reason: "falta on" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_body");
  });

  it("(d2) whitespace-only reason → 400 (server-side trim validation)", async () => {
    const res = await request(buildHarness({}).app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "   " });
    expect(res.status).toBe(400);
  });

  it("(b) census absent → 503 fail-safe deny, NO audit row, NO redis write", async () => {
    const h = buildHarness({ census: null });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender" });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("census_not_published");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(e) class C → 403 module_class_c_locked, no audit, no redis write", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "terminus_paper_live", on: true, reason: "intentar live" });
    expect(res.status).toBe(403);
    expect(res.body.error).toBe("module_class_c_locked");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(e2) §34.3 terminus mislabeled class A by census mistake → 403 terminus_denied_c343 anyway (defense in depth)", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "exec_flipping", on: true, reason: "census lo etiquetó A" });
    expect(res.status).toBe(403);
    expect(res.body.error).toBe("terminus_denied_c343");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(f) class B → 409 restart_required, no audit, no redis write (CB-05 owns class B)", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "scoring_hard_gate", on: true, reason: "endurecer" });
    expect(res.status).toBe(409);
    expect(res.body.error).toBe("module_class_b_restart_required");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(g) unknown module id → 404 module_not_in_census", async () => {
    const res = await request(buildHarness({}).app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "no_existe", on: true, reason: "x" });
    expect(res.status).toBe(404);
    expect(res.body.error).toBe("module_not_in_census");
  });

  it("(h) class A without control_key (census defect) → 503 census_invalid_class_a, no write", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "broken_class_a", on: true, reason: "x" });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("census_invalid_class_a");
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(i) class A with a FOREIGN control_key (killswitch JSON — cross-exam G4) → 503 control_key_not_board_writable, no audit, no write", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "foreign_key_class_a", on: true, reason: "sobrescribir clave ajena" });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("control_key_not_board_writable");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(i2) killswitch LINK row is never board-writable either (R5 — toggle lives at /admin/killswitch)", async () => {
    const h = buildHarness({});
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "kill_switch", on: false, reason: "parar detección" });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("control_key_not_board_writable");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(n) §4.3/GATE-CB02-1: route_scanner census declared_on:false (worker NOT spawned) → 409 module_not_spawned, no audit, no writes", async () => {
    // CB-02 (2026-09-07) — future census dialect: the scanner as class A
    // with the board toggle key, but the boot/spawn state is OFF (the
    // worker was never spawned — ARBX_ROUTE_SCANNER_MODE=off).
    const census = JSON.stringify({
      modules: CENSUS.modules.map((m) =>
        m.id === "route_scanner_multihop" ? { ...m, declared_on: false } : m,
      ),
    });
    const h = buildHarness({ census });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender sin worker" });
    expect(res.status).toBe(409);
    expect(res.body.error).toBe("module_not_spawned");
    expect(String(res.body.detail)).toContain("CB-05");
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
    expect(h.redisHset).not.toHaveBeenCalled();
  });

  it("(n2) declared_on:true (worker spawned) does NOT trigger the gate — the toggle proceeds", async () => {
    const census = JSON.stringify({
      modules: CENSUS.modules.map((m) =>
        m.id === "route_scanner_multihop" ? { ...m, declared_on: true } : m,
      ),
    });
    const h = buildHarness({ census });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender RU-3 ya spawneado" });
    expect(res.status).toBe(200);
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
    expect(h.redisHset).toHaveBeenCalledTimes(1);
  });

  it("(j0) PG unavailable → 503 db_unavailable: without audit there is NO toggle", async () => {
    const h = buildHarness({ poolNull: true });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender RU-3" });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(j) happy path: audit_log INSERT (011 columns) BEFORE redis SET, idempotent \"true\"/\"false\" write, fresh snapshot in response", async () => {
    const h = buildHarness({ declaredRaw: "false" });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender route_scanner para validar hops 2-6" });

    expect(res.status).toBe(200);
    // Idempotent board format, exact RESOLVED key from the census.
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
    // (g2) §4.3: the approval-registry HSET rides along — field per module_id
    // carrying {on, reason, actor, updated_at} (the GET's declared source).
    expect(h.redisHset).toHaveBeenCalledTimes(1);
    expect(h.redisHset.mock.calls[0]![0]).toBe(CONTROL_BOARD_APPROVED_HASH);
    expect(h.redisHset.mock.calls[0]![1]).toBe("route_scanner_multihop");
    const approved = JSON.parse(String(h.redisHset.mock.calls[0]![2])) as Record<string, unknown>;
    expect(approved["on"]).toBe(true);
    expect(approved["reason"]).toBe("encender route_scanner para validar hops 2-6");
    expect(approved["actor"]).toBe("admin");
    expect(typeof approved["updated_at"]).toBe("string");
    // THE charter ordering: audit_log INSERT strictly before BOTH Redis writes
    // (SET then HSET — INV-CB02-2 audit-first).
    expect(h.poolQuery.mock.invocationCallOrder[0]).toBeLessThan(
      h.redisSet.mock.invocationCallOrder[0],
    );
    expect(h.redisSet.mock.invocationCallOrder[0]).toBeLessThan(
      h.redisHset.mock.invocationCallOrder[0],
    );
    // CB-02-DISENO §15-R1 row shape: audit_log (singular), target_kind
    // 'control_board_module', before_state/after_state JSONB — no RETURNING.
    const insertCall = h.poolQuery.mock.calls.find(([t]) =>
      String(t).startsWith("INSERT INTO audit_log"),
    )!;
    expect(String(insertCall[0])).toContain("target_kind");
    expect(String(insertCall[0])).not.toContain("RETURNING");
    expect(insertCall[1]).toEqual([
      "admin",
      CONTROL_BOARD_TOGGLE_AUDIT_ACTION,
      "route_scanner_multihop",
      JSON.stringify({ on: false }), // before_state captured from the live key
      expect.stringContaining("encender route_scanner para validar hops 2-6"), // after_state.reason
    ]);
    const afterState = JSON.parse(String(insertCall[1][4])) as Record<string, unknown>;
    expect(afterState["on"]).toBe(true);
    expect(afterState["applied"]).toBe(true);
    expect(afterState["control_key"]).toBe(CLASS_A_KEY);
    // Response is the FRESH snapshot — CB-03 re-renders from it directly.
    const a = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(a["declared_on"]).toBe(true);
  });

  it("(k) audit INSERT fails → 500 audit_write_failed and Redis is NEVER written (audit-first gate)", async () => {
    const h = buildHarness({
      poolImpl: async () => {
        throw new Error("pg insert refused");
      },
    });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender" });
    expect(res.status).toBe(500);
    expect(res.body.error).toBe("audit_write_failed");
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(l) Redis SET fails → 500 redis_write_failed + COMPENSATORY INSERT toggle_failed (applied:false) — never an UPDATE (011:21 append-only); approval HSET never runs", async () => {
    const h = buildHarness({ failRedisSet: true });
    const res = await request(h.app)
      .put("/api/v1/control-board")
      .set(...AUTH)
      .send({ id: "route_scanner_multihop", on: true, reason: "encender" });
    expect(res.status).toBe(500);
    expect(res.body.error).toBe("redis_write_failed");
    // The approval HSET is sequential AFTER the SET: a failed SET means no
    // approval registry write either (no partial approval ever recorded).
    expect(h.redisHset).not.toHaveBeenCalled();

    // The second pool call must be the R1 compensatory INSERT — the ledger is
    // append-only, so the original row is NEVER corrected via UPDATE.
    const insertCalls = h.poolQuery.mock.calls.filter(([t]) =>
      String(t).startsWith("INSERT INTO audit_log"),
    );
    expect(insertCalls).toHaveLength(2);
    const [toggleCall, failedCall] = insertCalls as unknown as Array<[string, unknown[]]>;
    expect(toggleCall[1][1]).toBe(CONTROL_BOARD_TOGGLE_AUDIT_ACTION);
    expect(failedCall[1][1]).toBe(CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION);
    expect(failedCall[1][2]).toBe("route_scanner_multihop");
    const failedAfter = JSON.parse(String(failedCall[1][4])) as Record<string, unknown>;
    expect(failedAfter["applied"]).toBe(false);
    expect(String(failedAfter["redis_error"])).toContain("redis write refused");
    expect(String(failedAfter["reason"])).toBe("encender");
    const updateCall = h.poolQuery.mock.calls.find(([t]) => String(t).startsWith("UPDATE"));
    expect(updateCall).toBeUndefined();
  });
});
