"""Regression against real PostgreSQL; run in CI with the postgres service.

Uses only psql from the already-running container, no apt or Python packages.
The database is disposable and contains no business data.
"""
import os
from pathlib import Path
import subprocess
import time
import unittest


class MigrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.command = ["docker", "exec", "-i", "-e",
                       "PGOPTIONS=-c lock_timeout=500ms -c statement_timeout=10s",
                       os.environ["PG_TEST_CONTAINER"], "psql", "-X", "-U", "postgres",
                       "-d", "migration_regression", "-v", "ON_ERROR_STOP=1", "-At"]
        cls.migration = Path("database/migrations/103_math_evidence_scoring.sql").read_text()

    def sql(self, sql, success=True):
        result = subprocess.run(self.command, input=sql, text=True, capture_output=True, timeout=15)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def setUp(self):
        self.sql("DROP TABLE IF EXISTS math_operator_calibration; "
                 "DROP TABLE IF EXISTS scored_opportunities; "
                 "CREATE TABLE scored_opportunities(id bigint PRIMARY KEY);")

    def test_fresh_and_partial_schema(self):
        for partial in [False, True]:
            with self.subTest(partial=partial):
                self.setUp()
                if partial:
                    self.sql("ALTER TABLE scored_opportunities ADD evidence_vector jsonb;")
                self.sql(self.migration)
                count = self.sql("SELECT count(*) FROM pg_attribute WHERE "
                                 "attrelid='scored_opportunities'::regclass AND NOT attisdropped "
                                 "AND attname IN ('evidence_vector','evidence_computed_at');")
                self.assertEqual(count.stdout.strip(), "2")
                self.assertEqual(self.sql("SELECT count(*) FROM math_operator_calibration;").stdout.strip(), "0")

    def test_wrong_type_aborts_and_rolls_back(self):
        self.sql("ALTER TABLE scored_opportunities ADD evidence_computed_at text;")
        failure = self.sql(self.migration, success=False)
        self.assertIn("Schema drift", failure.stderr)
        # The first ADD in the transaction must roll back too.
        self.assertEqual(self.sql("SELECT count(*) FROM pg_attribute WHERE "
                                 "attrelid='scored_opportunities'::regclass "
                                 "AND attname='evidence_vector' AND NOT attisdropped;").stdout.strip(), "0")

    def hold_writer(self):
        holder = subprocess.Popen(self.command, stdin=subprocess.PIPE,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        holder.stdin.write("BEGIN; LOCK TABLE scored_opportunities IN ROW EXCLUSIVE MODE; SELECT pg_sleep(4); COMMIT;\n")
        holder.stdin.close()
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            locked = self.sql("SELECT count(*) FROM pg_locks WHERE relation="
                              "'scored_opportunities'::regclass AND mode='RowExclusiveLock' AND granted;")
            if locked.stdout.strip() == "1":
                self.addCleanup(lambda: holder.wait(timeout=10))
                return
            time.sleep(0.05)
        holder.wait(timeout=10)
        self.fail("Writer lock was not acquired")

    def test_replay_does_not_queue_exclusive_lock(self):
        self.sql(self.migration)
        self.hold_writer()
        # Demonstrate that the old migration fails under this exact lock.
        old = self.sql("ALTER TABLE scored_opportunities ADD COLUMN IF NOT EXISTS evidence_vector jsonb;", success=False)
        self.assertIn("lock timeout", old.stderr)
        self.sql(self.migration)

    def test_missing_columns_still_require_real_lock(self):
        self.hold_writer()
        failure = self.sql(self.migration, success=False)
        self.assertIn("lock timeout", failure.stderr)
        self.assertEqual(self.sql("SELECT to_regclass('math_operator_calibration') IS NULL;").stdout.strip(), "t")


if __name__ == "__main__":
    unittest.main()
