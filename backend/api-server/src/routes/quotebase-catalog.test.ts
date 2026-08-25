/**
 * QuoteBase catalog contract tests — EMIT-07/EMIT-08 (FE-MASTER P6/P7).
 *
 * Pins the frozen wire contract of GET /api/strategies/catalog (264 rows) and
 * GET /api/detectors/catalog (60 rows) against the generated module
 * (`../generated/quotebase_catalog.ts`). The generator validates STRUCTURAL
 * invariants at generation time (so a workbook re-ingestion can legitimately
 * change counts); THESE tests pin TODAY's canon exactly — a failure here means
 * the canon changed and a human must review the regenerated artifact:
 *   - 264/60 rows, unique ascending MEV_ID / Detector_ID
 *   - status counts 79 ROUTE_READY / 174 NEEDS_ROUTE_DATA / 8 OBSERVE_ONLY /
 *     3 NO_COMPATIBLE_ROUTE
 *   - DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY (37/37) · OBSERVE_ONLY ⟺ class
 *   - Sum(strategies_count) == 264 · detector join complete
 *   - field sets EXACT (18 / 14 keys — no drift; DP-006 amended the detector
 *     shape 13 → 14 keys by adding example_mev)
 *   - allowed_hops expanded sorted ints (TS never decodes bits, §79)
 *
 * Route layer: 200 `{ entries }` serving the arrays VERBATIM (RULE 00 — the
 * route computes nothing).
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it } from "vitest";

import quotebaseCatalog from "./quotebase-catalog.js";
import {
  QUOTEBASE_DETECTOR_CATALOG,
  QUOTEBASE_STRATEGY_CATALOG,
} from "../generated/quotebase_catalog.js";

const STRATEGY_KEYS = [
  "mev_id",
  "group",
  "name",
  "family",
  "surface",
  "backend_module",
  "detector_id",
  "min_legs",
  "max_legs",
  "allowed_hops",
  "graph_model",
  "quotebase_role",
  "search_policy",
  "execution_class",
  "primary_ops",
  "discovery_equation",
  "gate_live",
  "status",
] as const;

const DETECTOR_KEYS = [
  "detector_id",
  "strategies_count",
  "example_surface",
  "example_mev",
  "execution_class",
  "primary_ops",
  "secondary_ops",
  "exact_discovery_criterion",
  "required_data",
  "frontend_config",
  "graph_policy",
  "hop_envelope",
  "hot_seed",
  "do_not_do",
] as const;

describe("generated quotebase_catalog module (canon pin)", () => {
  it("has exactly 264 strategy rows with unique ascending MEV_IDs", () => {
    expect(QUOTEBASE_STRATEGY_CATALOG).toHaveLength(264);
    const ids = QUOTEBASE_STRATEGY_CATALOG.map((s) => s.mev_id);
    expect(new Set(ids).size).toBe(264);
    expect([...ids].sort()).toEqual(ids);
  });

  it("has exactly 60 detector rows with unique Detector_IDs and Sum(count)==264", () => {
    expect(QUOTEBASE_DETECTOR_CATALOG).toHaveLength(60);
    const ids = QUOTEBASE_DETECTOR_CATALOG.map((d) => d.detector_id);
    expect(new Set(ids).size).toBe(60);
    expect(
      QUOTEBASE_DETECTOR_CATALOG.reduce((acc, d) => acc + d.strategies_count, 0),
    ).toBe(264);
  });

  it("every strategy row has EXACTLY the 18 frozen keys", () => {
    for (const s of QUOTEBASE_STRATEGY_CATALOG) {
      expect(Object.keys(s).sort()).toEqual([...STRATEGY_KEYS].sort());
    }
  });

  it("every detector row has EXACTLY the 14 frozen keys", () => {
    for (const d of QUOTEBASE_DETECTOR_CATALOG) {
      expect(Object.keys(d).sort()).toEqual([...DETECTOR_KEYS].sort());
    }
  });

  it("DP-006: every example_mev points at a REAL row of the 264 canon", () => {
    const canon = new Set(QUOTEBASE_STRATEGY_CATALOG.map((s) => s.mev_id));
    for (const d of QUOTEBASE_DETECTOR_CATALOG) {
      expect(d.example_mev).toMatch(/^MEV-\d{2}-\d{3}$/);
      expect(canon.has(d.example_mev)).toBe(true);
    }
  });

  it("DP-006: Do_Not_Do is the ONE universal rule — identical across all 60 rows", () => {
    // The workbook carries a per-row Do_Not_Do column, but today's canon holds
    // a single universal anti-pattern sentence (60/60 in the source JSON). The
    // generator only passes the column through (no fail-fast), so THIS pin is
    // the guard: a divergent or reworded cell fails here and a human reviews
    // the regenerated artifact.
    const UNIVERSAL_DO_NOT = "Do not replace detector math with generic spot-price spread.";
    for (const d of QUOTEBASE_DETECTOR_CATALOG) {
      expect(d.do_not_do).toBe(UNIVERSAL_DO_NOT);
    }
  });

  it("pins today's status counts: 79/174/8/3", () => {
    const counts = { ROUTE_READY: 0, NEEDS_ROUTE_DATA: 0, OBSERVE_ONLY: 0, NO_COMPATIBLE_ROUTE: 0 };
    for (const s of QUOTEBASE_STRATEGY_CATALOG) counts[s.status] += 1;
    expect(counts).toEqual({
      ROUTE_READY: 79,
      NEEDS_ROUTE_DATA: 174,
      OBSERVE_ONLY: 8,
      NO_COMPATIBLE_ROUTE: 3,
    });
  });

  it("DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY (37/37) and OBSERVE_ONLY ⟺ class (8/8)", () => {
    const exec = QUOTEBASE_STRATEGY_CATALOG.filter(
      (s) => s.execution_class === "DETERMINISTIC_EXECUTABLE",
    );
    expect(exec).toHaveLength(37);
    expect(exec.every((s) => s.status === "ROUTE_READY")).toBe(true);
    for (const s of QUOTEBASE_STRATEGY_CATALOG) {
      expect((s.status === "OBSERVE_ONLY") === (s.execution_class === "OBSERVE_ONLY")).toBe(true);
    }
    expect(QUOTEBASE_STRATEGY_CATALOG.filter((s) => s.status === "OBSERVE_ONLY")).toHaveLength(8);
  });

  it("allowed_hops are expanded sorted ints in [2,7]; legs sane; group positive", () => {
    for (const s of QUOTEBASE_STRATEGY_CATALOG) {
      expect(s.allowed_hops.length).toBeGreaterThan(0);
      expect([...s.allowed_hops].sort((a, b) => a - b)).toEqual(s.allowed_hops);
      for (const h of s.allowed_hops) expect(h).toBeGreaterThanOrEqual(2), expect(h).toBeLessThanOrEqual(7);
      expect(s.min_legs).toBeGreaterThanOrEqual(1);
      expect(s.max_legs).toBeLessThanOrEqual(16);
      expect(s.min_legs).toBeLessThanOrEqual(s.max_legs);
      expect(s.group).toBeGreaterThan(0);
    }
  });

  it("detector join is complete (every strategy detector has a policy row)", () => {
    const policyIds = new Set(QUOTEBASE_DETECTOR_CATALOG.map((d) => d.detector_id));
    for (const s of QUOTEBASE_STRATEGY_CATALOG) {
      expect(policyIds.has(s.detector_id)).toBe(true);
    }
    // And each policy's count equals the ACTUAL join count (drift check).
    const actual = new Map<string, number>();
    for (const s of QUOTEBASE_STRATEGY_CATALOG) {
      actual.set(s.detector_id, (actual.get(s.detector_id) ?? 0) + 1);
    }
    for (const d of QUOTEBASE_DETECTOR_CATALOG) {
      expect(d.strategies_count).toBe(actual.get(d.detector_id) ?? 0);
    }
  });

  it("detector rows: hop envelope, hot_seed enum, non-empty phrase arrays", () => {
    for (const d of QUOTEBASE_DETECTOR_CATALOG) {
      expect(d.hop_envelope.min).toBeGreaterThanOrEqual(2);
      expect(d.hop_envelope.max).toBeLessThanOrEqual(7);
      expect(d.hop_envelope.min).toBeLessThanOrEqual(d.hop_envelope.max);
      expect(["SEED_CANDIDATE", "OBSERVE_EVIDENCE"]).toContain(d.hot_seed);
      for (const p of [...d.primary_ops, ...d.secondary_ops, ...d.frontend_config]) {
        expect(p.length).toBeGreaterThan(0);
      }
    }
    // The single telemetry-only detector maps to OBSERVE_EVIDENCE (may_seed()).
    expect(
      QUOTEBASE_DETECTOR_CATALOG.filter((d) => d.hot_seed === "OBSERVE_EVIDENCE"),
    ).toHaveLength(1);
  });

  it("first row matches the workbook canon (MEV-01-001 spot-check)", () => {
    const r0 = QUOTEBASE_STRATEGY_CATALOG[0];
    expect(r0.mev_id).toBe("MEV-01-001");
    expect(r0.surface).toBe("DEX_AMM");
    expect(r0.backend_module).toBe("route_graph_engine");
    expect(r0.detector_id).toBe("R_CLOSED_CYCLE");
    expect(r0.allowed_hops).toEqual([2, 3, 4, 5, 6, 7]);
    expect(r0.graph_model).toBe("TOKEN_MULTIGRAPH");
    expect(r0.quotebase_role).toBe("PRIMARY_PAIR+NUMERAIRE");
    expect(r0.status).toBe("ROUTE_READY");
    expect(r0.primary_ops[0]).toBe("op_27 Path Ordering");
    expect(r0.primary_ops).toHaveLength(4);
    expect(r0.discovery_equation.length).toBeGreaterThan(0);
  });
});

describe("quotebase-catalog router (EMIT-07/08)", () => {
  function buildApp(): Express {
    const app = express();
    app.use("/api", quotebaseCatalog);
    return app;
  }

  it("GET /api/strategies/catalog serves 264 entries VERBATIM", async () => {
    const res = await request(buildApp()).get("/api/strategies/catalog");
    expect(res.status).toBe(200);
    expect(Object.keys(res.body)).toEqual(["entries"]);
    expect(res.body.entries).toHaveLength(264);
    expect(res.body.entries).toEqual(QUOTEBASE_STRATEGY_CATALOG as unknown as unknown[]);
  });

  it("GET /api/detectors/catalog serves 60 entries VERBATIM", async () => {
    const res = await request(buildApp()).get("/api/detectors/catalog");
    expect(res.status).toBe(200);
    expect(Object.keys(res.body)).toEqual(["entries"]);
    expect(res.body.entries).toHaveLength(60);
    expect(res.body.entries).toEqual(QUOTEBASE_DETECTOR_CATALOG as unknown as unknown[]);
  });
});
