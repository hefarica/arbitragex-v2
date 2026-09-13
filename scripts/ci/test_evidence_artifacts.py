import copy
import unittest

from evidence_artifacts import select_run, validate_payload


class EvidenceTests(unittest.TestCase):
    sha = "a" * 40
    repo = "owner/repo"

    def run_record(self, **changes):
        row = dict(id=10, run_attempt=1, head_sha=self.sha, event="push",
                   head_branch="main", repository={"full_name": self.repo},
                   path=".github/workflows/sim-evidence-unit-tests.yml",
                   status="completed", conclusion="success")
        row.update(changes)
        return row

    def test_only_exact_main_push_can_supply_evidence(self):
        self.assertEqual(select_run([self.run_record()], self.sha, self.repo), 10)
        for changes in (dict(head_sha="b" * 40), dict(event="pull_request"),
                        dict(head_branch="feature"), dict(repository={"full_name": "fork/repo"})):
            self.assertIsNone(select_run([self.run_record(**changes)], self.sha, self.repo))

    def test_newer_failure_or_pending_attempt_invalidates_old_success(self):
        for changes in (dict(id=11, conclusion="failure"),
                        dict(run_attempt=2, status="in_progress", conclusion=None)):
            with self.assertRaises(ValueError):
                select_run([self.run_record(), self.run_record(**changes)], self.sha, self.repo)

    def test_no_path_filtered_run_has_no_evidence_to_publish(self):
        self.assertIsNone(select_run([], self.sha, self.repo))

    def test_payload_binds_sha_item_run_and_actual_success(self):
        payload = dict(gate_id="G-SIM-1", item_key="unit_tests", status="evidenced",
                       evidence_ref=f"https://github.com/{self.repo}/actions/runs/10",
                       verified_by="ci:sim-evidence-unit-tests.yml", detail={"source_sha": self.sha})
        validate_payload(payload, "unit_tests", self.sha, self.repo, 10)
        for key, value in (("gate_id", "another"), ("item_key", "dep_tree"),
                           ("status", "failed"), ("evidence_ref", "another-run"),
                           ("verified_by", "other"), ("detail", {"source_sha": "b" * 40}),
                           ("detail", None)):
            changed = copy.deepcopy(payload)
            changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                validate_payload(changed, "unit_tests", self.sha, self.repo, 10)


if __name__ == "__main__":
    unittest.main()
