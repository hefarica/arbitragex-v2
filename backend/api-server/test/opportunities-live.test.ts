/**
 * Task 9 — Integration tests for GET /api/v1/opportunities/live
 *
 * Uses testcontainers (postgres:15) to run against a real PG instance.
 * Migration loading order mirrors migrations.test.ts:
 *   001_roles.sql       — roles + GRANT CONNECT (works because POSTGRES_DB=arbitragex)
 *   003_opportunities   — core table + rejection_reason column
 *   033_*               — fail-honest nullable + cross-chain slots
 *   034_tokens_table    — tokens registry for LEFT JOIN enrichment
 *
 * Tests (6):
 *   1. token_in_info is null when no tokens row exists
 *   2. token_in_info is populated when tokens row exists
 *   3. expected_profit_usd is null (R8 fail-honest — not coerced to 0)
 *   4. cross-chain row: chain_id_out present, bridge/bridge_fee_usd null
 *   5. HTTP 503 when pool is null (db_unavailable)
 *   6. viable_only=true excludes rejected rows; viable_only=false (default) returns them
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { GenericContainer, type StartedTestContainer, Wait } from "testcontainers";
import { Pool } from "pg";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import express from "express";
import request from "supertest";
import { mountOpportunitiesLive } from "../src/routes/opportunities-live.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const MIGRATIONS_DIR = path.join(__dirname, "../../../database/migrations");

// ── Helpers ──────────────────────────────────────────────────────────────────

function loadMigration(filename: string): string {
  return readFileSync(path.join(MIGRATIONS_DIR, filename), "utf8");
}

function silentLogger() {
  return { warn: (_obj: object, _msg?: string) => {} };
}

// ── Container + pool setup ───────────────────────────────────────────────────

let container: StartedTestContainer;
let pool: Pool;

beforeAll(async () => {
  // POSTGRES_DB=arbitragex is required so that 001_roles.sql's
  //   GRANT CONNECT ON DATABASE arbitragex
  // succeeds. This mirrors the pattern from migrations.test.ts.
  container = await new GenericContainer("postgres:15")
    .withEnvironment({
      POSTGRES_PASSWORD: "test",
      POSTGRES_DB:       "arbitragex",
    })
    .withExposedPorts(5432)
    // Postgres logs "ready to accept connections" TWICE (bootstrap temp server,
    // then the real one). Matching the 1st caused `read ECONNRESET` when PG
    // restarted under the just-opened pool. Wait for the 2nd occurrence.
    .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2))
    .start();

  pool = new Pool({
    host:     container.getHost(),
    port:     container.getMappedPort(5432),
    user:     "postgres",
    password: "test",
    database: "arbitragex",
  });

  // Apply migrations in dependency order.
  for (const f of [
    "001_roles.sql",
    "003_opportunities.sql",
    // 021 creates the tokens table (+ chains/dexes/etc.); 034 only ALTERs it,
    // so 021 must be applied first or 034 fails with "relation tokens does not exist".
    "021_defi_registries.sql",
    "033_opportunities_fail_honest_and_cross_chain_slots.sql",
    "034_tokens_table.sql",
    // 049 adds opportunities.net_expected_profit_usd, which the live route
    // SELECTs; without it the endpoint query errors and returns 503.
    "049_h2_net_expected_profit.sql",
    // 072 reconciles tokens to the discovery-first model: symbol/decimals
    // nullable, chain_id required, lowercase-address check. Without it, a
    // discovered-token insert (address + resolved_via only) fails on the
    // legacy symbol NOT NULL from 021.
        "072_reconcile_tokens_schema.sql",
    // 099 adds opportunities.route_metadata (JSONB): the live route SELECTs it
    // (multi-leg A→B, PR hardening 2026-08-11); without this migration the
    // testcontainer schema lacks the column and EVERY GET returns 503
    // query_failed ("column route_metadata does not exist"). This was the
    // persistent "flaky" third member of the CI trio — not flaky at all:
    // deterministic breakage latent since the route started selecting the
    // column, surfaced on every PR run (CI-GATE-RELIABILITY part 3).
    "099_opportunities_route_metadata.sql",
    // 102 formalizes opportunities.cartridge_id (existed in PROD only via a
    // manual ALTER — schema drift; the live route SELECTs it). Same drift
    // family as 099 above: without it the testcontainer query fails 503.
    "102_opportunities_cartridge_id.sql",
  ]) {
    await pool.query(loadMigration(f));
  }
  // tokens.chain_id has a FK to chains(chain_id) (migration 021). The token
  // enrichment rows below use chain_id=1, so seed the chain registry row.
  await pool.query(
    `INSERT INTO chains (chain_id, name, native_currency)
     VALUES (1, 'Ethereum', 'ETH') ON CONFLICT (chain_id) DO NOTHING`,
  );
}, 60_000);

afterAll(async () => {
  await pool.end();
  await container.stop();
}, 30_000);

// ── Factories ────────────────────────────────────────────────────────────────

/**
 * Builds a minimal Express app that only mounts the route under test.
 * Keeps each test group independent of global state.
 */
