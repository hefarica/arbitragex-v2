// frontend/features/route-discovery/__tests__/unavailable-copy.test.ts
//
// R8 regression: the STALE alert must name the reason the route actually
// emitted. The defect it pins: the old copy asserted "Postgres is unreachable"
// for every 503, including the self-healing `rollup_backfilling` and the
// budget-blowing `query_failed`.
import { describe, expect, it } from "vitest";

import {
  OUTCOMES_POLL_SECONDS,
  outcomesUnavailableCopy,
} from "../unavailable-copy";

describe("outcomesUnavailableCopy — reason-faithful STALE copy", () => {
  it("rollup_backfilling is described as a self-healing catch-up, never an outage", () => {
    const copy = outcomesUnavailableCopy("rollup_backfilling", {
      missing_buckets: 3,
      retry_after_s: 30,
    });
    expect(copy).toContain("rollup is catching up");
    expect(copy).toContain("3 buckets still to fill");
    expect(copy).toContain("retries in 30s");
    expect(copy).not.toMatch(/postgres is unreachable/i);
  });

  it("rollup_backfilling without recovery detail still never claims an outage", () => {
    const copy = outcomesUnavailableCopy("rollup_backfilling");
    expect(copy).toContain("rollup is catching up");
    expect(copy).not.toMatch(/unreachable/i);
  });

  it("db_unavailable is the ONLY reason that names the database", () => {
    expect(outcomesUnavailableCopy("db_unavailable")).toMatch(/Postgres pool/);
    for (const reason of ["query_failed", "edge_unreachable", "weird_new_reason"]) {
      expect(outcomesUnavailableCopy(reason)).not.toMatch(/Postgres/i);
    }
  });

  it("query_failed names the 15s budget, not a missing database", () => {
    const copy = outcomesUnavailableCopy("query_failed");
    expect(copy).toContain("15s");
    expect(copy).toContain("query_failed");
  });

  it("an unknown reason is quoted verbatim — never explained away", () => {
    expect(outcomesUnavailableCopy("weird_new_reason")).toContain("weird_new_reason");
    expect(outcomesUnavailableCopy(null)).toBe("The outcomes summary could not be read.");
  });

  it("pins the poll cadence both panels must disclose (60s, not the stale 8s)", () => {
    expect(OUTCOMES_POLL_SECONDS).toBe(60);
  });
});
