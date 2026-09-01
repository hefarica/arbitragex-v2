#!/usr/bin/env python3
"""alpha-map-parity.py — ALPHA-MAP-ID-DRIFT exact parity gate (reproducible).

Finding (Holy Grail workbook, sheet 50_CURRENT_DEFECTS): "Static repository
MEV-ID cardinality is 266, not the canonical 264". PASS predicate: export the
runtime-consumed canonical registry, diff exact IDs against the 264 catalog,
classify extras/missing, and prove status/HopMask/detector parity.

The 2026-08-31 remediation (#496) proved all of that ONCE, but with a /tmp
script that never landed — the proof was not reproducible from the repo, and
the raw repo scan re-counted 266 because the audit docs themselves carried
the removed example ID as a literal. This tool makes the invariant durable:

  static mode (CI-safe, default):
    1. canonical  = MEV_ID set of docs/quotebase_strategy_hop_map.json
                    (the SSOT the Rust generators consume; 264, unique).
    2. rust tables = ID sets parsed from the three generated tables the
                    searcher compiles (hop mask / detector / dispatch status);
                    each must equal the canonical set EXACTLY.
    3. repo scan  = unique MEV-XX-XXX across git-tracked files. Every ID must
                    be canonical or in the EXTRAS allowlist below (each
                    allowlisted ID is a classified, intentional non-strategy
                    occurrence). Anything else fails the gate with its files.

  runtime mode (--runtime-url URL [--out FILE]):
    4. exports GET /api/cartridges/runtime (the registry searcher-rs actually
                    loaded), normalizes mev_NN_NNN_slug -> MEV-NN-NNN, asserts
                    all 264 canonical are present, and classifies every other
                    cartridge id against the LEGACY_SLUG allowlist.

Fail-fast: exits 1 with the exact offending IDs/files on any drift.
Run:  python automation/tools/alpha-map-parity.py
      python automation/tools/alpha-map-parity.py --runtime https://arbx.ape-tv.net
"""
import argparse
import json
import os
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CANONICAL_JSON = ROOT / "docs" / "quotebase_strategy_hop_map.json"

RUST_TABLES = {
    "STRATEGY_HOP_MASKS": ROOT / "backend" / "searcher-rs" / "src" / "strategy_hop_mask.rs",
    "STRATEGY_DETECTOR": ROOT / "backend" / "searcher-rs" / "src" / "detector_policy.rs",
    "STRATEGY_DISPATCH_STATUS": ROOT / "backend" / "searcher-rs" / "src" / "strategy_dispatch_status.rs",
}

EXPECTED_CANONICAL = 264

# Classified, intentional non-strategy MEV-pattern occurrences in the repo.
# Adding an entry here without a doc trail is gaming the gate — each one must
# carry a classification in docs/audits/alpha-map-parity-*.md.
EXTRAS_ALLOWLIST = {
    # Negative-control sentinel: well-formed but unknown ID. Every use asserts
    # the fail-honest path (hop_mask -> None, honest skip). Renaming it would
    # weaken the negative control, so the gate allowlists it instead.
    "MEV-99-999": "TEST_SENTINEL",
}

# Pre-v3 slug cartridges the searcher loads alongside the 264 (LEGACY_RUNTIME
# in the 2026-08-31 classification). Registry-retirement is an operator
# decision; the gate only asserts the set stays EXACTLY this.
LEGACY_SLUGS = {
    "backrun",
    "dex_arb",
    "funding_rate_arbitrage",
    "liquidation",
    "mean_reversion_arbitrage",
    "omega_strategy_pack",
    "triangular_arb",
}

MEV_RE = re.compile(r"MEV-[0-9]{2}-[0-9]{3}")
RUST_ID_RE = re.compile(r'\("?(MEV-[0-9]{2}-[0-9]{3})"?,')


def canonical_ids():
    rows = json.loads(CANONICAL_JSON.read_text(encoding="utf-8"))
    ids = [r["MEV_ID"] for r in rows]
    assert len(rows) == EXPECTED_CANONICAL, f"canonical rows {len(rows)} != {EXPECTED_CANONICAL}"
    assert len(set(ids)) == EXPECTED_CANONICAL, "duplicate MEV_ID in canonical JSON"
    return set(ids)


def rust_table_ids(path: Path):
    # The generated table literals precede the #[cfg(test)] module; test
    # asserts legitimately reference the MEV-99-999 negative-control sentinel
    # (assert hop_mask("MEV-99-999") == None) and must not count as entries.
    pre_tests = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    return set(RUST_ID_RE.findall(pre_tests))


