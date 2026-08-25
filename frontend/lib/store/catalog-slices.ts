/**
 * =============================================================================
 * FE-MASTER · Catalog slices for the Omni-Store (FE-0004, tramo 2)
 * =============================================================================
 *
 * The P5–P7 surfaces whose shape contracts landed in tramo 2
 * (apex/schemas/{pairs,strategies,detectors}.ts) and whose EMIT payloads
 * (06/07/08) exist behind `{ entries }` envelopes:
 *
 *   - StrategiesSlice   workbook strategy catalog (264 rows) — STATIC-PER-
 *                       CANON: fetched once, refetched only on force/new
 *                       canon. Runtime enabled state lives in
 *                       trading_config, never here (§28).
 *   - DetectorsSlice    detector policy catalog (60 rows) — same
 *                       static-per-canon semantics.
 *   - PairsSlice        live PairView[] of the effective universe — the
 *                       ONLY catalog surface with live refresh; mirrors the
 *                       TelemetrySlice pattern (snapshot fetch + direct
 *                       setter for the realtime path, cadence owned by the
 *                       realtime provider FE-0008).
 *
 * RULE 00 / R8: `null` = never served (render "—", never 0); error strings
 * surface honestly; no fabricated rows. Performance rules of omni-store.ts
 * apply unchanged (selectors only).
 */

import type { StoreApi } from "zustand";
import type {
  DetectorPolicyView,
  PairView,
  StrategyCatalogRow,
} from "@/lib/apex/schemas";
import {
  getDetectorsCatalog,
  getPairs,
  getStrategiesCatalog,
} from "@/lib/api-client";
import type { FetchStatus } from "./runtime-slices";

// =============================================================================
// Strategies slice — workbook catalog, static-per-canon (P6 · EMIT-07)
// =============================================================================

export interface StrategiesSlice {
  /** 264 workbook rows (null = never fetched). Count is a contract test, not a runtime guarantee. */
  strategyCatalog: StrategyCatalogRow[] | null;
  /** O(1) lookup index by mev_id, rebuilt on fetch. */
  strategyByMevId: Map<string, StrategyCatalogRow>;
  strategyCatalogStatus: FetchStatus;
  strategyCatalogError: string | null;
  strategyCatalogUpdatedAt: string | null;
  /**
   * Fetch once per canon: skips while loading OR once ready unless
   * `force` (new workbook ingestion bumped the canon).
   */
  fetchStrategyCatalog: (opts?: { force?: boolean }) => Promise<void>;
}

export function createStrategiesSlice(
  set: StoreApi<StrategiesSlice>["setState"],
  get: () => StrategiesSlice,
): StrategiesSlice {
  return {
    strategyCatalog: null,
    strategyByMevId: new Map(),
    strategyCatalogStatus: "idle",
    strategyCatalogError: null,
    strategyCatalogUpdatedAt: null,
    fetchStrategyCatalog: async (opts) => {
      const s = get();
      if (s.strategyCatalogStatus === "loading") return;
      if (s.strategyCatalogStatus === "ready" && s.strategyCatalog && !opts?.force) return;
      set({ strategyCatalogStatus: "loading", strategyCatalogError: null });
      const res = await getStrategiesCatalog();
      if (res.ok) {
        const byId = new Map(res.data.entries.map((row) => [row.mev_id, row]));
        set({
          strategyCatalog: res.data.entries,
          strategyByMevId: byId,
          strategyCatalogStatus: "ready",
          strategyCatalogUpdatedAt: new Date().toISOString(),
        });
      } else {
        // Honest absence (404 pre-EMIT, parse drift): surfaced, never zeroed.
        set({ strategyCatalogStatus: "error", strategyCatalogError: res.error });
      }
    },
  };
}

// =============================================================================
// Detectors slice — policy catalog, static-per-canon (P7 · EMIT-08)
// =============================================================================

export interface DetectorsSlice {
  /** 60 family rows (null = never fetched). */
  detectorCatalog: DetectorPolicyView[] | null;
  /** O(1) lookup index by detector_id (the P6 link target), rebuilt on fetch. */
  detectorById: Map<string, DetectorPolicyView>;
  detectorCatalogStatus: FetchStatus;
  detectorCatalogError: string | null;
  detectorCatalogUpdatedAt: string | null;
  fetchDetectorCatalog: (opts?: { force?: boolean }) => Promise<void>;
}

export function createDetectorsSlice(
  set: StoreApi<DetectorsSlice>["setState"],
  get: () => DetectorsSlice,
): DetectorsSlice {
  return {
    detectorCatalog: null,
    detectorById: new Map(),
    detectorCatalogStatus: "idle",
    detectorCatalogError: null,
    detectorCatalogUpdatedAt: null,
    fetchDetectorCatalog: async (opts) => {
      const s = get();
      if (s.detectorCatalogStatus === "loading") return;
      if (s.detectorCatalogStatus === "ready" && s.detectorCatalog && !opts?.force) return;
      set({ detectorCatalogStatus: "loading", detectorCatalogError: null });
      const res = await getDetectorsCatalog();
      if (res.ok) {
        const byId = new Map(res.data.entries.map((row) => [row.detector_id, row]));
        set({
          detectorCatalog: res.data.entries,
          detectorById: byId,
          detectorCatalogStatus: "ready",
          detectorCatalogUpdatedAt: new Date().toISOString(),
        });
      } else {
        set({ detectorCatalogStatus: "error", detectorCatalogError: res.error });
      }
    },
  };
}

// =============================================================================
// Pairs slice — live PairView[] of the effective universe (P5 · EMIT-06)
// =============================================================================

export interface PairsSlice {
  /** Latest per-pair snapshot (null = never served). Empty array = honest empty universe. */
  pairs: PairView[] | null;
  pairsStatus: FetchStatus;
  pairsError: string | null;
  pairsUpdatedAt: string | null;
  /** Snapshot fetch — cadence/retry belongs to the realtime provider (FE-0008). */
  fetchPairs: (chainId?: number) => Promise<void>;
  /** Direct setter for the realtime push path (mirrors TelemetrySlice.setTick). */
  setPairs: (pairs: PairView[]) => void;
}

export function createPairsSlice(
  set: StoreApi<PairsSlice>["setState"],
  get: () => PairsSlice,
): PairsSlice {
  return {
    pairs: null,
    pairsStatus: "idle",
    pairsError: null,
    pairsUpdatedAt: null,
    fetchPairs: async (chainId = 1) => {
      if (get().pairsStatus === "loading") return;
      set({ pairsStatus: "loading", pairsError: null });
      const res = await getPairs(chainId);
      if (res.ok) {
        set({
          pairs: res.data.entries,
          pairsStatus: "ready",
          pairsUpdatedAt: new Date().toISOString(),
        });
      } else {
        set({ pairsStatus: "error", pairsError: res.error });
      }
    },
    setPairs: (pairs) =>
      set({ pairs, pairsStatus: "ready", pairsUpdatedAt: new Date().toISOString() }),
  };
}
