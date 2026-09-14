/** Real PostgreSQL 15, isolated test container. No production access or data.
 * Guards row visits, wire precision, calibration semantics and SQL cancellation.
 */
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";
import { Pool } from "pg";
import express from "express";
import request from "supertest";
import { mountScoringStatus } from "../src/routes/scoring-status.js";
import { readScoringSummary, SCORING_SUMMARY_SQL, withScoringRead } from "../src/readiness/scoring-summary.js";

let container: StartedTestContainer | undefined;
let pool: Pool;
const keys = ["ARBX_SCORING_ARCHIVER_MODE", "ARBX_CALIBRATION_MIN_SCORED"] as const;
const saved = new Map(keys.map(k => [k, process.env[k]]));
beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "test", POSTGRES_DB: "scoring_test" })
    .withExposedPorts(5432)
    .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2)).start();
  pool = new Pool({ host: container.getHost(), port: container.getMappedPort(5432),
    user: "postgres", password: "test", database: "scoring_test", max: 4 });
  await pool.query(`CREATE TABLE scored_opportunities(id bigint, created_at timestamptz NOT NULL);
    CREATE INDEX idx_scored_opportunities_created ON scored_opportunities(created_at DESC);
    CREATE TABLE gate_c_validation(gate text, status text);
    CREATE TABLE bayesian_priors(observation_count int);
    CREATE TABLE paper_trade_runs(created_at timestamptz);
    CREATE INDEX ON paper_trade_runs(created_at DESC);`);
}, 120_000);
afterEach(async () => {
  if (pool) await pool.query("TRUNCATE scored_opportunities, gate_c_validation, bayesian_priors, paper_trade_runs");
  for (const k of keys) {
    const value = saved.get(k);
    if (value === undefined) delete process.env[k]; else process.env[k] = value;
  }
});
afterAll(async () => { if (pool) await pool.end(); if (container) await container.stop(); }, 30_000);
const endpoint = "/api/v1/scoring/status";
function app() { const a = express(); mountScoringStatus(a, { pool, logger: { warn: () => {} } }); return a; }
async function seed(n: number) {
  await pool.query("INSERT INTO scored_opportunities SELECT n, '2026-08-01'::timestamptz + n * interval '1 second' FROM generate_series(1,$1) n", [n]);
}

