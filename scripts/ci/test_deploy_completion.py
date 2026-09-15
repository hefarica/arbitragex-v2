"""Isolated EX-01 regressions. No SSH, production Docker, RPC or signing.

The shell tests execute the real workflow tail with recording shell adapters.
A deliberate stdin consumer reproduces premature EOF with exit zero.
"""
from datetime import datetime, timedelta, timezone
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import textwrap
import unittest

from verify_deploy_identity import MAX_BYTES, SERVICES, parse_payload, validate

ROOT = Path(__file__).resolve().parents[2]
SHA = "ab" * 20
RUN = "123456789"
NOW = datetime(2026, 9, 15, 1, 1, tzinfo=timezone.utc)
AT = "2026-09-15T01:00:00Z"


def sample(now=NOW, deployed_at=AT):
    return {"ok": True, "deploy": {"sha": SHA, "id": RUN, "at": deployed_at},
            "ts": now.isoformat().replace("+00:00", "Z"),
            "services": {name: {"ok": True, "status": 200} for name in SERVICES}}


def workflow_script():
    source = (ROOT / ".github/workflows/auto-deploy-vps.yml").read_text(encoding="utf-8")
    section = source.split("      - name: Deploy to VPS via SSH\n", 1)[1].split("\n      - name:", 1)[0]
    run = textwrap.dedent(section.split("        run: |\n", 1)[1])
    remote = run.split("<<'EOF'\n", 1)[1].rsplit("\nEOF", 1)[0]
    return run, remote


class IdentityTests(unittest.TestCase):
    def check(self, payload):
        return validate(payload, SHA, RUN, AT, now=NOW)

    def test_current_exact_identity(self):
        result = self.check(sample())
        self.assertEqual(result["sha"], SHA)
        self.assertFalse(result["execution_verified"])
        self.assertFalse(result["persistence_verified"])
        self.assertFalse(result["image_content_attested"])

    def test_healthy_http_payload_with_wrong_sha(self):
        body = sample(); body["deploy"]["sha"] = "cd" * 20
        with self.assertRaises(ValueError): self.check(body)

    def test_same_sha_wrong_run_is_not_accepted(self):
        body = sample(); body["deploy"]["id"] = "98765"
        with self.assertRaises(ValueError): self.check(body)

    def test_same_sha_run_wrong_deploy_timestamp(self):
        body = sample(); body["deploy"]["at"] = "2026-09-14T01:00:00Z"
        with self.assertRaises(ValueError): self.check(body)

    def test_missing_fields_never_default_to_success(self):
        for key in ("ok", "deploy", "ts", "services"):
            with self.subTest(key=key):
                body = sample(); del body[key]
                with self.assertRaises(ValueError): self.check(body)

    def test_top_level_not_an_object(self):
        for body in (None, [], 1, True, "ok"):
            with self.subTest(body=body), self.assertRaises(ValueError): self.check(body)

    def test_top_level_ok_requires_boolean_true(self):
        for bad in ("true", 1, False, None):
            body = sample(); body["ok"] = bad
            with self.subTest(bad=bad), self.assertRaises(ValueError): self.check(body)

    def test_each_required_service_must_have_evidence(self):
        for name in SERVICES:
            body = sample(); del body["services"][name]
            with self.subTest(name=name), self.assertRaises(ValueError): self.check(body)

    def test_service_health_and_code_are_strict(self):
        for bad in (None, {}, {"ok": True, "status": "200"}, {"ok": 1, "status": 200},
                    {"ok": False, "status": 200}, {"ok": True, "status": 503}, {"ok": True, "status": True}):
            body = sample(); body["services"][SERVICES[0]] = bad
            with self.subTest(bad=bad), self.assertRaises(ValueError): self.check(body)

    def test_stale_future_and_predeploy_timestamps(self):
        for stamp in (NOW - timedelta(seconds=61), NOW + timedelta(seconds=31), NOW - timedelta(days=1)):
            with self.subTest(stamp=stamp), self.assertRaises(ValueError): self.check(sample(stamp))

    def test_invalid_or_ambiguous_timestamp(self):
        for stamp in (None, "yesterday", "2026-09-15", "2026-99-15T00:00:00Z", 0):
            body = sample(); body["ts"] = stamp
            with self.subTest(stamp=stamp), self.assertRaises(ValueError): self.check(body)

    def test_bad_expected_identity_is_not_echoed_as_valid(self):
        for sha, run in ((SHA[:8], RUN), (SHA.upper(), RUN), (SHA, "0"), (SHA, "001"), (SHA, "one")):
            with self.subTest(sha=sha, run=run), self.assertRaises(ValueError):
                validate(sample(), sha, run, AT, now=NOW)

    def test_extra_fields_not_copied_into_receipt(self):
        body = sample(); body["secret_fixture"] = "DO_NOT_PRINT"
        self.assertNotIn("DO_NOT_PRINT", json.dumps(self.check(body)))

    def test_json_input_rejects_duplicates_nonfinite_and_malformed(self):
        for raw in (b'{"ok":true,"ok":false}', b'{"x":NaN}', b'{"x":Infinity}', b'not-json', b'\xff'):
            with self.subTest(raw=raw), self.assertRaises((ValueError, UnicodeError)): parse_payload(raw)

    def test_size_is_bounded(self):
        for raw in (b"", b" " * (MAX_BYTES + 1)):
            with self.assertRaises(ValueError): parse_payload(raw)

    def test_ordinary_payload_parses(self):
        self.assertEqual(parse_payload(json.dumps(sample()).encode()), sample())


class StreamingShellTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Windows system32/bash.exe is a WSL launcher, not necessarily installed Linux.
        git_bash = Path("C:/Program Files/Git/bin/bash.exe")
        cls.bash = os.environ.get("BASH_TEST_EXECUTABLE") or (str(git_bash) if git_bash.is_file() else shutil.which("bash"))
        if not cls.bash:
            raise RuntimeError("Bash required: missing tests must not be reported as passed")
        cls.outer_script, cls.remote = workflow_script()
        start = cls.remote.index("# EX-01: EOF")
        end = cls.remote.index("trap deploy_completion_guard EXIT") + len("trap deploy_completion_guard EXIT")
        cls.guard = cls.remote[start:end]
        cls.tail = cls.remote[cls.remote.index('echo "hot-path trace'):]
        start = cls.remote.index("deploy_self_heal() {")
        end = cls.remote.index("trap deploy_self_heal EXIT") + len("trap deploy_self_heal EXIT")
        cls.heal = cls.remote[start:end]

    def shell(self, body, *, payload=None, head=SHA, frontend_fail="0"):
        now = datetime.now(timezone.utc)
        at = (now - timedelta(seconds=5)).strftime("%Y-%m-%dT%H:%M:%SZ")
        value = sample(now, at) if payload is None else payload
        env = os.environ.copy()
        env.update(TARGET_SHA=SHA, DEPLOY_RUN_ID=RUN, ARBX_DEPLOYED_AT=at,
                   ENV_FILE="fixture-only", COMPOSE_FILE="fixture-only", TEST_HEAD=head,
                   TEST_BODY=json.dumps(value), FRONTEND_FAIL=frontend_fail)
        python_path = shlex.quote(sys.executable.replace("\\", "/"))
        adapters = r'''
set -euo pipefail
docker() {
  case " $* " in
    *" exec "*)
      case " $* " in *" --interactive=false "*) : ;; *) cat >/dev/null ;; esac
      printf '10004\n' ;;
    *" ps "*) printf 'arbitragex-v2-api-server-1\n' ;;
    *) echo "unexpected Docker fixture command" >&2; return 99 ;;
  esac
}
git() { printf '%s\n' "$TEST_HEAD"; }
curl() {
  case " $* " in
    *"/status"*) printf '%s' "$TEST_BODY" ;;
    *"/opportunities/exchange"*) return "$FRONTEND_FAIL" ;;
    *) echo "unexpected HTTP fixture command" >&2; return 99 ;;
  esac
}
'''
        adapters += "\npython3() { " + python_path + ' "$@"; }\n'
        return subprocess.run([self.bash, "--noprofile", "--norc", "-s"], input=adapters + body + "\n",
                              text=True, encoding="utf-8", capture_output=True, cwd=ROOT, env=env, timeout=15)

    def test_workflow_shell_syntax(self):
        for body in (self.outer_script, self.remote):
            result = subprocess.run([self.bash, "--noprofile", "--norc", "-n"], input=body,
                                    text=True, encoding="utf-8", capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_baseline_reproduces_false_green(self):
        legacy = 'docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" exec -T redis redis-cli XLEN arbx:opps:detected\necho FINAL_CHECK_REACHED\n'
        result = self.shell(legacy)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("FINAL_CHECK_REACHED", result.stdout)

    def test_premature_eof_fails_with_completion_guard(self):
        result = self.shell(self.guard + '\ndocker compose exec -T redis redis-cli XLEN fixture\necho NEVER_REACHED\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("NEVER_REACHED", result.stdout)
        self.assertIn("mandatory final verification", result.stdout)

    def test_early_exit_zero_fails(self):
        result = self.shell(self.guard + "\nexit 0\n")
        self.assertNotEqual(result.returncode, 0)

    def test_original_error_code_preserved(self):
        result = self.shell(self.guard + "\nexit 17\n")
        self.assertEqual(result.returncode, 17)

    def test_full_tail_runs_and_validates_identity(self):
        result = self.shell(self.guard + "\n" + self.tail)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("G4 PASS:", result.stdout)
        self.assertIn('"image_content_attested": false', result.stdout)
        self.assertIn("Deploy completed", result.stdout)

    def test_restore_trap_does_not_swallow_incomplete_verification(self):
        result = self.shell(self.guard + "\n" + self.heal + "\nexit 0\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mandatory final verification", result.stdout)

    def test_restore_trap_preserves_failure(self):
        result = self.shell(self.guard + "\n" + self.heal + "\nexit 23\n")
        self.assertEqual(result.returncode, 23)

    def test_wrong_checkout_fails_before_success(self):
        result = self.shell(self.guard + "\n" + self.tail, head="cd" * 20)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("G4 PASS:", result.stdout)

    def test_frontend_document_failure_does_not_pass(self):
        result = self.shell(self.guard + "\n" + self.tail, frontend_fail="22")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("G4 PASS:", result.stdout)

    def test_wrong_served_identity_fails_without_body_leak(self):
        body = sample(); body["deploy"]["sha"] = "cd" * 20; body["secret"] = "DO_NOT_PRINT"
        result = self.shell(self.guard + "\n" + self.tail, payload=body)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("G4 PASS:", result.stdout)
        self.assertNotIn("DO_NOT_PRINT", result.stdout + result.stderr)

    def test_explicit_stdin_and_completion_checks_remain_in_workflow(self):
        self.assertIn("exec -T --interactive=false redis", self.remote)
        self.assertIn("XLEN arbx:opps:detected </dev/null", self.remote)
        self.assertLess(self.tail.index("verify_deploy_identity.py"), self.tail.index("DEPLOY_POSTCHECK_COMPLETE=1"))
        self.assertIn('--max-time 15', self.tail)
        self.assertIn('deploy_completion_guard "$rc"', self.heal)


if __name__ == "__main__":
    unittest.main()
