"""Regression and fail-closed checks for the offline TLS guard, with fixture lockfiles."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest

from tls_dependency_guard import findings, main


def lock(*versions):
    return {"version": 4, "package": [{"name": "rustls", "version": v} for v in versions]}


class TlsDependencyGuardTests(unittest.TestCase):
    def test_locked_vulnerable_release_is_rejected(self):
        self.assertEqual(findings(lock("0.23.43"))[0]["advisory"], "RUSTSEC-2026-0285")

    def test_every_affected_patch_is_rejected(self):
        for patch in range(13, 45):
            with self.subTest(patch=patch):
                self.assertEqual(len(findings(lock(f"0.23.{patch}"))), 1)

    def test_patched_release_is_accepted(self):
        self.assertEqual(findings(lock("0.23.45")), [])

    def test_later_patch_is_accepted(self):
        self.assertEqual(findings(lock("0.23.46")), [])

    def test_prerelease_is_not_a_patched_release(self):
        self.assertEqual(len(findings(lock("0.23.45-rc.1"))), 1)

    def test_build_metadata_does_not_hide_an_affected_patch(self):
        self.assertEqual(len(findings(lock("0.23.43+build.1"))), 1)
        self.assertEqual(findings(lock("0.23.45+build.1")), [])

    def test_unaffected_release_is_not_misclassified(self):
        self.assertEqual(findings(lock("0.21.12", "0.23.12", "0.24.0")), [])

    def test_multiple_versions_cannot_hide_vulnerable_copy(self):
        self.assertEqual(len(findings(lock("0.21.12", "0.23.43", "0.23.45"))), 1)

    def test_real_removal_of_dependency_is_valid(self):
        self.assertEqual(findings({"package": [{"name": "other", "version": "1.0.0"}]}), [])

    def test_invalid_versions_fail_closed(self):
        for value in [None, 45, "v0.23.45", "0.23.43;exit 0", "0.23.43\n", "00.23.45"]:
            with self.subTest(version=value), self.assertRaises(ValueError):
                findings(lock(value))

    def test_invalid_or_missing_package_list_fails_closed(self):
        for data in [{}, {"package": []}, {"package": {}}, {"package": [None]}]:
            with self.subTest(data=data), self.assertRaises(ValueError):
                findings(data)

    def run_cli(self, path):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = main(["--lockfile", str(path)])
        return code, json.loads(output.getvalue())

    def test_cli_exit_codes_match_real_findings(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "Cargo.lock"
            for version, expected in [("0.23.43", 1), ("0.23.45", 0)]:
                path.write_text(f'[[package]]\nname="rustls"\nversion="{version}"\n', encoding="utf8")
                code, result = self.run_cli(path)
                self.assertEqual(code, expected)
                self.assertEqual(result["status"], "failed" if expected else "passed")

    def test_cli_missing_file_fails(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertEqual(self.run_cli(Path(folder) / "absent.lock")[0], 1)

    def test_cli_malformed_lockfile_does_not_expose_content(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "Cargo.lock"
            path.write_text('private_fixture_value = "unterminated', encoding="utf8")
            code, result = self.run_cli(path)
            self.assertEqual(code, 1)
            self.assertNotIn("private_fixture", json.dumps(result))

if __name__ == "__main__":
    unittest.main()
