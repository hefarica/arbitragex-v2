#!/usr/bin/env python3
"""ADAPTER-CASE-01 — the G-SIM-1 export must not filter on adapter SPELLING.

Measured defect (2026-10-20): `scripts/gsim1_variance_export.sql` filtered with

    (route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb

which is an exact, case-SENSITIVE array containment, while the scanner persists
kebab-case ('uniswap-v2'). A real 2-leg UniswapV2 route was dropped by the
export and the evidence row kept reporting "0 distinct A.3.a-encodable
topologies" — an instrument failure wearing the mask of an empty market.

This test runs the REAL export SQL against a throwaway PostgreSQL and asserts,
in one place:
  * RED BEFORE   — the legacy predicate returns 0 rows for a kebab 2-leg row;
  * GREEN AFTER  — the shipped export returns it;
  * the scope filters still hold (V3 leg, 3 legs, zero amount, stale timestamp).

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
      -- 1. KEBAB 2-leg, both UniswapV2: the row the legacy filter dropped.
      ('00000000-0000-0000-0000-000000000001', 1, 'uniswap-v2', '0xaaa', '0xbbb', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xp1","0xp2"],"token_addresses":["0xaaa","0xbbb"]}'),
      -- 2. CamelCase 2-leg: exported by both predicates (control).
      ('00000000-0000-0000-0000-000000000002', 1, 'UniswapV2', '0xccc', '0xddd', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["UniswapV2","SushiSwap"],"pool_addresses":["0xp3","0xp4"],"token_addresses":["0xccc","0xddd"]}'),
      -- 3. 2-leg but one V3 leg: unsupported by the A.3.a encoder -> excluded.
      ('00000000-0000-0000-0000-000000000003', 1, 'uniswap-v3', '0xeee', '0xfff', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v3"],"pool_addresses":["0xp5","0xp6"],"token_addresses":["0xeee","0xfff"]}'),
      -- 4. 3 legs: not a round trip -> excluded.
      ('00000000-0000-0000-0000-000000000004', 1, 'uniswap-v2', '0x111', '0x222', 1000000000000000, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2","uniswap-v2"],"pool_addresses":["0xp7","0xp8"],"token_addresses":["0x111","0x222"]}'),
      -- 5. zero amount -> excluded.
      ('00000000-0000-0000-0000-000000000005', 1, 'uniswap-v2', '0x333', '0x444', 0, now() - interval '10 minutes',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xp9","0xpa"],"token_addresses":["0x333","0x444"]}'),
      -- 6. stale (3 h) -> outside the freshness window.
      ('00000000-0000-0000-0000-000000000006', 1, 'uniswap-v2', '0x555', '0x666', 1000000000000000, now() - interval '3 hours',
       '{"dex_adapters":["uniswap-v2","uniswap-v2"],"pool_addresses":["0xpb","0xpc"],"token_addresses":["0x555","0x666"]}');
    """
)

# The predicate as it shipped before ADAPTER-CASE-01, verbatim.
LEGACY_PREDICATE = """
SELECT count(*) FROM opportunities
WHERE chain_id = 1
  AND jsonb_array_length(route_metadata->'dex_adapters') = 2
  AND (route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb;
"""


def ids(rows: list[str]) -> set[str]:
    return {json.loads(r)["opportunity_id"] for r in rows}


def main() -> int:
    if not EXPORT_SQL.exists():
        print(f"FATAL: {EXPORT_SQL} not found", file=sys.stderr)
        return 2

    psql(FIXTURE)

    # RED BEFORE: the old predicate cannot see the kebab row.
    legacy_total = int(psql(LEGACY_PREDICATE))
    check("legacy predicate sees only the CamelCase row (proves the defect)", 1, legacy_total)

    exported = psql_file(EXPORT_SQL)
    got = ids(exported)
    check("export returns exactly 2 rows", 2, len(exported))
    check("kebab 2-leg row is exported (was invisible before)", True,
          "00000000-0000-0000-0000-000000000001" in got)
    check("CamelCase 2-leg row still exported (no regression)", True,
          "00000000-0000-0000-0000-000000000002" in got)
    check("V3 leg excluded (encoder scope)", False, "00000000-0000-0000-0000-000000000003" in got)
    check("3-leg route excluded", False, "00000000-0000-0000-0000-000000000004" in got)
    check("zero amount excluded", False, "00000000-0000-0000-0000-000000000005" in got)
    check("stale row excluded", False, "00000000-0000-0000-0000-000000000006" in got)

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
