// frontend/lib/store/__tests__/catalog-slices.test.ts
//
// FE-0004 tramo 2 — catalog slices (strategies/detectors/pairs).
//
// Locks the store semantics, NOT the wire (wire locks live in the schema
// tests): static-per-canon fetch-once, loading guard, honest error/absence
// (R8 — null stays null, never fabricated rows), index consistency.
// api-client is mocked at the module boundary because the unit under test
// is the slice state machine, not the transport.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { create } from "zustand";

const getStrategiesCatalog = vi.fn();
const getDetectorsCatalog = vi.fn();
const getPairs = vi.fn();

vi.mock("@/lib/api-client", () => ({
  getStrategiesCatalog: (...a: unknown[]) => getStrategiesCatalog(...a),
  getDetectorsCatalog: (...a: unknown[]) => getDetectorsCatalog(...a),
  getPairs: (...a: unknown[]) => getPairs(...a),
}));

import {
  createDetectorsSlice,
  createPairsSlice,
  createStrategiesSlice,
  type DetectorsSlice,
  type PairsSlice,
  type StrategiesSlice,
} from "../catalog-slices";
import type { DetectorPolicyView, PairView, StrategyCatalogRow } from "@/lib/apex/schemas";

const STRATEGY_ROW: StrategyCatalogRow = {
  mev_id: "MEV-01-001",
  group: 1,
  name: "DEX–DEX arbitrage",
  family: "Arbitrajes spot DEX dentro de una misma cadena",
  surface: "DEX_AMM",
  backend_module: "route_graph_engine",
  detector_id: "R_CLOSED_CYCLE",
  min_legs: 2,
  max_legs: 8,
  allowed_hops: [2, 3, 4, 5, 6, 7],
  graph_model: "TOKEN_MULTIGRAPH",
  quotebase_role: "PRIMARY_PAIR+NUMERAIRE",
  search_policy: "dirty pair/edge → closed-cycle/order route search",
  execution_class: "DETERMINISTIC_EXECUTABLE",
  primary_ops: ["op_27 Path Ordering"],
  discovery_equation: "Q_R(x)",
  gate_live: "Sim PASS",
  status: "ROUTE_READY",
};

const DETECTOR_ROW: DetectorPolicyView = {
  detector_id: "R_CLOSED_CYCLE",
  strategies_count: 25,
  example_surface: "DEX_AMM",
  example_mev: "MEV-01-001",
  execution_class: "DETERMINISTIC_EXECUTABLE",
  primary_ops: ["op_27 Path Ordering"],
  secondary_ops: ["op_01 SVD"],
  exact_discovery_criterion: "Q_R(x)",
  required_data: "reserves",
  frontend_config: ["enabled", "require_same_block=true"],
  graph_policy: "dirty pair/edge → closed-cycle/order route search",
  hop_envelope: { min: 2, max: 7 },
  hot_seed: "SEED_CANDIDATE",
  do_not_do: "Do not replace detector math with generic spot-price spread.",
};

const PAIR_ROW: PairView = {
  chain_id: 1,
  token_a: { chain_id: 1, address: "0x" + "a".repeat(40), symbol: "WETH", decimals: 18 },
  token_b: { chain_id: 1, address: "0x" + "b".repeat(40), symbol: "USDC", decimals: 6 },
  pools: [],
  venue_count: 0,
  alpha_forward: null,
  alpha_reverse: null,
  dirty: false,
  last_reserve_update: null,
};

