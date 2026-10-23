#!/usr/bin/env python3
"""Controls for scripts/ci/audit_entry_expiry.py (A1 · audit gate).

Positive control, negative controls and the empty case, per the measurement
doctrine: a check that cannot fail is not a check.
"""
import datetime
import json
import tempfile
import unittest
from pathlib import Path

import audit_entry_expiry as gate

TODAY = datetime.date(2026, 10, 21)


def write(tmp: Path, entries: list[dict], ignores: list[str]) -> tuple[Path, Path]:
    manifest = tmp / "manifest.json"
    manifest.write_text(json.dumps({"cargo": entries}))
    workflow = tmp / "security.yml"
    workflow.write_text("\n".join(f"            --ignore {i}" for i in ignores))
    return manifest, workflow


class EntryExpiryTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp())

    def entries(self) -> list[dict]:
        return [
            {"id": "RUSTSEC-2026-0381", "reason": "bundled CA roots, not reachable", "owner": "MANTIS", "expires": "2026-11-20"},
            {"id": "RUSTSEC-2026-0382", "reason": "rustls 0.21 line", "owner": "MANTIS", "expires": "2026-11-20"},
        ]

    def test_valid_entry_passes_and_backlog_is_reported(self) -> None:
        """Positive: a valid entry passes; an un-manifested ignore is backlog, not a failure."""
        manifest, workflow = write(self.tmp, self.entries(), ["RUSTSEC-2026-0381", "RUSTSEC-2026-0382", "RUSTSEC-2024-0388"])
        entries = gate.load_manifest(manifest, "cargo")
        errors, valid, backlog = gate.assess(entries, gate.workflow_ignores(workflow), TODAY)
        self.assertEqual(errors, [])
        self.assertEqual(len(valid), 2)
        self.assertEqual(backlog, ["RUSTSEC-2024-0388"])

    def test_expired_entry_fails(self) -> None:
        """Negative 1: one day past the date must fail (the global-2030 defect)."""
        entries = self.entries()
        entries[0]["expires"] = "2026-10-20"
        manifest, workflow = write(self.tmp, entries, ["RUSTSEC-2026-0381", "RUSTSEC-2026-0382"])
        errors, _, _ = gate.assess(gate.load_manifest(manifest, "cargo"), gate.workflow_ignores(workflow), TODAY)
        self.assertEqual(len(errors), 1)
        self.assertIn("RUSTSEC-2026-0381 expired 2026-10-20", errors[0])

    def test_expiry_day_itself_is_still_valid(self) -> None:
        """Boundary: the expiry date is inclusive."""
        entries = self.entries()
        entries[0]["expires"] = TODAY.isoformat()
        manifest, workflow = write(self.tmp, entries, ["RUSTSEC-2026-0381", "RUSTSEC-2026-0382"])
        errors, _, _ = gate.assess(gate.load_manifest(manifest, "cargo"), gate.workflow_ignores(workflow), TODAY)
        self.assertEqual(errors, [])

    def test_stale_manifest_entry_fails(self) -> None:
        """Negative 2: an entry whose ignore was removed must not linger as dead paper."""
        manifest, workflow = write(self.tmp, self.entries(), ["RUSTSEC-2026-0381"])
        errors, _, _ = gate.assess(gate.load_manifest(manifest, "cargo"), gate.workflow_ignores(workflow), TODAY)
        self.assertEqual(len(errors), 1)
        self.assertIn("absent from the workflow", errors[0])

    def test_malformed_manifest_fails(self) -> None:
        """Negative 3: no dates, no gate."""
        entries = [{"id": "RUSTSEC-2026-0381", "reason": "x", "owner": "MANTIS"}]
        manifest, _ = write(self.tmp, entries, ["RUSTSEC-2026-0381"])
        with self.assertRaises(ValueError):
            gate.load_manifest(manifest, "cargo")

    def test_empty_workflow_and_empty_manifest_passes_vacuously(self) -> None:
        """Empty case: nothing declared, nothing enforced — and it says so."""
        manifest, workflow = write(self.tmp, [], [])
        errors, valid, backlog = gate.assess(gate.load_manifest(manifest, "cargo"), gate.workflow_ignores(workflow), TODAY)
        self.assertEqual((errors, valid, backlog), ([], [], []))


if __name__ == "__main__":
    unittest.main()
