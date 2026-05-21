import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { GenericContainer, StartedTestContainer } from "testcontainers";
import { Pool } from "pg";
import { readFileSync } from "node:fs";
import path from "node:path";

let container: StartedTestContainer;
let pool: Pool;

beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({
      POSTGRES_PASSWORD: "test",
      POSTGRES_DB: "arbitragex",
    })
    .withExposedPorts(5432)
    .start();
  pool = new Pool({
    host: container.getHost(),
    port: container.getMappedPort(5432),
    user: "postgres", password: "test", database: "arbitragex",
  });
  // Apply prereqs in numeric order. 021 creates the tokens table (+ chains/
  // dexes/etc.); migration 034 (applied below) only ALTERs tokens, so 021 must
  // run first or 034 fails with "relation tokens does not exist".
  for (const f of ["001_roles.sql", "003_opportunities.sql", "021_defi_registries.sql"]) {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations", f), "utf8");
    await pool.query(sql);
  }
}, 60_000);

afterAll(async () => {
  await pool.end();
  await container.stop();
}, 30_000);

describe("migration 033", () => {
  it("drops NOT NULL on expected_profit_usd, roi_pct, risk_score", async () => {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations/033_opportunities_fail_honest_and_cross_chain_slots.sql"), "utf8");
    await pool.query(sql);
    const r = await pool.query(`
      SELECT column_name, is_nullable FROM information_schema.columns
      WHERE table_name = 'opportunities'
        AND column_name IN ('expected_profit_usd','roi_pct','risk_score')`);
    for (const row of r.rows) expect(row.is_nullable).toBe("YES");
  });

  it("adds cross-chain columns as NULLABLE", async () => {
    const r = await pool.query(`
      SELECT column_name, is_nullable FROM information_schema.columns
      WHERE table_name = 'opportunities'
        AND column_name IN ('chain_id_out','bridge','bridge_fee_usd')`);
    expect(r.rows.length).toBe(3);
    for (const row of r.rows) expect(row.is_nullable).toBe("YES");
  });

  it("rejects chain_id_out equal to chain_id (chk_cross_chain_distinct)", async () => {
    await expect(pool.query(`
      INSERT INTO opportunities (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id, chain_id_out)
      VALUES (1, 'dex_arb', 'uniswap-v2', '0x' || repeat('a', 40), '0x' || repeat('b', 40), 0, gen_random_uuid(), 1)
    `)).rejects.toThrow(/chk_cross_chain_distinct/);
  });

  it("is idempotent on rerun", async () => {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations/033_opportunities_fail_honest_and_cross_chain_slots.sql"), "utf8");
    await expect(pool.query(sql)).resolves.toBeTruthy();
  });
});

// NOTE: requires migration 033 from previous describe to have run first
describe("migration 034", () => {
  it("creates tokens table with correct schema", async () => {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations/034_tokens_table.sql"), "utf8");
    await pool.query(sql);
    // 072 reconciles the tokens schema to the discovery-first model (symbol/
    // decimals nullable, chain_id NOT NULL, resolved_via NOT NULL, lowercase
    // chk_address_format). Applied here so the schema assertions below reflect
    // the full, canonical tokens migration chain — not an artificial state.
    const reconcile = readFileSync(path.join(__dirname, "../../../database/migrations/072_reconcile_tokens_schema.sql"), "utf8");
    await pool.query(reconcile);
    const r = await pool.query(`
      SELECT column_name, data_type, is_nullable FROM information_schema.columns
      WHERE table_name = 'tokens'
      ORDER BY ordinal_position`);
    expect(r.rows.length).toBeGreaterThanOrEqual(8);
    const cols = Object.fromEntries(r.rows.map(c => [c.column_name, c]));
    expect(cols.chain_id.is_nullable).toBe("NO");
    expect(cols.address.is_nullable).toBe("NO");
    expect(cols.symbol.is_nullable).toBe("YES");
    expect(cols.decimals.is_nullable).toBe("YES");
    expect(cols.logo_url.is_nullable).toBe("YES");
    expect(cols.resolved_via.is_nullable).toBe("NO");
  });

  it("rejects non-lowercase address (chk_address_format)", async () => {
    await expect(pool.query(`
      INSERT INTO tokens (chain_id, address, resolved_via)
      VALUES (1, '0xCAFEbabeCAFEbabeCAFEbabeCAFEbabeCAFEbabe', 'failed')
    `)).rejects.toThrow(/chk_address_format/);
  });

  it("rejects invalid resolved_via value", async () => {
    await expect(pool.query(`
      INSERT INTO tokens (chain_id, address, resolved_via)
      VALUES (1, '0x' || repeat('c', 40), 'invalid_kind')
    `)).rejects.toThrow();
  });

  it("PRIMARY KEY (chain_id, address) prevents duplicates", async () => {
    const addr = "0x" + "d".repeat(40);
    await pool.query(`INSERT INTO tokens (chain_id, address, resolved_via) VALUES (1, $1, 'failed')`, [addr]);
    await expect(pool.query(`INSERT INTO tokens (chain_id, address, resolved_via) VALUES (1, $1, 'failed')`, [addr]))
      .rejects.toThrow(/duplicate key/);
  });
});