function makeStore<S extends object>(factory: (set: never, get: () => S) => S) {
  // Compose through zustand so the slice's (set, get) wiring is exercised.
  return create<S>()((set, get) => factory(set as never, get));
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("StrategiesSlice — static-per-canon (P6)", () => {
  it("idle → fetch → ready: rows + Map index consistent, updatedAt stamped", async () => {
    getStrategiesCatalog.mockResolvedValue({ ok: true, data: { entries: [STRATEGY_ROW] } });
    const store = makeStore<StrategiesSlice>(createStrategiesSlice);
    expect(store.getState().strategyCatalog).toBeNull(); // R8: null = never served
    await store.getState().fetchStrategyCatalog();
    const s = store.getState();
    expect(s.strategyCatalogStatus).toBe("ready");
    expect(s.strategyCatalog).toHaveLength(1);
    expect(s.strategyByMevId.get("MEV-01-001")).toBe(s.strategyCatalog![0]);
    expect(s.strategyCatalogUpdatedAt).not.toBeNull();
  });

  it("fetch-once semantics: a second call (no force) does NOT hit the API again", async () => {
    getStrategiesCatalog.mockResolvedValue({ ok: true, data: { entries: [STRATEGY_ROW] } });
    const store = makeStore<StrategiesSlice>(createStrategiesSlice);
    await store.getState().fetchStrategyCatalog();
    await store.getState().fetchStrategyCatalog();
    expect(getStrategiesCatalog).toHaveBeenCalledTimes(1);
  });

  it("force refetch hits the API again (new canon bump)", async () => {
    getStrategiesCatalog.mockResolvedValue({ ok: true, data: { entries: [STRATEGY_ROW] } });
    const store = makeStore<StrategiesSlice>(createStrategiesSlice);
    await store.getState().fetchStrategyCatalog();
    await store.getState().fetchStrategyCatalog({ force: true });
    expect(getStrategiesCatalog).toHaveBeenCalledTimes(2);
  });

  it("honest error: status error + message, catalog stays null (RULE 00)", async () => {
    getStrategiesCatalog.mockResolvedValue({ ok: false, error: "HTTP 404: not emitted" });
    const store = makeStore<StrategiesSlice>(createStrategiesSlice);
    await store.getState().fetchStrategyCatalog();
    const s = store.getState();
    expect(s.strategyCatalogStatus).toBe("error");
    expect(s.strategyCatalogError).toBe("HTTP 404: not emitted");
    expect(s.strategyCatalog).toBeNull();
  });

  it("loading guard: a call while in-flight is a no-op", async () => {
    let release: (() => void) | null = null;
    getStrategiesCatalog.mockImplementation(
      () => new Promise((resolve) => { release = () => resolve({ ok: true, data: { entries: [STRATEGY_ROW] } }); }),
    );
    const store = makeStore<StrategiesSlice>(createStrategiesSlice);
    const first = store.getState().fetchStrategyCatalog();
    const second = store.getState().fetchStrategyCatalog(); // in-flight
    release!();
    await Promise.all([first, second]);
    expect(getStrategiesCatalog).toHaveBeenCalledTimes(1);
  });
});

describe("DetectorsSlice — static-per-canon (P7)", () => {
  it("fetch → ready: rows + detectorById index (the P6 link target)", async () => {
    getDetectorsCatalog.mockResolvedValue({ ok: true, data: { entries: [DETECTOR_ROW] } });
    const store = makeStore<DetectorsSlice>(createDetectorsSlice);
    await store.getState().fetchDetectorCatalog();
    const s = store.getState();
    expect(s.detectorCatalogStatus).toBe("ready");
    expect(s.detectorById.get("R_CLOSED_CYCLE")).toBe(s.detectorCatalog![0]);
  });

  it("fetch-once + force semantics mirror the strategies slice", async () => {
    getDetectorsCatalog.mockResolvedValue({ ok: true, data: { entries: [DETECTOR_ROW] } });
    const store = makeStore<DetectorsSlice>(createDetectorsSlice);
    await store.getState().fetchDetectorCatalog();
    await store.getState().fetchDetectorCatalog();
    await store.getState().fetchDetectorCatalog({ force: true });
    expect(getDetectorsCatalog).toHaveBeenCalledTimes(2);
  });

  it("honest error path (null catalog, surfaced error)", async () => {
    getDetectorsCatalog.mockResolvedValue({ ok: false, error: "schema drift" });
    const store = makeStore<DetectorsSlice>(createDetectorsSlice);
    await store.getState().fetchDetectorCatalog();
    expect(store.getState().detectorCatalogStatus).toBe("error");
    expect(store.getState().detectorCatalog).toBeNull();
  });
});

describe("PairsSlice — live surface (P5 · EMIT-06)", () => {
  it("fetch → ready: entries land with honest null alphas preserved (R8)", async () => {
    getPairs.mockResolvedValue({ ok: true, data: { entries: [PAIR_ROW] } });
    const store = makeStore<PairsSlice>(createPairsSlice);
    await store.getState().fetchPairs(1);
    const s = store.getState();
    expect(s.pairsStatus).toBe("ready");
    expect(s.pairs?.[0]?.alpha_forward).toBeNull();
    expect(getPairs).toHaveBeenCalledWith(1);
  });

  it("empty entries = honest empty universe (NOT an error, NOT null)", async () => {
    getPairs.mockResolvedValue({ ok: true, data: { entries: [] } });
    const store = makeStore<PairsSlice>(createPairsSlice);
    await store.getState().fetchPairs(1);
    expect(store.getState().pairsStatus).toBe("ready");
    expect(store.getState().pairs).toEqual([]);
  });

  it("error path: null pairs + surfaced message (pre-EMIT 404 honesty)", async () => {
    getPairs.mockResolvedValue({ ok: false, error: "HTTP 404" });
    const store = makeStore<PairsSlice>(createPairsSlice);
    await store.getState().fetchPairs(1);
    expect(store.getState().pairsStatus).toBe("error");
    expect(store.getState().pairs).toBeNull();
    expect(store.getState().pairsError).toBe("HTTP 404");
  });

  it("setPairs direct setter marks ready (realtime push path)", () => {
    const store = makeStore<PairsSlice>(createPairsSlice);
    store.getState().setPairs([PAIR_ROW]);
    const s = store.getState();
    expect(s.pairsStatus).toBe("ready");
    expect(s.pairsUpdatedAt).not.toBeNull();
  });
});