function buildApp(overridePool: Pool | null = pool) {
  const app = express();
  // 2026-05-10: target-driven simulation wiring added a `redis` parameter so
  // the route can load `arbx:trading_config:<chain>` snapshots per-row. These
  // integration tests don't exercise the simulation path (no Redis instance
  // in the testcontainers setup), so pass null — the route correctly skips
  // simulation when redis is null (R8 fail-honest, simulated_* fields stay
  // null in the wire shape).
  mountOpportunitiesLive(app, overridePool, null, silentLogger());
  return app;
}

// ── Test: token_in_info null when no tokens row ──────────────────────────────

describe("GET /api/v1/opportunities/live — token enrichment", () => {
  it("token_in_info surfaces UNVERIFIED when no tokens row exists", async () => {
    // 2026-05-11 contract change: tokenInfoFromRow no longer returns null
    // when DB JOIN misses. It always emits a TokenInfoResult so the curated-
    // registry verification (verified=false here, since 0xaaa...aaa is not
    // in the Uniswap default list) reaches the dashboard. Operator never
    // sees an unmarked unknown token.
    const tokenIn  = "0x" + "a".repeat(40);
    const tokenOut = "0x" + "b".repeat(40);
    await pool.query(`
      INSERT INTO opportunities
        (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
      VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, 1000000, gen_random_uuid())
    `, [tokenIn, tokenOut]);

    const res = await request(buildApp())
      .get("/api/v1/opportunities/live?limit=200")
      .expect(200);

    expect(res.body.count).toBeGreaterThanOrEqual(1);
    // Find the row we just inserted (by token address).
    const item = res.body.items.find((o: any) => o.token_in === tokenIn);
    expect(item).toBeDefined();
    // R8 fail-honest: no tokens row → metadata fields stay null, but the
    // verification block IS populated so the operator sees ⚠ UNVERIFIED.
    expect(item.token_in_info).not.toBeNull();
    expect(item.token_in_info.symbol).toBeNull();
    expect(item.token_in_info.verified).toBe(false);
    expect(item.token_in_info.verified_notes).toContain("address-not-in-registry");
    expect(item.token_out_info).not.toBeNull();
    expect(item.token_out_info.verified).toBe(false);
  });

  it("token_in_info is populated when tokens row exists", async () => {
    const tokenIn  = "0x" + "c".repeat(40);
    const tokenOut = "0x" + "d".repeat(40);

    // Insert token enrichment data first.
    await pool.query(`
      INSERT INTO tokens (chain_id, address, symbol, decimals, logo_url, resolved_via)
      VALUES (1, $1, 'WETH', 18, 'https://example.com/weth.png', 'onchain_full')
    `, [tokenIn]);

    // Insert opportunity referencing the token above.
    await pool.query(`
      INSERT INTO opportunities
        (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
      VALUES (1, 'dex_arb', 'sushiswap', $1, $2, 2000000, gen_random_uuid())
    `, [tokenIn, tokenOut]);

    const res = await request(buildApp())
      .get("/api/v1/opportunities/live?limit=200")
      .expect(200);

    const item = res.body.items.find((o: any) => o.token_in === tokenIn);
    expect(item).toBeDefined();
    expect(item.token_in_info).not.toBeNull();
    expect(item.token_in_info.symbol).toBe("WETH");
    expect(item.token_in_info.decimals).toBe(18);
    expect(item.token_in_info.logo_url).toBe("https://example.com/weth.png");
    expect(item.token_in_info.resolved_via).toBe("onchain_full");
    // token_out still has no row in tokens → metadata null but block exists
    // with verified=false so the dashboard renders ⚠ UNVERIFIED.
    expect(item.token_out_info).not.toBeNull();
    expect(item.token_out_info.symbol).toBeNull();
    expect(item.token_out_info.verified).toBe(false);
  });
});

// ── Test: R8 fail-honest — null profit ───────────────────────────────────────

