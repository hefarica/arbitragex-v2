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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--static-only", action="store_true")
    options = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CommandShape)
    if not options.static_only:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(PostgreSQL15))
    else:
        print("STATIC ONLY: PostgreSQL integration tests were NOT executed", flush=True)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