def repo_scan():
    """Unique MEV-pattern IDs across git-tracked files, with per-ID file lists."""
    tracked = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.splitlines()
    occurrences = {}
    for rel in tracked:
        p = ROOT / rel
        if not p.is_file():
            continue
        try:
            text = p.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for mid in set(MEV_RE.findall(text)):
            occurrences.setdefault(mid, []).append(rel)
    return occurrences


def cmd_static():
    canonical = canonical_ids()
    ok = True

    # 2. Generated Rust tables == canonical, exactly.
    for name, path in RUST_TABLES.items():
        if not path.exists():
            print(f"FAIL: {name}: missing {path.relative_to(ROOT)}")
            ok = False
            continue
        ids = rust_table_ids(path)
        extra = ids - canonical
        missing = canonical - ids
        if extra or missing or len(ids) != EXPECTED_CANONICAL:
            ok = False
            print(f"FAIL: {name}: extra={sorted(extra)} missing={sorted(missing)} n={len(ids)}")
        else:
            print(f"rust {name}: {len(ids)} | parity OK")

    # 3. Repo-wide raw scan.
    occ = repo_scan()
    repo_ids = set(occ)
    extras = repo_ids - canonical
    missing = canonical - repo_ids
    unexpected = extras - set(EXTRAS_ALLOWLIST)
    for mid in sorted(unexpected):
        ok = False
        print(f"FAIL: unexpected MEV id {mid} in: {', '.join(occ[mid][:8])}")
    for mid in sorted(missing):
        ok = False
        print(f"FAIL: canonical id {mid} not present anywhere in tracked repo")
    classified = {mid: EXTRAS_ALLOWLIST[mid] for mid in sorted(extras & set(EXTRAS_ALLOWLIST))}
    print(
        f"repo scan: {len(repo_ids)} unique = {len(repo_ids & canonical)} canonical "
        f"+ {len(classified)} allowlisted ({classified or 'none'})"
    )
    print(f"canonical parity: {'264/264 OK' if canonical and len(canonical) == 264 else 'BROKEN'}")
    print("ALPHA-MAP-PARITY " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def norm_runtime_id(raw: str):
    m = re.match(r"mev_([0-9]{2})_([0-9]{3})_", raw)
    return f"MEV-{m.group(1)}-{m.group(2)}" if m else raw


def cmd_runtime(url: str, out: Path | None):
    canonical = canonical_ids()
    # nginx fronts the DApp with bot filtering on the default urllib UA — send
    # the same browser-like UA the revalidation probes use.
    req = urllib.request.Request(
        f"{url.rstrip('/')}/api/cartridges/runtime",
        headers={"accept": "application/json", "user-agent": "Mozilla/5.0 (X11; Linux x86_64) arbx-alpha-parity/1.0"},
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        payload = json.load(r)
    cartridges = (payload.get("data") or {}).get("cartridges") or []
    by_norm = {}
    for c in cartridges:
        by_norm.setdefault(norm_runtime_id(c.get("id", "")), []).append(c.get("id", ""))

    runtime_norm = set(by_norm)
    runtime_mev = {n for n in runtime_norm if MEV_RE.fullmatch(n)}
    legacy = {rid: by_norm[rid] for rid in sorted(runtime_norm - runtime_mev)}
    missing = canonical - runtime_norm
    extra_mev = runtime_mev - canonical

    ok = not missing and not extra_mev and set(legacy) <= LEGACY_SLUGS
    report = {
        "url": url,
        "source": payload.get("source"),
        "cartridges_total": len(cartridges),
        "canonical_present": len(canonical & runtime_mev),
        "canonical_missing": sorted(missing),
        "extra_mev_pattern": sorted(extra_mev),
        "legacy_slugs": {k: v for k, v in sorted(legacy.items())},
        "unexpected_legacy": sorted(set(legacy) - LEGACY_SLUGS),
        "verdict": "PASS" if ok else "FAIL",
    }
    text = json.dumps(report, indent=2)
    print(text)
    if out:
        out = out if out.is_absolute() else (ROOT / out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text + "\n", encoding="utf-8")
        # --out may legitimately point outside the repo (manual evidence runs);
        # never let the success path crash (and lie via exit code) on a
        # cosmetic relative_to. R8: report exactly where the file landed.
        try:
            shown = out.relative_to(ROOT)
        except ValueError:
            shown = out
        print(f"written: {shown}")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--runtime", metavar="URL", help="also export + diff the live runtime registry")
    ap.add_argument("--out", type=Path, help="runtime report output path (default docs/audits/alpha-map-parity-runtime.json)")
    args = ap.parse_args()

    rc = cmd_static()
    if args.runtime:
        out = args.out or (ROOT / "docs" / "audits" / "alpha-map-parity-runtime.json")
        rc = cmd_runtime(args.runtime, out) or rc
    return rc


if __name__ == "__main__":
    sys.exit(main())