// =====================================================================
// OMEGA-8 / M3 — Migration 069: runtime_ack idempotency UNIQUE + CHECKs
// =====================================================================
// Prereqs: migration 067 must be applied (creates the runtime_ack table).
// This block applies 067 first (idempotent) and then 069.
describe("migration 069 (OMEGA-8/M3 P1-1)", () => {
  beforeAll(async () => {
    // Apply 067 so runtime_ack exists. The file is large but idempotent.
    const sql067 = readFileSync(
      path.join(__dirname, "../../../database/migrations/067_config_hash_registry_drift_runtime_ack.sql"),
      "utf8",
    );
    await pool.query(sql067);
    const sql069 = readFileSync(
      path.join(__dirname, "../../../database/migrations/069_runtime_ack_idempotency_unique.sql"),
      "utf8",
    );
    await pool.query(sql069);
  });

  it("creates UNIQUE constraint on (event_id, layer)", async () => {
    const r = await pool.query(`
      SELECT conname FROM pg_constraint WHERE conname = 'runtime_ack_event_layer_unique'`);
    expect(r.rowCount).toBe(1);
  });

  it("creates CHECK constraint runtime_ack_chain_id_positive", async () => {
    const r = await pool.query(`
      SELECT conname FROM pg_constraint WHERE conname = 'runtime_ack_chain_id_positive'`);
    expect(r.rowCount).toBe(1);
  });

  it("creates idx_runtime_ack_failures partial index", async () => {
    const r = await pool.query(`
      SELECT indexname FROM pg_indexes WHERE indexname = 'idx_runtime_ack_failures'`);
    expect(r.rowCount).toBe(1);
  });

  it("DB rejects duplicate INSERT on (event_id, layer)", async () => {
    const eid = "44444444-4444-4444-4444-444444444444";
    await pool.query(
      `INSERT INTO runtime_ack
       (event_id, resource, chain_id, idempotency_key, config_hash_after,
        worker_id, layer, status)
       VALUES ($1, 'trading_config', 1, 'k1', repeat('a', 64), 'w1',
               'searcher_rs', 'applied')`,
      [eid],
    );
    await expect(pool.query(
      `INSERT INTO runtime_ack
       (event_id, resource, chain_id, idempotency_key, config_hash_after,
        worker_id, layer, status)
       VALUES ($1, 'trading_config', 1, 'k2', repeat('b', 64), 'w2',
               'searcher_rs', 'applied')`,
      [eid],
    )).rejects.toThrow(/runtime_ack_event_layer_unique|duplicate key/);
  });

  it("DB rejects negative chain_id", async () => {
    const eid = "55555555-5555-5555-5555-555555555555";
    await expect(pool.query(
      `INSERT INTO runtime_ack
       (event_id, resource, chain_id, idempotency_key, config_hash_after,
        worker_id, layer, status)
       VALUES ($1, 'trading_config', -1, 'k', repeat('c', 64), 'w',
               'searcher_rs', 'applied')`,
      [eid],
    )).rejects.toThrow(/runtime_ack_chain_id_positive/);
  });

  it("ON CONFLICT (event_id, layer) DO NOTHING returns 0 rows on duplicate", async () => {
    const eid = "66666666-6666-6666-6666-666666666666";
    const first = await pool.query(
      `INSERT INTO runtime_ack
       (event_id, resource, chain_id, idempotency_key, config_hash_after,
        worker_id, layer, status)
       VALUES ($1, 'trading_config', 1, 'k1', repeat('a', 64), 'w1',
               'searcher_rs', 'applied')
       ON CONFLICT (event_id, layer) DO NOTHING
       RETURNING id`,
      [eid],
    );
    expect(first.rowCount).toBe(1);
    const second = await pool.query(
      `INSERT INTO runtime_ack
       (event_id, resource, chain_id, idempotency_key, config_hash_after,
        worker_id, layer, status)
       VALUES ($1, 'trading_config', 1, 'k2', repeat('b', 64), 'w2',
               'searcher_rs', 'applied')
       ON CONFLICT (event_id, layer) DO NOTHING
       RETURNING id`,
      [eid],
    );
    expect(second.rowCount).toBe(0);
  });

  it("migration 069 is idempotent (re-running succeeds)", async () => {
    const sql069 = readFileSync(
      path.join(__dirname, "../../../database/migrations/069_runtime_ack_idempotency_unique.sql"),
      "utf8",
    );
    await expect(pool.query(sql069)).resolves.toBeTruthy();
  });
});
