import { describe, it, expect, beforeAll } from "vitest";
import { GenericContainer, StartedTestContainer } from "testcontainers";
import { Pool } from "pg";
import { readFileSync } from "node:fs";
import path from "node:path";

let container: StartedTestContainer;
let pool: Pool;

beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "test" })
    .withExposedPorts(5432)
    .start();
  pool = new Pool({
    host: container.getHost(),
    port: container.getMappedPort(5432),
    user: "postgres", password: "test", database: "postgres",
  });
  // Apply prereqs (003_opportunities + role 001_roles).
  for (const f of ["001_roles.sql", "003_opportunities.sql"]) {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations", f), "utf8");
    await pool.query(sql);
  }
}, 60_000);

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

describe("migration 034", () => {
  it("creates tokens table with correct schema", async () => {
    const sql = readFileSync(path.join(__dirname, "../../../database/migrations/034_tokens_table.sql"), "utf8");
    await pool.query(sql);
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
