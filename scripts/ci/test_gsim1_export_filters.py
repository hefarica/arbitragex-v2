#!/usr/bin/env python3
"""POBLACION-FILTER-01 — the G-SIM-1 export must mirror the sim's own shape gate.

Measured defect (2026-10-23): `scripts/gsim1_variance_export.sql` filtered the
export population on `jsonb_array_length(dex_adapters) = 2` but NOT on the
token-path half of the harness's own per-row gate
(backend/sim-core/tests/variance_benchmark.rs):

    if row.dex_adapters.len() != 2 || row.token_addresses.len() != 3 {
        hist.bad_shape += 1; continue;   // skip:bad_shape
    }

so rows with a malformed token path (or with NO token_addresses key at all —
the export then emits `"token_addresses": null` and the harness PANICS on
deserialization, killing the whole benchmark run) were exported, occupied the
DISTINCT/LIMIT 500 population budget, and were then skipped by the sim. The
export IS the labelable population of the window: a row the sim itself cannot
process must not be in it.

The same commit tightens the adapter mirror to the EXACT set of
`adapter_to_semantic()` (trim + case-sensitive: UniswapV2 | uniswap-v2 |
uniswapv2 | SushiSwap | sushi | sushiswap). The previous mirror compared
lower(leg) IN (four lowercase tokens) — a case-insensitive SUPERSET that
exported spellings (e.g. 'UNISWAP-V2') the harness itself skips as
unsupported_adapter.

Honest note on the ADAPTER-CASE-01 fixture (2026-10-20): every row of that
first fixture carried token_addresses of length 2 — the exact shape the
harness skips as bad_shape. The adapter axis was therefore validated against
rows the sim itself would never label, which is how the missing token-path
filter went unnoticed. This fixture gives every EXPORTED row the real 2-leg
round-trip shape measured in production (token path length 3, closed cycle
[token_in, intermediate, token_in]: 4,423,902/4,423,902 two-leg rows).

This test runs the REAL export SQL against a throwaway PostgreSQL and asserts,
in one place:
  * RED BEFORE (ADAPTER-CASE-01)  — the legacy `<@` predicate returns 0 rows
    for a kebab 2-leg row;
  * RED BEFORE (POBLACION-FILTER-01) — the pre-fix WHERE (verbatim below)
    exports 6 rows: the 2 good ones PLUS 4 rows the sim itself cannot process
    (token path 2, token path 4, casing-superset adapter, missing token key);
  * GREEN AFTER — the shipped export returns EXACTLY the 2 good rows
    (positive control: the filter is not "exclude everything");
  * the scope filters still hold (V3 leg, 3 legs, zero amount, stale
    timestamp).

Run: PG_DSN=postgresql://postgres:postgres@127.0.0.1:5432/postgres \
        python3 scripts/ci/test_gsim1_export_filters.py
Requires: psql on PATH (CI: services: postgres:15).
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXPORT_SQL = ROOT / "scripts" / "gsim1_variance_export.sql"
DSN = os.environ.get("PG_DSN", "postgresql://postgres:postgres@127.0.0.1:5432/postgres")

failures: list[str] = []
checks = 0


def check(name: str, expected, actual) -> None:
    global checks
    checks += 1
    if expected == actual:
        print(f"  ok    {name}")
    else:
        failures.append(name)
        print(f"  FAIL  {name}\n        expected: {expected!r}\n        actual:   {actual!r}")


def psql(sql: str) -> str:
    return subprocess.run(
        ["psql", DSN, "-v", "ON_ERROR_STOP=1", "-At", "-c", sql],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def psql_file(path: Path) -> list[str]:
    out = subprocess.run(
        ["psql", DSN, "-v", "ON_ERROR_STOP=1", "-At", "-f", str(path)],
        check=True, capture_output=True, text=True,
    ).stdout
    return [line for line in out.splitlines() if line.strip().startswith("{")]


FIXTURE = textwrap.dedent(
    """
    DROP TABLE IF EXISTS opportunities;
    CREATE TABLE opportunities (
      id            uuid PRIMARY KEY,
      chain_id      integer NOT NULL,
      dex_a         text,
      token_in      text,
      token_out     text,
      amount_in_wei numeric,
      detected_at   timestamptz NOT NULL,
      route_metadata jsonb
    );

    INSERT INTO opportunities (id, chain_id, dex_a, token_in, token_out, amount_in_wei, detected_at, route_metadata) VALUES
      -- 1. KEBAB 2-leg, both uniswap-v2, real round-trip token path [in, mid, in]:
      --    the row the ADAPTER-CASE-01 legacy filter dropped. MUST be exported.
      ('00000000-0000-0000-0000-000000000001', 1, 'uniswap-v2', '0xa01', '0xa02', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xp1","0xp2"],"token_addresses":["0xa01","0xa02","0xa01"]}'),
      -- 2. CamelCase 2-leg with real round-trip token path: exported by every
      --    predicate generation (POSITIVE CONTROL — the fix must not exclude it).
      ('00000000-0000-0000-0000-000000000002', 1, 'UniswapV2', '0xb01', '0xb02', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["UniswapV2","SushiSwap"],"pool_addresses":["0xp3","0xp4"],"token_addresses":["0xb01","0xb02","0xb01"]}'),
      -- 3. 2-leg but one V3 leg: unsupported by the A.3.a encoder -> excluded.
      ('00000000-0000-0000-0000-000000000003', 1, 'uniswap-v3', '0xc01', '0xc02', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v3"],"pool_addresses":["0xp5","0xp6"],"token_addresses":["0xc01","0xc02","0xc01"]}'),
      -- 4. 3 legs: not a round trip -> excluded.
      ('00000000-0000-0000-0000-000000000004', 1, 'uniswap-v2', '0xd01', '0xd02', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2","uniswap-v2"],"pool_addresses":["0xp7","0xp8","0xp9"],"token_addresses":["0xd01","0xd02","0xd01"]}'),
      -- 5. zero amount -> excluded.
      ('00000000-0000-0000-0000-000000000005', 1, 'uniswap-v2', '0xe01', '0xe02', 0, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xpa","0xpb"],"token_addresses":["0xe01","0xe02","0xe01"]}'),
      -- 6. stale (3 h) -> outside the freshness window.
      ('00000000-0000-0000-0000-000000000006', 1, 'uniswap-v2', '0xf01', '0xf02', 1000000000000000, now() - interval '3 hours',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xpc","0xpd"],"token_addresses":["0xf01","0xf02","0xf01"]}'),
      -- 7. POBLACION-FILTER-01 defect row A: 2-leg, allowed adapters, but a
      --    2-entry token path (the shape of EVERY row in the 2026-10-20
      --    fixture, and a shape the harness skips as skip:bad_shape). The
      --    pre-fix export included it; the fixed export must not.
      ('00000000-0000-0000-0000-000000000007', 1, 'uniswap-v2', '0x107', '0x108', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xpe","0xpf"],"token_addresses":["0x107","0x108"]}'),
      -- 8. defect row B: 4-entry token path (open 3-hop path, not a round
      --    trip). Pre-fix export included it; fixed export must not.
      ('00000000-0000-0000-0000-000000000008', 1, 'uniswap-v2', '0x208', '0x209', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0x10","0x11"],"token_addresses":["0x208","0x209","0x20a","0x208"]}'),
      -- 9. defect row C: adapters spelled 'UNISWAP-V2' — passes the pre-fix
      --    case-insensitive lower() mirror, but adapter_to_semantic() is a
      --    trim + CASE-SENSITIVE match over six spellings and rejects it
      --    (skip:unsupported_adapter in the harness). Fixed export must not
      --    include it.
      ('00000000-0000-0000-0000-000000000009', 1, 'UNISWAP-V2', '0x309', '0x30a', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["UNISWAP-V2","UNISWAP-V2"],"pool_addresses":["0x12","0x13"],"token_addresses":["0x309","0x30a","0x309"]}'),
      -- 10. defect row D: NO token_addresses key at all. The pre-fix export
      --     emitted '"token_addresses": null' for it and the harness PANICKED
      --     on deserialization (one row killed the whole benchmark run).
      --     Fixed export must not include it.
      ('00000000-0000-0000-0000-00000000000a', 1, 'uniswap-v2', '0x40b', '0x40c', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0x14","0x15"]}');
    """
)

# The predicate as it shipped before ADAPTER-CASE-01, verbatim.
LEGACY_PREDICATE = """
SELECT count(*) FROM opportunities
WHERE chain_id = 1
  AND jsonb_array_length(route_metadata->'dex_adapters') = 2
  AND (route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb;
"""

# The WHERE clause as it shipped BEFORE POBLACION-FILTER-01, verbatim: no
# token-path shape filter and a case-insensitive adapter mirror.
PRE_POBLACION_FILTER_PREDICATE = """
SELECT count(*) FROM opportunities
WHERE chain_id = 1
  AND route_metadata IS NOT NULL
  AND route_metadata::text NOT IN ('', '{}')
  AND route_metadata ? 'dex_adapters'
  AND jsonb_array_length(route_metadata->'dex_adapters') = 2
  AND (SELECT bool_and(lower(leg) IN ('uniswapv2', 'uniswap-v2', 'sushi', 'sushiswap'))
       FROM jsonb_array_elements_text(route_metadata->'dex_adapters') AS legs(leg))
  AND amount_in_wei IS NOT NULL
  AND amount_in_wei <> '0'
  AND detected_at > now() - interval '2 hours';
"""


def ids(rows: list[str]) -> set[str]:
    return {json.loads(r)["opportunity_id"] for r in rows}


GOOD = {"00000000-0000-0000-0000-000000000001", "00000000-0000-0000-0000-000000000002"}
DEFECT = {
    "00000000-0000-0000-0000-000000000007",  # token path length 2 -> bad_shape
    "00000000-0000-0000-0000-000000000008",  # token path length 4 -> bad_shape
    "00000000-0000-0000-0000-000000000009",  # casing superset -> unsupported_adapter
    "00000000-0000-0000-0000-00000000000a",  # missing token key -> harness panic
}


def main() -> int:
    if not EXPORT_SQL.exists():
        print(f"FATAL: {EXPORT_SQL} not found", file=sys.stderr)
        return 2

    psql(FIXTURE)

    # RED BEFORE (ADAPTER-CASE-01): the old predicate cannot see the kebab row.
    legacy_total = int(psql(LEGACY_PREDICATE))
    check("legacy predicate sees only the CamelCase row (proves the 2026-10-20 defect)", 1, legacy_total)

    # RED BEFORE (POBLACION-FILTER-01): the pre-fix WHERE exports the 2 good
    # rows PLUS all 4 defect rows the sim itself cannot process.
    pre_total = int(psql(PRE_POBLACION_FILTER_PREDICATE))
    check("pre-fix WHERE exports 6 rows (2 good + 4 the sim cannot label — proves the defect)", 6, pre_total)

    exported = psql_file(EXPORT_SQL)
    got = ids(exported)
    check("export returns exactly the 2 labelable rows (positive control: not 'exclude everything')", GOOD, got)
    check("kebab 2-leg round trip exported (ADAPTER-CASE-01 regression guard)", True,
          "00000000-0000-0000-0000-000000000001" in got)
    check("CamelCase 2-leg round trip exported", True,
          "00000000-0000-0000-0000-000000000002" in got)
    check("token path length 2 excluded (sim skips it as bad_shape)", False,
          "00000000-0000-0000-0000-000000000007" in got)
    check("token path length 4 excluded (open path, not a round trip)", False,
          "00000000-0000-0000-0000-000000000008" in got)
    check("adapter casing outside adapter_to_semantic excluded", False,
          "00000000-0000-0000-0000-000000000009" in got)
    check("row without token_addresses key excluded (harness would panic on null)", False,
          "00000000-0000-0000-0000-00000000000a" in got)
    check("V3 leg excluded (encoder scope)", False, "00000000-0000-0000-0000-000000000003" in got)
    check("3-leg route excluded", False, "00000000-0000-0000-0000-000000000004" in got)
    check("zero amount excluded", False, "00000000-0000-0000-0000-000000000005" in got)
    check("stale row excluded", False, "00000000-0000-0000-0000-000000000006" in got)

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
