import { afterEach, describe, expect, it, vi } from "vitest";
import type pg from "pg";
import { isScoringPipelineWired } from "./scoring-status.js";

const initial = process.env["ARBX_SCORING_ARCHIVER_MODE"];
afterEach(() => {
  if (initial === undefined) delete process.env["ARBX_SCORING_ARCHIVER_MODE"];
  else process.env["ARBX_SCORING_ARCHIVER_MODE"] = initial;
});
function queryPool(table: boolean, sample: boolean) {
  const query = vi.fn(async (sql: string) => {
    if (/COUNT|MIN\(|MAX\(/i.test(sql)) throw new Error("expensive aggregate forbidden for boolean probe");
    if (sql.includes("to_regclass")) return { rows: [{ exists: table }] };
    if (sql.includes("EXISTS")) return { rows: [{ has_sample: sample }] };
    throw new Error("unexpected query");
  });
  return { pool: { query } as unknown as pg.Pool, query };
}
describe("scoring wiring is a bounded boolean probe", () => {
  it.each([
    [false, false, "off", false], [false, true, "on", false],
    [true, false, "off", false], [true, false, "on", true],
    [true, true, "off", true], [true, true, "on", true],
  ])("table=%s sample=%s archiver=%s yields %s without scanning history", async (table, sample, mode, expected) => {
    process.env["ARBX_SCORING_ARCHIVER_MODE"] = mode;
    const { pool, query } = queryPool(table, sample);
    expect(await isScoringPipelineWired(pool)).toBe(expected);
    expect(query.mock.calls.length).toBeLessThanOrEqual(2);
  });
  it("no pool is not a proof of wiring", async () => {
    expect(await isScoringPipelineWired(null)).toBe(false);
  });
  it("errors are logged and never reported as wired", async () => {
    const warn = vi.fn();
    const pool = { query: vi.fn(async () => { throw new Error("offline"); }) } as unknown as pg.Pool;
    expect(await isScoringPipelineWired(pool, { warn })).toBe(false);
    expect(warn).toHaveBeenCalledTimes(1);
  });
});
