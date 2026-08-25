/**
 * QuoteBase workbook catalogs — EMIT-07/EMIT-08 (FE-MASTER P6/P7).
 *
 * Serves the two static-per-canon tables VERBATIM from the generated module
 * (`src/generated/quotebase_catalog.ts`, produced by
 * `scripts/gen_quotebase_catalog_ts.py` from docs/quotebase_strategy_hop_map.json
 * + docs/quotebase_detector_policy.json — structural invariants validated at
 * generation time):
 *
 *   GET /api/strategies/catalog  — 264 rows, one per workbook strategy
 *   GET /api/detectors/catalog   — 60 rows, one per detector family
 *
 * Static compile-time data: no DB, no Redis, no 503 path — unlike the
 * strategy-catalog sibling (which mirrors the PG `strategy_catalog` table of
 * internal strategy kinds — a DIFFERENT domain that keeps serving unchanged).
 * The wire shape is the frozen contract in .ai-work/FE-P5-P7-DOMAIN-SHAPES.md
 * §2/§3; `allowed_hops` arrives already expanded so TS never decodes bits (§79),
 * and runtime enabled/active state lives in trading_config, never here (§28).
 *
 * Envelope mirrors the sibling convention: `{ entries }` on 200.
 */

import { Router } from "express";

import {
  QUOTEBASE_DETECTOR_CATALOG,
  QUOTEBASE_STRATEGY_CATALOG,
} from "../generated/quotebase_catalog.js";

const quotebaseCatalog = Router();

quotebaseCatalog.get("/strategies/catalog", (_req, res) => {
  res.status(200).json({ entries: QUOTEBASE_STRATEGY_CATALOG });
});

quotebaseCatalog.get("/detectors/catalog", (_req, res) => {
  res.status(200).json({ entries: QUOTEBASE_DETECTOR_CATALOG });
});

export default quotebaseCatalog;
