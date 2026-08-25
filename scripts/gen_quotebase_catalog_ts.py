#!/usr/bin/env python3
"""Generate backend/api-server/src/generated/quotebase_catalog.ts from the
QuoteBase workbook canon (EMIT-07/EMIT-08, FE-MASTER P6/P7).

Single source of truth -> generated TS (mirrors gen_strategy_kinds.py):
  - docs/quotebase_strategy_hop_map.json   (264 rows, workbook 11_STRATEGY_HOP_MAP)
  - docs/quotebase_detector_policy.json    (60 rows,  workbook 25_DETECTOR_POLICY)

The generated module is what `GET /api/strategies/catalog` and
`GET /api/detectors/catalog` serve VERBATIM (wire shape = the frozen contract
in .ai-work/FE-P5-P7-DOMAIN-SHAPES.md §2/§3 — amended 2026-08-24:
frontend_config is string[], not knob specs). Compile-time inclusion: the
api-server Docker image carries no docs/ tree, and the catalog is
static-per-canon (changes only on workbook re-ingestion).

Structural invariants are enforced FAIL-FAST at generation time:
  - 264 unique ascending MEV_IDs; 60 unique Detector_IDs; join sets equal
  - Allowed_Hops <-> HopMask_u8 (bit h-2) consistency, hops in [2,7]
  - min/max legs sane (min >= 1, max <= 16, min <= max) — workbook canon goes
    up to 16 legs (71 rows > 8); the repo's hot-path 7-leg cap is runtime
    policy, NOT catalog metadata
  - status in the 4-value DispatchStatus enum
  - DETERMINISTIC_EXECUTABLE execution_class => ROUTE_READY status
  - OBSERVE_ONLY status <=> OBSERVE_ONLY execution_class
  - per-detector join count == the Strategies column (drift check)
  - hop envelope parse; hot_seed mapping total over the 5 workbook sentences
Exact status counts are NOT pinned here (workbook re-ingestion may legitimately
change them) — the route contract test pins today's canon.

Usage: python scripts/gen_quotebase_catalog_ts.py
"""

from __future__ import annotations

import collections
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
HOP_MAP = ROOT / "docs" / "quotebase_strategy_hop_map.json"
DETECTOR_POLICY = ROOT / "docs" / "quotebase_detector_policy.json"
OUT_FILE = (
    ROOT / "backend" / "api-server" / "src" / "generated" / "quotebase_catalog.ts"
)

DISPATCH_STATUSES = {
    "ROUTE_READY",
    "NEEDS_ROUTE_DATA",
    "OBSERVE_ONLY",
    "NO_COMPATIBLE_ROUTE",
}
# may_seed() == !TelemetryOnly (searcher-rs detector_policy.rs HotSeed).
HOT_SEED_SENTENCES = {
    "Detector-specific threshold from exact criterion": "SEED_CANDIDATE",
    "Spread/log-alpha/depth dislocation": "SEED_CANDIDATE",
    "State change / post-event delta": "SEED_CANDIDATE",
    "Cross-domain price/settlement dislocation": "SEED_CANDIDATE",
    "No hot opportunity seed; telemetry evidence only": "OBSERVE_EVIDENCE",
}


def fail(msg: str) -> None:
    sys.exit(f"ERROR: {msg}")


def split_phrases(raw: str) -> list[str]:
    """Split a workbook ';'-delimited cell into trimmed phrases (trailing '.'
    stripped — display normalization, applied uniformly)."""
    return [p.strip().rstrip(".").strip() for p in str(raw).split(";")]


def parse_hops(raw: str) -> list[int]:
    txt = str(raw).strip()
    if not txt:
        return []
    return [int(x) for x in txt.split(",") if x.strip()]


def js_str(s: str) -> str:
    # json.dumps emits a valid TS double-quoted string literal; keep Unicode.
    return json.dumps(s, ensure_ascii=False)


