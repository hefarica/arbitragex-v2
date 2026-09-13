"""Synthetic LOCAL fixtures only. These are NOT VPS/financial evidence."""
from __future__ import annotations
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SKILL = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("validate_map", SKILL / "scripts/validate_map.py")
assert SPEC and SPEC.loader
VM = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VM)


def timestamp(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


class MapValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.now = datetime.now(timezone.utc)
        evidence = b"SYNTHETIC UNIT TEST FILE - NOT A VPS DIAGNOSTIC\n"
        (self.root / "evidence").mkdir()
        (self.root / "evidence/state.txt").write_bytes(evidence)
        self.doc = {
            "schema_version": 1, "snapshot_id": "synthetic-unit-test",
            "captured_at_utc": timestamp(self.now),
            "target": {"ssh_alias": "test-only", "identity": "unit-fixture-host",
                       "project_path": "/fixture", "identity_evidence_id": "ev1"},
            "domains": [], "assets": [], "evidence": [
                {"id": "ev1", "path": "evidence/state.txt", "sha256": hashlib.sha256(evidence).hexdigest(),
                 "captured_at_utc": timestamp(self.now), "target_identity": "unit-fixture-host"}],
            "open_gaps": [], "mapper_attestation": {
                "inventory_sources_reconciled": True, "secrets_excluded": True,
                "no_intentional_remote_mutations": True},
        }
        for did, checks in VM.REQUIRED_CHECKS.items():
            aid = "asset-" + did
            self.doc["domains"].append({
                "id": did, "enumeration_complete": True, "enumeration_evidence_ids": ["ev1"],
                "asset_ids": [aid], "absence_reason": None,
                "checks": [{"id": c, "status": "verified", "finding": "synthetic assertion",
                            "evidence_ids": ["ev1"]} for c in checks]})
            self.doc["assets"].append({"id": aid, "domain_id": did, "label": "synthetic asset",
                "verified": True, "depends_on": [], "evidence_ids": ["ev1"]})

    def errors(self, doc=None):
        return VM.validate(self.doc if doc is None else doc, self.root, now=self.now)

    def test_complete_synthetic_manifest_passes_metadata_checks(self):
        self.assertEqual(self.errors(), [])

    def test_delivered_blank_template_fails(self):
        doc = json.loads((SKILL / "templates/MAPA_VPS.template.json").read_text())
        self.assertTrue(self.errors(doc))

    def test_every_required_domain_is_required(self):
        for i in range(len(self.doc["domains"])):
            with self.subTest(domain=i):
                doc = copy.deepcopy(self.doc)
                del doc["domains"][i]
                self.assertTrue(self.errors(doc))

    def test_each_mandatory_check_is_required(self):
        for i, domain in enumerate(self.doc["domains"]):
            for j in range(len(domain["checks"])):
                with self.subTest(domain=i, check=j):
                    doc = copy.deepcopy(self.doc)
                    del doc["domains"][i]["checks"][j]
                    self.assertTrue(self.errors(doc))

    def test_blocked_check_cannot_be_complete(self):
        self.doc["domains"][4]["checks"][0]["status"] = "blocked"
        self.assertTrue(self.errors())

    def test_unfinished_pagination_cannot_be_complete(self):
        self.doc["domains"][0]["enumeration_complete"] = False
        self.assertTrue(self.errors())

    def test_open_gap_prevents_completion(self):
        self.doc["open_gaps"] = ["database catalog not available"]
        self.assertTrue(self.errors())

    def test_missing_evidence_file_fails(self):
        (self.root / "evidence/state.txt").unlink()
        self.assertTrue(self.errors())

    def test_altered_evidence_hash_fails(self):
        (self.root / "evidence/state.txt").write_text("changed")
        self.assertTrue(self.errors())

    def test_cross_target_evidence_fails(self):
        self.doc["evidence"][0]["target_identity"] = "another-host"
        self.assertTrue(self.errors())

    def test_stale_snapshot_and_stale_evidence_fail_independently(self):
        for target in (self.doc, self.doc["evidence"][0]):
            previous = target["captured_at_utc"]
            target["captured_at_utc"] = timestamp(self.now - timedelta(hours=2))
            self.assertTrue(self.errors())
            target["captured_at_utc"] = previous

    def test_future_and_zone_free_timestamps_fail(self):
        for value in (timestamp(self.now + timedelta(seconds=1)), "2026-09-12T10:00:00", "bad"):
            self.doc["captured_at_utc"] = value
            self.assertTrue(self.errors())

    def test_traversal_absolute_and_windows_paths_fail(self):
        for value in ("../state.txt", "/etc/passwd", r"C:\state.txt", "file:///tmp/state", ""):
            self.doc["evidence"][0]["path"] = value
            self.assertTrue(self.errors())

    def test_symlink_evidence_is_rejected(self):
        link = self.root / "evidence/link.txt"
        try:
            link.symlink_to(self.root / "evidence/state.txt")
        except OSError:
            self.skipTest("OS does not allow creating symlink for this local test")
        self.doc["evidence"][0]["path"] = "evidence/link.txt"
        self.assertTrue(self.errors())

    def test_duplicate_ids_are_rejected(self):
        for field in ("domains", "assets", "evidence"):
            with self.subTest(field=field):
                doc = copy.deepcopy(self.doc)
                doc[field].append(copy.deepcopy(doc[field][0]))
                self.assertTrue(self.errors(doc))

    def test_missing_reference_is_rejected(self):
        self.doc["domains"][0]["checks"][0]["evidence_ids"] = ["missing"]
        self.assertTrue(self.errors())

    def test_unreconciled_assets_are_rejected(self):
        self.doc["domains"][0]["asset_ids"] = []
        self.assertTrue(self.errors())

    def test_unverified_asset_is_rejected(self):
        self.doc["assets"][0]["verified"] = False
        self.assertTrue(self.errors())

    def test_orphan_and_self_dependencies_are_rejected(self):
        for value in (["missing"], [self.doc["assets"][0]["id"]], [None]):
            self.doc["assets"][0]["depends_on"] = value
            self.assertTrue(self.errors())

    def test_absence_needs_evidence_and_reason(self):
        asset = self.doc["assets"].pop()
        domain = self.doc["domains"][-1]
        domain["asset_ids"] = []
        self.assertTrue(self.errors())
        domain["absence_reason"] = "Synthetic evidence establishes no assets in this domain"
        self.assertEqual(self.errors(), [])
        domain["enumeration_evidence_ids"] = []
        self.assertTrue(self.errors())

    def test_missing_attestation_fails(self):
        self.doc["mapper_attestation"]["no_intentional_remote_mutations"] = False
        self.assertTrue(self.errors())

    def test_malformed_types_fail_without_uncaught_exception(self):
        for value in (None, [], "bad", 1):
            self.assertTrue(VM.validate(value, self.root, now=self.now))
        for field in ("domains", "assets", "evidence", "target", "open_gaps", "mapper_attestation"):
            with self.subTest(field=field):
                doc = copy.deepcopy(self.doc)
                doc[field] = None
                self.assertTrue(self.errors(doc))
        self.doc["assets"][0]["domain_id"] = []
        self.assertTrue(self.errors())

    def test_boolean_is_not_a_schema_version(self):
        self.doc["schema_version"] = True
        self.assertTrue(self.errors())

    def test_cli_pass_has_no_authorization_claim(self):
        path = self.root / "map.json"
        path.write_text(json.dumps(self.doc))
        run = subprocess.run([sys.executable, str(SKILL / "scripts/validate_map.py"), str(path)],
                             capture_output=True, text=True, timeout=10)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn("MAPA_FORMAL_COMPLETO", run.stdout)
        self.assertIn("NO autoriza escrituras ni trading", run.stdout)

    def test_invalid_json_cli_returns_input_error(self):
        path = self.root / "bad.json"
        path.write_text("{")
        run = subprocess.run([sys.executable, str(SKILL / "scripts/validate_map.py"), str(path)],
                             capture_output=True, text=True, timeout=10)
        self.assertEqual(run.returncode, 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