describe("GET /api/v1/opportunities/live — fail-honest nulls", () => {
  it("expected_profit_usd is null (not 0) when not set", async () => {
    const tokenIn  = "0x" + "e".repeat(40);
    const tokenOut = "0x" + "f".repeat(40);

    // Insert with no expected_profit_usd (migration 033 makes it nullable).
    await pool.query(`
      INSERT INTO opportunities
        (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
      VALUES (1, 'triangular', 'curve', $1, $2, 500000, gen_random_uuid())
    `, [tokenIn, tokenOut]);

    const res = await request(buildApp())
      .get("/api/v1/opportunities/live?limit=200")
      .expect(200);

    const item = res.body.items.find((o: any) => o.token_in === tokenIn);
    expect(item).toBeDefined();
    // Must be null, not 0. R8: NULL means "not computed yet".
    expect(item.expected_profit_usd).toBeNull();
    expect(item.roi_pct).toBeNull();
    expect(item.risk_score).toBeNull();
  });
});

// ── Test: cross-chain row ─────────────────────────────────────────────────────

describe("GET /api/v1/opportunities/live — cross-chain slots", () => {
  it("cross-chain row: chain_id_out present, bridge/bridge_fee_usd null", async () => {
    const tokenIn  = "0x1" + "1".repeat(39);
    const tokenOut = "0x2" + "2".repeat(39);

    await pool.query(`
      INSERT INTO opportunities
        (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id, chain_id_out)
      VALUES (1, 'dex_arb', 'uniswap-v3', $1, $2, 999999, gen_random_uuid(), 42161)
    `, [tokenIn, tokenOut]);

    const res = await request(buildApp())
      .get("/api/v1/opportunities/live?limit=200")
      .expect(200);

    const item = res.body.items.find((o: any) => o.token_in === tokenIn);
    expect(item).toBeDefined();
    expect(item.chain_id_out).toBe(42161);
    expect(item.bridge).toBeNull();
    expect(item.bridge_fee_usd).toBeNull();
  });
});

// ── Test: 503 when pool is null ───────────────────────────────────────────────

describe("GET /api/v1/opportunities/live — db_unavailable", () => {
  it("returns HTTP 503 with db_unavailable when pool is null", async () => {
    const appNoDb = buildApp(null);
    const res = await request(appNoDb)
      .get("/api/v1/opportunities/live")
      .expect(503);

    expect(res.body.error).toBe("db_unavailable");
  });
});

// ── Test: viable_only filter (regression for commit 97ffc52) ─────────────────

describe("GET /api/v1/opportunities/live — viable_only filter", () => {
  it("viable_only=true excludes rejected rows; default=false returns them (CARDS-MIRROR-01)", async () => {
    const tokenIn  = "0x" + "e".repeat(39) + "1";
    const tokenOut = "0x" + "f".repeat(39) + "1";

    // Insert an opportunity that has been gate-rejected (rejection_reason set).
    await pool.query(`
      INSERT INTO opportunities
        (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id, rejection_reason)
      VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, 100, gen_random_uuid(), 'TokenNotAllowed')
    `, [tokenIn, tokenOut]);

    // viable_only=true must NOT return the rejected row (opt-in filter).
    const resViable = await request(buildApp())
      .get("/api/v1/opportunities/live?viable_only=true&limit=200")
      .expect(200);
    expect(resViable.body.viable_only).toBe(true);
    const notPresent = resViable.body.items.find(
      (o: any) => o.token_in === tokenIn && o.rejection_reason === "TokenNotAllowed",
    );
    expect(notPresent).toBeUndefined();

    // viable_only=false MUST return the rejected row.
    const resAll = await request(buildApp())
      .get("/api/v1/opportunities/live?viable_only=false&limit=200")
      .expect(200);
    expect(resAll.body.viable_only).toBe(false);
    const present = resAll.body.items.find(
      (o: any) => o.token_in === tokenIn && o.rejection_reason === "TokenNotAllowed",
    );
    expect(present).toBeDefined();
    expect(present.rejection_reason).toBe("TokenNotAllowed");

    // Default (no param) behaves identically to viable_only=false — cards must
    // show ALL recent opps including rejected ones, never hide them (directive
    // 2026-08-22 CARDS-MIRROR-01: cards mirror paper history).
    const resDefault = await request(buildApp())
      .get("/api/v1/opportunities/live?limit=200")
      .expect(200);
    expect(resDefault.body.viable_only).toBe(false);
    const presentDefault = resDefault.body.items.find(
      (o: any) => o.token_in === tokenIn && o.rejection_reason === "TokenNotAllowed",
    );
    expect(presentDefault).toBeDefined();
    expect(presentDefault.rejection_reason).toBe("TokenNotAllowed");
  });
});