describe("scoring summary on actual PostgreSQL", () => {
  for (const n of [0, 1, 999, 1000, 1001, 150000]) {
    it(`reports correct precision and full-history bounds for ${n} rows`, async () => {
      await seed(n);
      const s = await withScoringRead(pool, db => readScoringSummary(db, 100));
      const full = await pool.query("SELECT COUNT(*)::int AS n, MIN(created_at) AS first, MAX(created_at) AS last FROM scored_opportunities");
      expect(s.count).toBe(Math.min(n, 1000));
      expect(s.exact).toBe(n <= 1000);
      expect(s.first).toBe(full.rows[0].first?.toISOString() ?? null);
      expect(s.last).toBe(full.rows[0].last?.toISOString() ?? null);
      expect(s.limit).toBe(1000);
    });
  }
  it("uses bounded indexed visits instead of reading 150000 historical rows", async () => {
    await seed(150000); await pool.query("ANALYZE scored_opportunities");
    const plan = await pool.query("EXPLAIN (ANALYZE, FORMAT JSON) " + SCORING_SUMMARY_SQL, [1001]);
    const scans: Record<string, any>[] = [];
    const visit = (node: Record<string, any>) => {
      if (node["Relation Name"] === "scored_opportunities") scans.push(node);
      for (const child of node.Plans ?? []) visit(child);
    };
    visit(plan.rows[0]["QUERY PLAN"][0].Plan);
    expect(scans).toHaveLength(3);
    expect(scans.every(s => /Index/.test(s["Node Type"]))).toBe(true);
    expect(scans.map(s => s["Actual Rows"]).sort((a,b) => a-b)).toEqual([1, 1, 1001]);
  });
  it("preserves a configured calibration threshold larger than the display floor", async () => {
    process.env["ARBX_CALIBRATION_MIN_SCORED"] = "2000";
    process.env["ARBX_SCORING_ARCHIVER_MODE"] = "on";
    await seed(1999);
    await pool.query("UPDATE scored_opportunities SET created_at=now() WHERE id=1999");
    const a = app();
    const below = await request(a).get(endpoint).expect(200);
    expect(below.body.recent_scored_count).toBe(1999);
    expect(below.body.a5_state).toBe("PAPER_SHADOW_WARMING");
    await pool.query("INSERT INTO scored_opportunities VALUES (2000,now())");
    const at = await request(a).get(endpoint).expect(200);
    expect(at.body.recent_scored_count).toBe(2000);
    expect(at.body.recent_scored_count_exact).toBe(true);
    expect(at.body.a5_state).toBe("CALIBRATED_CANDIDATE");
    await pool.query("INSERT INTO scored_opportunities VALUES (2001,now())");
    const above = await request(a).get(endpoint).expect(200);
    expect(above.body.scoring_count_limit).toBe(2000);
    expect(above.body.recent_scored_count_exact).toBe(false);
    expect(above.body.a5_state).toBe(at.body.a5_state);
    expect(above.body.a4_state).toBe("A4_PENDING");
    expect(above.body.live_trading).toBe(false);
    expect(above.body.submit_enabled).toBe(false);
    await pool.query("INSERT INTO gate_c_validation VALUES ('a4_fork_validation','passed'); INSERT INTO bayesian_priors VALUES (40)");
    const calibrated = await request(a).get(endpoint).expect(200);
    expect(calibrated.body.a4_state).toBe("A4_PASSED");
    expect(calibrated.body.a5_state).toBe("CALIBRATED");
    expect(calibrated.body.live_trading).toBe(false);
  });
  it("cancels actual slow SQL on the server and releases the connection", async () => {
    const start = Date.now();
    await expect(withScoringRead(pool, db => db.query("SELECT pg_sleep(5) /* scoring_timeout_probe */"))).rejects.toThrow();
    expect(Date.now() - start).toBeLessThan(4000);
    const active = await pool.query("SELECT count(*)::int AS n FROM pg_stat_activity WHERE pid<>pg_backend_pid() AND state='active' AND query LIKE '%scoring_timeout_probe%'");
    expect(active.rows[0].n).toBe(0);
    await expect(withScoringRead(pool, db => db.query("SELECT 1"))).resolves.toBeDefined();
  });
  it("does not mutate pool defaults and prevents writes in the observation", async () => {
    const before = (await pool.query("SHOW statement_timeout")).rows[0].statement_timeout;
    await withScoringRead(pool, async db => {
      expect((await db.query("SHOW transaction_read_only")).rows[0].transaction_read_only).toBe("on");
      expect((await db.query("SHOW statement_timeout")).rows[0].statement_timeout).toBe("1500ms");
    });
    expect((await pool.query("SHOW statement_timeout")).rows[0].statement_timeout).toBe(before);
    await expect(withScoringRead(pool, db => db.query("INSERT INTO scored_opportunities VALUES (1,now())"))).rejects.toThrow();
  });
  it("returns 503 under a real table lock, never a fabricated empty count", async () => {
    const lock = await pool.connect();
    try {
      await lock.query("BEGIN; LOCK TABLE scored_opportunities IN ACCESS EXCLUSIVE MODE");
      const start = Date.now();
      const r = await request(app()).get(endpoint).expect(503);
      expect(Date.now()-start).toBeLessThan(4000);
      expect(r.body).toEqual({error:"scoring_status_unavailable"});
    } finally { await lock.query("ROLLBACK"); lock.release(); }
    await request(app()).get(endpoint).expect(200);
  });
  it("releases a late acquired connection after a pool-wait timeout", async () => {
    if (!container) throw new Error("test container unavailable");
    const one = new Pool({ host: container.getHost(), port: container.getMappedPort(5432),
      user: "postgres", password: "test", database: "scoring_test", max: 1 });
    const held = await one.connect();
    try {
      await expect(withScoringRead(one, db => db.query("SELECT 1"))).rejects.toThrow("scoring_pool_timeout");
    } finally { held.release(); }
    try {
      await expect(withScoringRead(one, db => db.query("SELECT 1"))).resolves.toBeDefined();
      expect(one.waitingCount).toBe(0);
      expect(one.idleCount).toBe(1);
    } finally { await one.end(); }
  });
  it("bounds non-SQL stalls and never queries after the deadline", async () => {
    const late = withScoringRead(pool, async db => {
      await new Promise(r => setTimeout(r, 2700));
      return db.query("SELECT 1 /* must_not_run_after_deadline */");
    });
    await expect(late).rejects.toThrow("scoring_read_timeout");
    await new Promise(r => setTimeout(r, 350));
    await expect(withScoringRead(pool, db => db.query("SELECT 1"))).resolves.toBeDefined();
  });
  it("no database is unavailable, not an empty healthy pipeline", async () => {
    const a = express(); mountScoringStatus(a, {pool:null,logger:{warn:()=>{}}});
    expect((await request(a).get(endpoint).expect(503)).body).toEqual({error:"scoring_status_unavailable"});
  });
});