def build_strategies(rows: list[dict]) -> list[dict]:
    out: list[dict] = []
    for r in rows:
        hops = parse_hops(r["Allowed_Hops"])
        mask = int(r["HopMask_u8"])
        if sum(1 << (h - 2) for h in hops) != mask:
            fail(f"{r['MEV_ID']}: Allowed_Hops {hops} inconsistent with HopMask_u8 {mask}")
        for h in hops:
            if not 2 <= h <= 7:
                fail(f"{r['MEV_ID']}: hop {h} outside [2,7]")
        status = str(r["Status"])
        if status not in DISPATCH_STATUSES:
            fail(f"{r['MEV_ID']}: unknown Status {status!r}")
        if r["Execution_Class"] == "DETERMINISTIC_EXECUTABLE" and status != "ROUTE_READY":
            fail(f"{r['MEV_ID']}: DETERMINISTIC_EXECUTABLE but status {status}")
        observe_class = r["Execution_Class"] == "OBSERVE_ONLY"
        if (status == "OBSERVE_ONLY") != observe_class:
            fail(f"{r['MEV_ID']}: OBSERVE_ONLY status/class mismatch")
        min_legs, max_legs = int(r["Min_Legs"]), int(r["Max_Legs"])
        if not (1 <= min_legs and max_legs <= 16 and min_legs <= max_legs):
            fail(f"{r['MEV_ID']}: legs [{min_legs},{max_legs}] outside envelope")
        out.append(
            {
                "mev_id": str(r["MEV_ID"]),
                "group": int(r["Group"]),
                "name": str(r["Strategy"]),
                "family": str(r["Family"]),
                "surface": str(r["Surface"]),
                "backend_module": str(r["Backend_Module"]),
                "detector_id": str(r["Detector_ID"]),
                "min_legs": min_legs,
                "max_legs": max_legs,
                "allowed_hops": hops,
                "graph_model": str(r["Graph_Model"]),
                "quotebase_role": str(r["QuoteBase_Role"]),
                "search_policy": str(r["Search_Policy"]),
                "execution_class": str(r["Execution_Class"]),
                "primary_ops": [p for p in split_phrases(r["Primary_Ops"]) if p],
                "discovery_equation": str(r["Discovery_Equation"]),
                "gate_live": str(r["Gate_LIVE"]),
                "status": status,
            }
        )
    return out


def build_detectors(rows: list[dict], strategies: list[dict]) -> list[dict]:
    join_counts = collections.Counter(s["detector_id"] for s in strategies)
    mev_ids = {s["mev_id"] for s in strategies}
    out: list[dict] = []
    for r in rows:
        det = str(r["Detector_ID"])
        hops = parse_hops(r["Hop_Use"])
        if not hops:
            fail(f"{det}: empty Hop_Use")
        sentence = str(r["Hot_Seed"])
        if sentence not in HOT_SEED_SENTENCES:
            fail(f"{det}: unmapped Hot_Seed sentence {sentence!r}")
        declared = int(str(r["Strategies"]).strip())
        if declared != join_counts.get(det, 0):
            fail(
                f"{det}: Strategies column {declared} != join count "
                f"{join_counts.get(det, 0)} (workbook drift)"
            )
        # DP-006: Example_MEV must point at a REAL row of the 264 canon (the
        # FE links the detector console straight to that strategy card).
        example_mev = str(r["Example_MEV"]).strip()
        if not example_mev:
            fail(f"{det}: empty Example_MEV")
        if example_mev not in mev_ids:
            fail(f"{det}: Example_MEV {example_mev!r} not in the 264-strategy canon")
        out.append(
            {
                "detector_id": det,
                "strategies_count": declared,
                "example_surface": str(r["Example_Surface"]),
                "example_mev": example_mev,
                "execution_class": str(r["Execution_Class"]),
                "primary_ops": [p for p in split_phrases(r["Primary_Ops"]) if p],
                "secondary_ops": [p for p in split_phrases(r["Secondary_Ops"]) if p],
                "exact_discovery_criterion": str(r["Exact_Discovery_Criterion"]),
                "required_data": str(r["Required_Data"]),
                "frontend_config": [p for p in split_phrases(r["Frontend_Config"]) if p],
                "graph_policy": str(r["Graph_Policy"]),
                "hop_envelope": {"min": min(hops), "max": max(hops)},
                "hot_seed": HOT_SEED_SENTENCES[sentence],
                "do_not_do": str(r["Do_Not_Do"]),
            }
        )
    return out


