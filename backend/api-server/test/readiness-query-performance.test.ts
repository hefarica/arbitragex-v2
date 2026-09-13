/** Real PostgreSQL 15 regression, isolated fixtures only; no production access.
 * Proves boolean equivalence on empty, expired and large recent populations,
 * bounded row visitation, and unchanged exact scoring-response counters.
 */
import { describe, it, expect, beforeAll, afterAll, afterEach } from "vitest";
import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";
import { Pool } from "pg";
import express from "express";
import request from "supertest";
import { PAPER_RECENT_ACTIVITY_SQL } from "../src/readiness/verifiers/g-pap-1.js";
import { isScoringPipelineWired, mountScoringStatus } from "../src/routes/scoring-status.js";

let container: StartedTestContainer | undefined;
let pool: Pool;
const oldMode = process.env["ARBX_SCORING_ARCHIVER_MODE"];
beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "test", POSTGRES_DB: "arbx_readiness_test" })
    .withExposedPorts(5432)
    .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2))
    .start();
  pool = new Pool({ host: container.getHost(), port: container.getMappedPort(5432),
    user: "postgres", password: "test", database: "arbx_readiness_test" });
  await pool.query(`CREATE TABLE opportunities (id bigint, detected_at timestamptz NOT NULL);
    CREATE INDEX ON opportunities(detected_at);
    CREATE TABLE scored_opportunities (id bigint, created_at timestamptz NOT NULL);
    CREATE INDEX ON scored_opportunities(created_at DESC);
    CREATE TABLE gate_c_validation (gate text, status text);
    CREATE TABLE bayesian_priors (observation_count int);
    CREATE TABLE paper_trade_runs (created_at timestamptz);`);
}, 120_000);
afterAll(async () => { if (pool) await pool.end(); if (container) await container.stop(); }, 30_000);
afterEach(async () => {
  await pool.query("TRUNCATE opportunities, scored_opportunities, gate_c_validation, bayesian_priors, paper_trade_runs");
  if (oldMode === undefined) delete process.env["ARBX_SCORING_ARCHIVER_MODE"];
  else process.env["ARBX_SCORING_ARCHIVER_MODE"] = oldMode;
});
async function comparePresence() {
  const actual = await pool.query(PAPER_RECENT_ACTIVITY_SQL);
  const old = await pool.query("SELECT count(*)::int AS n FROM opportunities WHERE detected_at > NOW() - interval '7 days'");
  expect(actual.rows[0]?.has_recent).toBe(old.rows[0].n > 0);
  return actual.rows[0].has_recent;
}
describe("readiness queries on real PostgreSQL", () => {
  it("an empty ledger is still inactive", async () => { expect(await comparePresence()).toBe(false); });
  it("expired history cannot fabricate current activity", async () => {
    await pool.query("INSERT INTO opportunities VALUES (1, now() - interval '8 days')");
    expect(await comparePresence()).toBe(false);
  });
  it("large recent history visits one qualifying row, not all rows", async () => {
    await pool.query("INSERT INTO opportunities SELECT n, now() - interval '1 day' FROM generate_series(1, 150000) n");
    await pool.query("ANALYZE opportunities");
    expect(await comparePresence()).toBe(true);
    const r = await pool.query("EXPLAIN (ANALYZE, FORMAT JSON) " + PAPER_RECENT_ACTIVITY_SQL);
    const plan = r.rows[0]["QUERY PLAN"][0].Plan;
    const scans: Array<{ "Actual Rows": number }> = [];
    function walk(node: any) {
      if (node["Relation Name"] === "opportunities") scans.push(node);
      for (const child of node.Plans ?? []) walk(child);
    }
    walk(plan);
    expect(scans.length).toBeGreaterThan(0);
    expect(scans.every(s => s["Actual Rows"] <= 1)).toBe(true);
  });
  it("wiring with a dormant archiver requires an actual scored row", async () => {
    process.env["ARBX_SCORING_ARCHIVER_MODE"] = "off";
    expect(await isScoringPipelineWired(pool)).toBe(false);
    await pool.query("INSERT INTO scored_opportunities VALUES (1, now())");
    expect(await isScoringPipelineWired(pool)).toBe(true);
  });
  it("a quiet but enabled archiver retains the existing wiring semantics", async () => {
    process.env["ARBX_SCORING_ARCHIVER_MODE"] = "on";
    expect(await isScoringPipelineWired(pool)).toBe(true);
  });
  it("HTTP scoring counts remain exact and are refreshed after the prior read settles", async () => {
    process.env["ARBX_SCORING_ARCHIVER_MODE"] = "on";
    await pool.query("INSERT INTO scored_opportunities VALUES (1, now()-interval '8 days'), (2, now()-interval '1 day'), (3, now())");
    const app = express(); mountScoringStatus(app, { pool, logger: { warn: () => {} } });
    const first = await request(app).get("/api/v1/scoring/status").expect(200);
    expect(first.body.recent_scored_count).toBe(3);
    expect(first.body.scoring_pipeline_state).toBe("WIRED_RUNTIME_SAMPLE");
    expect(first.body.a4_state).toBe("A4_PENDING");
    expect(first.body.live_trading).toBe(false);
    expect(first.body.submit_enabled).toBe(false);
    await pool.query("INSERT INTO scored_opportunities VALUES (4, now())");
    const next = await request(app).get("/api/v1/scoring/status").expect(200);
    expect(next.body.recent_scored_count).toBe(4);
  });
});
