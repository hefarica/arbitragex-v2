#!/usr/bin/env python3
"""Test the actual retention VACUUM argv against disposable PostgreSQL 15.

No production settings, endpoints, host mounts or volumes are used. The source
script is parsed, NEVER sourced or executed: none of its purge logic runs.
Default invocation requires Docker and runs every test; --static-only explicitly
runs only the three parser checks (not a PostgreSQL validation).
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import time
import unittest
import uuid

SOURCE = Path(__file__).resolve().parents[1] / "pg_retention.sh"
DOCKER = ["docker", "--context", "default"]


def vacuum_args(source: str, container: str, lock_timeout: str = "5s") -> list[str]:
    marker = "  # VACUUM ANALYZE ("
    if source.count(marker) != 1:
        raise ValueError("expected exactly one canonical VACUUM section")
    section = source.split(marker, 1)[1]
    match = re.search(r'(docker exec -i "\$PG_CONTAINER" psql.*?)\s+>/dev/null', section, re.S)
    if match is None:
        raise ValueError("canonical VACUUM command not found")
    args = shlex.split(match.group(1).replace("\\\n", ""))
    if args[:5] != ["docker", "exec", "-i", "$PG_CONTAINER", "psql"]:
        raise ValueError("unexpected command prefix")
    args = [a.replace("$PG_CONTAINER", container)
            .replace("$BATCH_LOCK_TIMEOUT", lock_timeout)
            .replace("$tbl", "vacuum_probe") for a in args]
    if any("$" in arg for arg in args):
        raise ValueError("unexpected shell expansion in VACUUM command")
    return DOCKER + args[1:]


def zero_fill_sql(source: str) -> str:
    marker = "ROLLUP-GAP-01 (2026-09-22): zero-fill honesto"
    if source.count(marker) != 1:
        raise ValueError("expected exactly one ROLLUP-GAP-01 zero-fill marker")
    section = source.split(marker, 1)[1]
    match = re.search(r'psql_batch "\n(.*?)"\s*\| tail', section, re.S)
    if match is None:
        raise ValueError("zero-fill SQL not found after marker")
    sql = match.group(1).replace("\\\n", "").strip()
    if "$" in sql:
        raise ValueError("unexpected shell expansion in zero-fill SQL")
    return sql


def commands(args: list[str]) -> list[str]:
    return [args[i + 1] for i, arg in enumerate(args[:-1]) if arg == "-c"]


class CommandShape(unittest.TestCase):
    def test_independent_bounded_requests(self) -> None:
        args = vacuum_args(SOURCE.read_text(encoding="utf-8"), "test-container")
        self.assertEqual(commands(args), ["SET lock_timeout='5s'",
                         "SET statement_timeout='600s'", "VACUUM (ANALYZE) vacuum_probe"])
        self.assertIn("ON_ERROR_STOP=1", args)
        self.assertIn("-X", args)
        self.assertNotIn("FULL", " ".join(commands(args)))

    def test_missing_section_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            vacuum_args("echo no maintenance", "test-container")

    def test_duplicate_section_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            vacuum_args(SOURCE.read_text(encoding="utf-8") * 2, "test-container")


class PostgreSQL15(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if shutil.which("docker") is None:
            raise RuntimeError("Docker is required; PostgreSQL tests were NOT executed")
        # No ports, network, host bind mounts or persistent volumes. Cleanup is
        # limited to the exact ID created by this test, never a production name.
        name = "arbx-vacuum-ci-" + uuid.uuid4().hex
        result = subprocess.run(DOCKER + ["run", "--detach", "--rm", "--network", "none",
            "--name", name, "--label", "arbx.test=retention-vacuum",
            "--tmpfs", "/var/lib/postgresql/data:rw,noexec,nosuid,size=256m",
            "-e", "POSTGRES_HOST_AUTH_METHOD=trust", "-e", "POSTGRES_DB=arbitragex",
            "postgres:15"], text=True, capture_output=True, timeout=180, check=True)
        cls.container = result.stdout.strip()
        if not re.fullmatch(r"[0-9a-f]{64}", cls.container):
            raise RuntimeError("Docker did not return an unambiguous container ID")
        cls.addClassCleanup(cls.cleanup)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            ready = subprocess.run(DOCKER + ["exec", cls.container, "sh", "-c",
                'test "$(cat /proc/1/comm)" = postgres && pg_isready -U postgres -d arbitragex'],
                capture_output=True, timeout=5)
            if ready.returncode == 0:
                return
            time.sleep(0.25)
        raise RuntimeError("disposable PostgreSQL did not become ready")

    @classmethod
    def cleanup(cls) -> None:
        subprocess.run(DOCKER + ["rm", "--force", cls.container],
                       capture_output=True, timeout=30, check=True)

    def sql(self, statement: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(DOCKER + ["exec", self.container, "psql", "-U", "postgres",
            "-d", "arbitragex", "-X", "-qAt", "-v", "ON_ERROR_STOP=1",
            "-v", "VERBOSITY=verbose", "-c", statement],
            text=True, capture_output=True, timeout=20)

    def setUp(self) -> None:
        fixture = self.sql("DROP TABLE IF EXISTS vacuum_probe; "
            "CREATE TABLE vacuum_probe(id integer); "
            "INSERT INTO vacuum_probe VALUES (1), (2); DELETE FROM vacuum_probe WHERE id=2")
        self.assertEqual(fixture.returncode, 0, fixture.stderr)

    def test_reproduces_original_transaction_error(self) -> None:
        result = self.sql("SET statement_timeout='600s'; VACUUM (ANALYZE) vacuum_probe")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("25001", result.stderr)

    def test_current_source_command_runs_real_vacuum(self) -> None:
        args = vacuum_args(SOURCE.read_text(encoding="utf-8"), self.container)
        result = subprocess.run(args, text=True, capture_output=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        count = self.sql("SELECT count(*) FROM vacuum_probe")
        self.assertEqual(count.returncode, 0, count.stderr)
        self.assertEqual(count.stdout.strip(), "1")

    def test_invalid_timeout_is_not_silently_ignored(self) -> None:
        args = vacuum_args(SOURCE.read_text(encoding="utf-8"), self.container, "not-a-duration")
        result = subprocess.run(args, text=True, capture_output=True, timeout=20)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("lock_timeout", result.stderr)


class ZeroFillShape(unittest.TestCase):
    def test_extracts_zero_fill_sql(self) -> None:
        sql = zero_fill_sql(SOURCE.read_text(encoding="utf-8"))
        self.assertIn("INSERT INTO route_discovery_outcome_rollup_5m", sql)
        self.assertIn("'__totals__'", sql)

    def test_honesty_guard_present(self) -> None:
        # R8/RULE 00: a bucket that still has raw rows must NEVER be zero-filled —
        # only buckets with no crudo may receive an honest zero row.
        sql = zero_fill_sql(SOURCE.read_text(encoding="utf-8"))
        self.assertRegex(sql, r"(?s)NOT EXISTS.*FROM route_discovery_outcomes\b")
        self.assertRegex(sql, r"ON CONFLICT DO NOTHING")

    def test_missing_marker_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            zero_fill_sql("echo no zero fill")

    def test_duplicate_marker_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            zero_fill_sql(SOURCE.read_text(encoding="utf-8") * 2)


class RollupZeroFill(unittest.TestCase):
    """ROLLUP-GAP-01: zero-fill honesto contra PostgreSQL 15 desechable.

    Escenario (espejo del incidente 2026-09-22): buckets vacíos intercalados
    quedan sin fila __totals__ tras agotarse el presupuesto del backfill; el
    zero-fill debe materializar SOLO esos, nunca un bucket con crudo pendiente
    de rollup, y debe ser idempotente.
    """

    @classmethod
    def setUpClass(cls) -> None:
        if shutil.which("docker") is None:
            raise RuntimeError("Docker is required; PostgreSQL tests were NOT executed")
        name = "arbx-zerofill-ci-" + uuid.uuid4().hex
        result = subprocess.run(DOCKER + ["run", "--detach", "--rm", "--network", "none",
            "--name", name, "--label", "arbx.test=retention-zerofill",
            "--tmpfs", "/var/lib/postgresql/data:rw,noexec,nosuid,size=256m",
            "-e", "POSTGRES_HOST_AUTH_METHOD=trust", "-e", "POSTGRES_DB=arbitragex",
            "postgres:15"], text=True, capture_output=True, timeout=180, check=True)
        cls.container = result.stdout.strip()
        if not re.fullmatch(r"[0-9a-f]{64}", cls.container):
            raise RuntimeError("Docker did not return an unambiguous container ID")
        cls.addClassCleanup(cls.cleanup)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            ready = subprocess.run(DOCKER + ["exec", cls.container, "sh", "-c",
                'test "$(cat /proc/1/comm)" = postgres && pg_isready -U postgres -d arbitragex'],
                capture_output=True, timeout=5)
            if ready.returncode == 0:
                return
            time.sleep(0.25)
        raise RuntimeError("disposable PostgreSQL did not become ready")

    @classmethod
    def cleanup(cls) -> None:
        subprocess.run(DOCKER + ["rm", "--force", cls.container],
                       capture_output=True, timeout=30, check=True)

    def sql(self, statement: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(DOCKER + ["exec", self.container, "psql", "-U", "postgres",
            "-d", "arbitragex", "-X", "-qAt", "-v", "ON_ERROR_STOP=1",
            "-c", statement], text=True, capture_output=True, timeout=20)

    def setUp(self) -> None:
        fixture = self.sql(
            "DROP TABLE IF EXISTS route_discovery_outcomes;"
            " DROP TABLE IF EXISTS route_discovery_outcome_rollup_5m;"
            " CREATE TABLE route_discovery_outcomes(ts_ms bigint);"
            " CREATE TABLE route_discovery_outcome_rollup_5m("
            " dim text, key text, bucket_ms bigint, n bigint, opportunities bigint,"
            " with_reserves bigint, profit_gt0 bigint, PRIMARY KEY(dim, key, bucket_ms));")
        self.assertEqual(fixture.returncode, 0, fixture.stderr)

    def test_zero_fill_fills_only_buckets_without_crudo(self) -> None:
        # 4 buckets in [oldest, last_complete]: crudo in the 3 oldest, the
        # last_complete bucket empty. __totals__ materialized for only 2 of
        # the 3 crudo buckets (backfill budget exhausted mid-backlog).
        setup = self.sql(
            "INSERT INTO route_discovery_outcomes"
            " SELECT (floor(extract(epoch FROM now())*1000)::bigint/300000*300000 - 300000)"
            "       - (g * 300000) FROM generate_series(1, 3) AS g;")
        self.assertEqual(setup.returncode, 0, setup.stderr)
        setup2 = self.sql(
            "INSERT INTO route_discovery_outcome_rollup_5m"
            " SELECT '__totals__', '', (min(ts_ms)/300000::bigint*300000), 7, 1, 1, 0"
            " FROM route_discovery_outcomes"
            " UNION ALL"
            " SELECT '__totals__', '', (min(ts_ms)/300000::bigint*300000) + 300000, 9, 2, 2, 1"
            " FROM route_discovery_outcomes")
        self.assertEqual(setup2.returncode, 0, setup2.stderr)

        sql = zero_fill_sql(SOURCE.read_text(encoding="utf-8"))
        result = self.sql(sql)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("INSERT 0", result.stdout)

        # The empty last_complete bucket now carries an honest zero row...
        zeros = self.sql(
            "SELECT count(*) FROM route_discovery_outcome_rollup_5m rr"
            " WHERE rr.dim='__totals__' AND rr.n=0 AND NOT EXISTS ("
            "  SELECT 1 FROM route_discovery_outcomes r"
            "  WHERE r.ts_ms >= rr.bucket_ms AND r.ts_ms < rr.bucket_ms + 300000)")
        self.assertEqual(zeros.stdout.strip(), "1", zeros.stderr)
        # ...pre-existing rows are untouched...
        kept = self.sql(
            "SELECT n FROM route_discovery_outcome_rollup_5m rr WHERE rr.n > 0 ORDER BY rr.bucket_ms")
        self.assertEqual(kept.stdout.split(), ["7", "9"], kept.stderr)
        # ...and the crudo bucket still pending rollup was NOT zero-filled
        # (RULE 00/R8: zero would hide real data the backfill must still count).
        pending_bucket = self.sql(
            "SELECT count(*) FROM route_discovery_outcome_rollup_5m rr"
            " WHERE rr.bucket_ms = (SELECT min(ts_ms)/300000::bigint*300000 + 600000"
            "                       FROM route_discovery_outcomes)")
        self.assertEqual(pending_bucket.stdout.strip(), "0", pending_bucket.stderr)

        # Idempotent: a second run inserts nothing.
        again = self.sql(sql)
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertIn("INSERT 0 0", again.stdout)

        # Once the backfill completes the pending bucket (simulated here),
        # the coverage gate finds zero missing buckets in [oldest, last_complete].
        complete = self.sql(
            "INSERT INTO route_discovery_outcome_rollup_5m"
            " SELECT '__totals__', '', (min(ts_ms)/300000::bigint*300000) + 600000, 11, 3, 3, 1"
            " FROM route_discovery_outcomes")
        self.assertEqual(complete.returncode, 0, complete.stderr)
        gate = self.sql(
            "WITH oldest AS (SELECT min(ts_ms)/300000::bigint*300000 AS b"
            " FROM route_discovery_outcomes)"
            " SELECT count(*) FROM generate_series((SELECT b FROM oldest),"
            " (SELECT floor(extract(epoch FROM now())*1000)::bigint/300000*300000 - 300000),"
            " 300000) g(bucket) WHERE NOT EXISTS ("
            "  SELECT 1 FROM route_discovery_outcome_rollup_5m rr"
            "  WHERE rr.dim='__totals__' AND rr.bucket_ms=g.bucket)")
        self.assertEqual(gate.stdout.strip(), "0", gate.stderr)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--static-only", action="store_true")
    options = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CommandShape)
    suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(ZeroFillShape))
    if not options.static_only:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(PostgreSQL15))
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(RollupZeroFill))
    else:
        print("STATIC ONLY: PostgreSQL integration tests were NOT executed", flush=True)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
