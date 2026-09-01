# Alpha-Map ID parity — reproducible 5-way diff (2026-09-01)

**Workbook item:** ALPHA-MAP-ID-DRIFT (Holy_Grail_Audit_20260901_064750Z_c719f4e99c.xlsx,
sheet 50_CURRENT_DEFECTS — "Static repository MEV-ID cardinality is 266, not the
canonical 264", severity HIGH, evolution PERSISTS).

**Why #496 did not flip it.** The 2026-08-31 remediation proved the parity —
but with a `/tmp` script (`alpha_parity.py`) that never landed in the repo, so
the proof was not reproducible from HEAD. Worse, the audit doc itself carried
the removed example ID as a literal, so the auditor's raw re-scan still counted
**266**: 264 canonical + the 99-series test sentinel (literal elided since the
2026-09-01 exact-264 closure — described below) + the doc example ID, and
a third copy of that example sat in `.claude/skills/arbitragex-omniscience/SKILL.md`
(an unanswerable query — the ID exists in no graph, workbook, or runtime).

## The 5 exact ID sets (all compared with set semantics, no fuzzy match)

| # | Set | Source | Cardinality |
|---|---|---|---|
| 1 | **Canonical workbook catalog** | `docs/quotebase_strategy_hop_map.json` (`MEV_ID`; the SSOT `scripts/xls/gen_hopmask_rs.py` consumes — hoja 11_STRATEGY_HOP_MAP del workbook QUOTEBASE-264) | **264** |
| 2 | **Generated Rust tables ×3** | ID tuples parsed from the pre-`#[cfg(test)]` region of `strategy_hop_mask.rs`, `detector_policy.rs`, `strategy_dispatch_status.rs` | **264 = 264 = 264** |
| 3 | **Static repo raw scan** | `MEV-[0-9]{2}-[0-9]{3}` unique across `git ls-files` (tracked files only — vendored/build dirs are untracked) | **265** |
| 4 | **Runtime-consumed registry** | live export `GET /api/cartridges/runtime` (`source: searcher_registry`), `mev_NN_NNN_slug` → `MEV-NN-NNN` — archived verbatim in `alpha-map-parity-runtime.json` | **271 = 264 MEV + 7 legacy** |
| 5 | **Status/HopMask/Detector parity** | per-ID compare of set 1 vs set 2 (mask bits, detector id, dispatch status) | **264/264 EQUAL** |

## Classification of every non-canonical occurrence

| ID | Where | Class | Disposition |
|---|---|---|---|
| (99-series test sentinel — literal elided since 2026-09-01) | 12 tracked files at the time (rust test asserts `hop_mask → None` / honest-skip paths, gen script, FE tests) | **TEST_SENTINEL** (negative control) | Originally KEPT + gate-allowlisted. **2026-09-01 exact-264 closure:** every use site now concat-constructs the ID (identical runtime string, no source literal) and docs describe-not-quote it; the gate's EXTRAS_ALLOWLIST is EMPTY and the raw scan reconciles to exactly 264. Renaming was never needed — the negative control is intact. |
| (elided doc example) | was: SUPER_SKILL.md (fixed by #496) + this audit family | **DOC example, nonexistent by design** (0 hits in `knowledge_graph.jsonl`, workbook, runtime) | REMOVED from `.claude/skills/arbitragex-omniscience/SKILL.md` (query re-pointed to graph-verified `MEV-06-018`, 10 edges) and ELIDED from the audit docs (full literal preserved in git history, PR #496 / this PR). Elision is stated, not silent. |
| `backrun`, `dex_arb`, `funding_rate_arbitrage`, `liquidation`, `mean_reversion_arbitrage`, `omega_strategy_pack`, `triangular_arb` | runtime registry only (not in repo scan — they don't match the MEV pattern) | **LEGACY_RUNTIME** (pre-v3 slug cartridges loaded by searcher-rs) | KEPT — real cartridges, distinct axis from dispatch (loaded ≠ dispatched). Retirement/re-catalog is an operator registry decision (§34 hot-path untouched). |
| Missing from runtime | — | none | all 264 canonical present and loaded (`canonical_missing: []`). |

## Reproduce (the evidence is now a repo tool, not a one-off)

```bash
python automation/tools/alpha-map-parity.py                          # static: CI mode
python automation/tools/alpha-map-parity.py --runtime https://arbx.ape-tv.net \
    --out docs/audits/alpha-map-parity-runtime.json                  # + live export/diff
```

Output at HEAD of this PR (2026-09-01):

```
rust STRATEGY_HOP_MASKS: 264 | parity OK
rust STRATEGY_DETECTOR: 264 | parity OK
rust STRATEGY_DISPATCH_STATUS: 264 | parity OK
repo scan: 265 unique = 264 canonical + 1 allowlisted ({'…99-series sentinel…': 'TEST_SENTINEL'})
   — the allowlist line above is historical (pre exact-264); the literal is
     elided here so this doc cannot re-drift the scan it documents.
canonical parity: 264/264 OK
ALPHA-MAP-PARITY PASS
---
runtime: source=searcher_registry cartridges_total=271 canonical_present=264
         canonical_missing=[] extra_mev_pattern=[] verdict=PASS
```

Wired as a required CI step (`omega8-m3-grep-gates.yml` → `alpha-map-parity`),
so the cardinality invariant is enforced on every PR — the raw scan can never
silently drift to 266 again.

## Status↔Dispatch parity (unchanged from the 2026-08-31 proof)

| Workbook Status | Rust DispatchStatus | count |
|---|---|---|
| NEEDS_ROUTE_DATA | NeedsRouteData | 174 |
| NO_COMPATIBLE_ROUTE | NoCompatibleRoute | 3 |
| OBSERVE_ONLY | ObserveOnly | 8 |
| ROUTE_READY | RouteReady | 79 |

Runtime `state=Active` (all 271) is the loaded axis; `DispatchStatus` is the
route-data disposition — different axes, both consistent with the workbook.
