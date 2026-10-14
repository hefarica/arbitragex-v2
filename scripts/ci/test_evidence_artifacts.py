import copy
import unittest

from evidence_artifacts import publication_verdict, select_run, validate_payload


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

    # DEPLOY-VERAZ-01: this step runs AFTER the deployment, so a superseded main
    # must skip the publication instead of reddening a delivery that already
    # happened — while the step one hop from POSTing stays a hard refusal.
    def test_superseded_main_skips_the_publication_instead_of_failing_the_delivery(self):
        other = "b" * 40
        self.assertEqual(publication_verdict(self.sha, self.sha, False), "publish")
        self.assertEqual(publication_verdict(other, self.sha, False), "skip-superseded")
        self.assertEqual(publication_verdict(self.sha, self.sha, True), "publish")
        self.assertEqual(publication_verdict(other, self.sha, True), "refuse")

    def test_only_a_superseded_sha_may_skip_and_only_while_selecting(self):
        # The skip is narrow: any other mismatch still refuses, in both phases.
        for main_sha in ("", "c" * 40, "a" * 39):
            for validating in (False, True):
                with self.subTest(main_sha=main_sha, validating=validating):
                    self.assertNotEqual(publication_verdict(main_sha, self.sha, validating), "publish")
        self.assertEqual(publication_verdict("c" * 40, self.sha, True), "refuse")


if __name__ == "__main__":
    unittest.main()
