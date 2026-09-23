// FRONT-01 regression proof (t_26eaafc3, triage t_83ba95b0 §3.3) —
// empirically clamps the WS NOTIFY payload shape for `amount_in_wei`.
//
// Halling (workspace-extreme-audit-2026-09-24): row_to_json(NEW) serializes
// NUMERIC(78,0) as an UNQUOTED JSON number; the api-server relays the NOTIFY
// payload to the `opportunities` WS room verbatim (websocket.ts
// broadcastOpportunity) and the browser JSON.parse turns it into a float64,
// silently corrupting any wei value above 2^53 (1234567890123456789 ->
// 1234567890123456800, -11 wei).
//
// Canonical fix: migration 124 (jsonb_set single, plain string). This test
// applies the FULL 025→124 sequence on a throwaway Postgres and asserts the
// bytes that actually cross the wire — including the jsonb_set-on-scalar
// semantics that the triage quoted from the PostgreSQL 17 docs ("All earlier
// steps in the path must exist, or the target is returned unchanged").
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { GenericContainer, StartedTestContainer, Wait } from "testcontainers";
import { Pool } from "pg";
import { readFileSync } from "node:fs";
import path from "node:path";

const MIG_DIR = path.join(__dirname, "..", "..", "..", "database", "migrations");
const MIG_025 = "025_opportunities_websocket_trigger.sql";
const MIG_124 = "124_opportunities_ws_trigger_amount_text.sql";

const MAX_SAFE = 2 ** 53; // 9007199254740992 — float64 exact-integer ceiling
const EDGE_2P53_PLUS = "1234567890123456789"; // > 2^53, the audit's repro value
const EDGE_78_DIGITS =
  "999999999999999999999999999999999999999999999999999999999999999999999999999999"; // 78×9

let container: StartedTestContainer;
let pool: Pool;

beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "test", POSTGRES_DB: "arbitragex" })
    .withExposedPorts(5432)
    // Postgres logs "ready to accept connections" TWICE (bootstrap temp server,
    // then the real one). Matching the 1st caused `read ECONNRESET` — wait for
    // the 2nd occurrence (same rationale as migrations.test.ts).
    .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2))
    .start();
  pool = new Pool({
    host: container.getHost(),
    port: container.getMappedPort(5432),
    user: "postgres",
    password: "test",
    database: "arbitragex",
  });
  // Prereq table (same sequence as migrations.test.ts).
  await pool.query(
    readFileSync(path.join(MIG_DIR, "003_opportunities.sql"), "utf8"),
  );
}, 120_000);

afterAll(async () => {
  if (pool) await pool.end();
  if (container) await container.stop();
}, 30_000);

/** Applies a migration file and drains every NOTIFY delivered on
 *  `opportunities_channel` while `insertRow` runs. Returns the raw payloads. */
async function insertAndCollectNotifications(
  amountInWei: string,
): Promise<string[]> {
  const client = await pool.connect();
  const notifications: string[] = [];
  const onNotification = (msg: { payload?: string }) => {
    if (msg.payload) notifications.push(msg.payload);
  };
  try {
    client.on("notification", onNotification);
    await client.query("LISTEN opportunities_channel");
    await client.query(
      `INSERT INTO opportunities
         (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
       VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, $3::numeric, gen_random_uuid())`,
      ["0x" + "ab".repeat(20), "0x" + "cd".repeat(20), amountInWei],
    );
    // The self-NOTIFY arrives asynchronously on this same connection; node-pg
    // emits 'notification' as soon as the socket drains (no query needed).
    const deadline = Date.now() + 5_000;
    while (notifications.length === 0 && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return notifications;
  } finally {
    client.removeListener("notification", onNotification);
    await client.query("UNLISTEN *").catch(() => {});
    client.release();
  }
}

describe("FRONT-01: migration 124 — amount_in_wei crosses WS as exact text", () => {
  it("applies 025 then 124 cleanly (full trigger chain)", async () => {
    await pool.query(readFileSync(path.join(MIG_DIR, MIG_025), "utf8"));
    await pool.query(readFileSync(path.join(MIG_DIR, MIG_124), "utf8"));
  });

  it.each([
    ["1234567890123456789", "audit repro value (> 2^53)"],
    [EDGE_78_DIGITS, "78-digit NUMERIC(78,0) boundary"],
    [String(MAX_SAFE - 1), "just under 2^53"],
  ])(
    "NOTIFY serializes amount_in_wei=%s as exact plain string (%s)",
    async (amount, _label) => {
      const payloads = await insertAndCollectNotifications(amount);
      expect(payloads.length).toBeGreaterThan(0);
      const payload = JSON.parse(payloads[payloads.length - 1]);
      // (a) typeof string
      expect(typeof payload.amount_in_wei).toBe("string");
      // (b) byte-exact, no exponent, no float roundtrip
      expect(payload.amount_in_wei).toBe(amount);
      expect(payload.amount_in_wei).not.toMatch(/[eE.]/);
      // (d) not the nested-object shape of the divergent double jsonb_set
      expect(payload.amount_in_wei).not.toEqual(
        expect.objectContaining({ text: expect.anything() }),
      );
      // (c) no field of the payload is a number ≥ 2^53
      for (const [k, v] of Object.entries(payload)) {
        if (typeof v === "number") {
          expect(Math.abs(v)).toBeLessThan(MAX_SAFE);
        }
      }
    },
  );

  it("re-running 124 is safe (idempotent trigger tail, catalog guard)", async () => {
    const sql = readFileSync(path.join(MIG_DIR, MIG_124), "utf8");
    await expect(pool.query(sql)).resolves.toBeTruthy();
    // The trigger still fires with the same body (CREATE OR REPLACE preserves
    // the OID; the guarded tail did NOT recreate it).
    const payloads = await insertAndCollectNotifications(EDGE_2P53_PLUS);
    expect(payloads.length).toBeGreaterThan(0);
    expect(JSON.parse(payloads[payloads.length - 1]).amount_in_wei).toBe(
      EDGE_2P53_PLUS,
    );
  });

  it("trigger count is exactly one after the 025→124→124 sequence", async () => {
    const r = await pool.query(
      `SELECT count(*)::int AS n FROM pg_trigger
       WHERE tgname = 'trg_notify_opportunity'
         AND tgrelid = 'public.opportunities'::regclass`,
    );
    expect(r.rows[0].n).toBe(1);
  });
});
