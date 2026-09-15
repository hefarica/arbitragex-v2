"""Exercise the exact production Lua on disposable loopback Redis, never the VPS."""
from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import re
import socket
import threading
import unittest
import uuid

if os.environ.get("ARBX_V3_FEE_DISPOSABLE_REDIS") != "1":
    raise SystemExit("Set ARBX_V3_FEE_DISPOSABLE_REDIS=1 for the CI fixture on 127.0.0.1:36379 only")

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "backend/searcher-rs/src/pool_discovery/v3_fee.rs"
MATCH = re.search(r'INDEX_COMPARE_AND_SET: &str = r#"(.*?)"#;', SOURCE.read_text(encoding="utf-8"), re.S)
if not MATCH:
    raise SystemExit("Production compare-and-set script not found")
SCRIPT = MATCH.group(1)


def command(*parts):
    encoded = [str(part).encode("utf-8") for part in parts]
    wire = b"*%d\r\n" % len(encoded) + b"".join(b"$%d\r\n" % len(part) + part + b"\r\n" for part in encoded)
    with socket.create_connection(("127.0.0.1", 36379), timeout=3) as sock:
        sock.sendall(wire)
        with sock.makefile("rb") as stream:
            header = stream.readline(65536)
            if not header.endswith(b"\r\n"):
                raise RuntimeError("Invalid Redis response")
            kind, value = header[:1], header[1:-2]
            if kind == b"-":
                raise RuntimeError(value.decode("utf-8", errors="replace"))
            if kind == b"+":
                return value.decode("utf-8")
            if kind == b":":
                return int(value)
            if kind == b"$":
                length = int(value)
                if length == -1:
                    return None
                if not 0 <= length <= 1024 * 1024:
                    raise RuntimeError("Unbounded Redis fixture response")
                raw = stream.read(length + 2)
                if len(raw) != length + 2 or not raw.endswith(b"\r\n"):
                    raise RuntimeError("Incomplete Redis fixture response")
                return raw[:-2].decode("utf-8")
            raise RuntimeError("Unexpected Redis response type")


class CompareAndSetTests(unittest.TestCase):
    def setUp(self):
        self.key = "arbx:v3-fee-test:" + uuid.uuid4().hex
        self.extra = self.key + ":other"
        self.assertEqual(command("PING"), "PONG")

    def tearDown(self):
        command("DEL", self.key, self.extra)

    def cas(self, previous, updated):
        return command("EVAL", SCRIPT, 1, self.key, "0" if previous is None else "1", previous or "", updated)

    def test_create_only_when_observed_absent(self):
        self.assertEqual(self.cas(None, "new"), 1)
        self.assertEqual(command("GET", self.key), "new")

    def test_update_the_exact_observed_value(self):
        command("SET", self.key, "old")
        self.assertEqual(self.cas("old", "verified"), 1)
        self.assertEqual(command("GET", self.key), "verified")

    def test_stale_value_cannot_overwrite_concurrent_update(self):
        command("SET", self.key, "competitor")
        self.assertEqual(self.cas("old", "stale"), 0)
        self.assertEqual(command("GET", self.key), "competitor")

    def test_missing_observation_cannot_overwrite_existing_key(self):
        command("SET", self.key, "existing")
        self.assertEqual(self.cas(None, "stale"), 0)
        self.assertEqual(command("GET", self.key), "existing")

    def test_deleted_value_is_not_resurrected_from_stale_observation(self):
        self.assertEqual(self.cas("old", "stale"), 0)
        self.assertIsNone(command("GET", self.key))

    def test_unrecognized_presence_flag_cannot_write(self):
        with self.assertRaisesRegex(RuntimeError, "invalid_observation_flag"):
            command("EVAL", SCRIPT, 1, self.key, "bad", "", "value")
        self.assertIsNone(command("GET", self.key))

    def test_two_competing_writers_have_exactly_one_winner(self):
        command("SET", self.key, "old")
        barrier = threading.Barrier(2)
        def writer(value):
            barrier.wait(timeout=3)
            return self.cas("old", value)
        with ThreadPoolExecutor(max_workers=2) as executor:
            result = list(executor.map(writer, ["one", "two"]))
        self.assertEqual(sorted(result), [0, 1])
        self.assertIn(command("GET", self.key), ["one", "two"])

    def test_other_key_is_preserved(self):
        command("SET", self.extra, "untouched")
        self.assertEqual(self.cas(None, "value"), 1)
        self.assertEqual(command("GET", self.extra), "untouched")


if __name__ == "__main__":
    unittest.main(verbosity=2)
