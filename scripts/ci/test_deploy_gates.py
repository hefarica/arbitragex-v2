import unittest

from wait_deploy_gates import assess


class GateTests(unittest.TestCase):
    def run_record(self, **changes):
        record = dict(id=10, run_attempt=1, head_sha="a" * 40, event="push",
                      head_branch="main", repository={"full_name": "owner/repo"},
                      path=".github/workflows/ci.yml", status="completed", conclusion="success")
        record.update(changes)
        return record

    def check(self, runs):
        return assess(runs, "a" * 40, "owner/repo", ["ci.yml"])

    def test_exact_success(self):
        self.assertEqual(self.check([self.run_record()])[:2], ([], []))

    def test_missing_never_passes(self):
        self.assertEqual(self.check([])[0], ["ci.yml"])

    def test_other_sha_event_branch_and_repository_do_not_count(self):
        for change in [dict(head_sha="b" * 40), dict(event="pull_request"),
                       dict(event="workflow_dispatch"), dict(head_branch="feature"),
                       dict(repository={"full_name": "fork/repo"})]:
            with self.subTest(change=change):
                self.assertEqual(self.check([self.run_record(**change)])[0], ["ci.yml"])

    def test_newer_failure_supersedes_success(self):
        runs = [self.run_record(id=11, conclusion="failure"), self.run_record()]
        self.assertTrue(self.check(runs)[1])

    def test_rerun_pending_supersedes_prior_attempt(self):
        runs = [self.run_record(run_attempt=2, status="in_progress", conclusion=None), self.run_record()]
        self.assertEqual(self.check(runs)[0], ["ci.yml"])

    def test_all_non_success_conclusions_fail(self):
        for conclusion in ["failure", "cancelled", "timed_out", "skipped", "neutral", None]:
            with self.subTest(conclusion=conclusion):
                self.assertTrue(self.check([self.run_record(conclusion=conclusion)])[1])

    def test_optional_triggered_workflow_failure_also_blocks(self):
        runs = [self.run_record(), self.run_record(path=".github/workflows/rust.yml", conclusion="failure")]
        self.assertTrue(self.check(runs)[1])

    def test_deploy_does_not_wait_on_itself(self):
        runs = [self.run_record(), self.run_record(path=".github/workflows/auto-deploy-vps.yml", status="in_progress")]
        self.assertEqual(self.check(runs)[:2], ([], []))


if __name__ == "__main__":
    unittest.main()