def emit_ts(strategies: list[dict], detectors: list[dict]) -> str:
    lines: list[str] = []
    a = lines.append
    a("// GENERATED FILE — DO NOT EDIT.")
    a("// Regenerate with: python scripts/gen_quotebase_catalog_ts.py")
    a("// Sources (canon, drift-checked): docs/quotebase_strategy_hop_map.json (264 rows,")
    a("// workbook 11_STRATEGY_HOP_MAP) + docs/quotebase_detector_policy.json (60 rows,")
    a("// workbook 25_DETECTOR_POLICY). Wire shape = frozen contract in")
    a("// .ai-work/FE-P5-P7-DOMAIN-SHAPES.md (P6 §2 / P7 §3, amended 2026-08-24).")
    a("// Served VERBATIM by GET /api/strategies/catalog and GET /api/detectors/catalog")
    a("// (EMIT-07/EMIT-08). Structural invariants validated at generation time.")
    a("")
    a("/** Workbook dispatch status (ARBX-0021) — col Status of 11_STRATEGY_HOP_MAP. */")
    a("export type QuotebaseDispatchStatus =")
    a('  | "ROUTE_READY"')
    a('  | "NEEDS_ROUTE_DATA"')
    a('  | "OBSERVE_ONLY"')
    a('  | "NO_COMPATIBLE_ROUTE";')
    a("")
    a("/** P6 catalog row — one per workbook strategy (264 total). */")
    a("export interface QuotebaseStrategyRow {")
    a("  mev_id: string;")
    a("  group: number;")
    a("  name: string;")
    a("  family: string;")
    a("  surface: string;")
    a("  backend_module: string;")
    a("  detector_id: string;")
    a("  min_legs: number;")
    a("  max_legs: number;")
    a("  allowed_hops: number[];")
    a("  graph_model: string;")
    a("  quotebase_role: string;")
    a("  search_policy: string;")
    a("  execution_class: string;")
    a("  primary_ops: string[];")
    a("  discovery_equation: string;")
    a("  gate_live: string;")
    a("  status: QuotebaseDispatchStatus;")
    a("}")
    a("")
    a("/** may_seed() of searcher-rs detector_policy.rs, 2-valued projection. */")
    a('export type QuotebaseHotSeed = "SEED_CANDIDATE" | "OBSERVE_EVIDENCE";')
    a("")
    a("/** P7 policy row — one per workbook detector family (60 total). */")
    a("export interface QuotebaseDetectorRow {")
    a("  detector_id: string;")
    a("  strategies_count: number;")
    a("  example_surface: string;")
    a("  example_mev: string;")
    a("  execution_class: string;")
    a("  primary_ops: string[];")
    a("  secondary_ops: string[];")
    a("  exact_discovery_criterion: string;")
    a("  required_data: string;")
    a("  frontend_config: string[];")
    a("  graph_policy: string;")
    a("  hop_envelope: { min: number; max: number };")
    a("  hot_seed: QuotebaseHotSeed;")
    a("  do_not_do: string;")
    a("}")
    a("")
    a(f"export const QUOTEBASE_STRATEGY_CATALOG: readonly QuotebaseStrategyRow[] = [")
    for s in strategies:
        a(f"  {json.dumps(s, ensure_ascii=False)},")
    a("];")
    a("")
    a(f"export const QUOTEBASE_DETECTOR_CATALOG: readonly QuotebaseDetectorRow[] = [")
    for d in detectors:
        a(f"  {json.dumps(d, ensure_ascii=False)},")
    a("];")
    a("")
    return "\n".join(lines)


def main() -> None:
    if not HOP_MAP.is_file():
        fail(f"hop map not found: {HOP_MAP}")
    if not DETECTOR_POLICY.is_file():
        fail(f"detector policy not found: {DETECTOR_POLICY}")
    raw_s = json.loads(HOP_MAP.read_text(encoding="utf-8"))
    raw_d = json.loads(DETECTOR_POLICY.read_text(encoding="utf-8"))
    if len(raw_s) != 264:
        fail(f"expected 264 strategy rows, got {len(raw_s)}")
    if len(raw_d) != 60:
        fail(f"expected 60 detector rows, got {len(raw_d)}")

    strategies = build_strategies(raw_s)
    ids = [s["mev_id"] for s in strategies]
    if len(set(ids)) != 264:
        fail("duplicate MEV_ID in hop map")
    strategies.sort(key=lambda s: s["mev_id"])
    if ids != sorted(ids):
        print("note: hop map was not MEV_ID-ascending; generated output sorted")

    detectors = build_detectors(raw_d, strategies)
    det_ids = [d["detector_id"] for d in detectors]
    if len(set(det_ids)) != 60:
        fail("duplicate Detector_ID in detector policy")
    detectors.sort(key=lambda d: d["detector_id"])
    if set(det_ids) != {s["detector_id"] for s in strategies}:
        fail("detector join sets differ between the two canon tables")

    total = sum(d["strategies_count"] for d in detectors)
    if total != 264:
        fail(f"Sum(strategies_count) = {total} != 264")

    OUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    OUT_FILE.write_text(emit_ts(strategies, detectors), encoding="utf-8", newline="\n")

    status_counts = collections.Counter(s["status"] for s in strategies)
    print(f"OK: wrote {OUT_FILE}")
    print(f"  strategies: {len(strategies)} rows")
    print(f"  detectors:  {len(detectors)} rows, Sum(strategies_count) = {total}")
    print(f"  status counts: {dict(status_counts)}")


if __name__ == "__main__":
    main()
