"""VPS-DEPLOY-GUARD regressions — guards the deploy tree and the disk watchdog.

Incident reproduced (2026-10-24 14:08Z, measured):
  1. An UNTRACKED file (backend/api-server/src/routes/internal-am.ts) lived in the
     deploy tree, entered the Docker build context, imported a symbol that did not
     exist and broke api-server `tsc` => the build aborted ~30 min in.
  2. The disk-floor watchdog `( while true; ... sleep 15 ) &` stayed ORPHANED when
     the remote script exited early; it keeps the ssh pipes open, so the runner job
     never sees EOF and HANGS (2 hung attempts) instead of failing fast.

These tests do NOT touch SSH, production Docker, RPC or signing. They extract the
real remote script from `.github/workflows/auto-deploy-vps.yml` (same technique as
`test_deploy_completion.py`) and exercise the guard/watchdog functions against
throw-away git repositories.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/auto-deploy-vps.yml"

# The 11 UNTRACKED infra entries measured on /opt/arbitragex-v2 (2026-10-24).
# They predate the guard and are NOT part of the deploy.
ALLOWLISTED = [
    ".secrets/",
    "archives/",
    "docker/compose.dev.yml.pre-f1-20261018",
    "docker/compose.prod.yml.pre-r2.1792813353",
    "monitoring/alertmanager/alertmanager.yml.pre-r2.1792813353",
    "scripts/disk_guard.sh.pre-guard02-20261018",
    "scripts/gsim1_variance_export.sql.bak",
    "scripts/pg_retention.sh.pre-f1p-20261018",
    "scripts/pg_retention.sh.pre-fk02-20261018",
    "scripts/pg_retention.sh.pre-part03-20261018",
    "scripts/price_seeder.sh.pre-pull-20260930T104945Z",
]


def remote_script() -> str:
    """The exact bash body the runner pipes into `ssh ... bash -s`."""
    source = WORKFLOW.read_text(encoding="utf-8")
    section = source.split("      - name: Deploy to VPS via SSH\n", 1)[1].split("\n      - name:", 1)[0]
    run = textwrap.dedent(section.split("        run: |\n", 1)[1])
    return run.split("<<'EOF'\n", 1)[1].rsplit("\nEOF", 1)[0]


def shell_function(name: str) -> str:
    """Extract one top-level `name() { ... }` definition from the remote script."""
    script = remote_script()
    pattern = re.compile(rf"^{re.escape(name)}\(\) \{{.*?^\}}$", re.MULTILINE | re.DOTALL)
    match = pattern.search(script)
    if not match:
        raise AssertionError(f"{name}() not found in the remote deploy script")
    return match.group(0)


def allowlist_block() -> str:
    """The guard reads its allowlist from an array assignment in the script."""
    script = remote_script()
    match = re.search(r"^DEPLOY_TREE_ALLOW_UNTRACKED=\(.*?^\)$", script, re.MULTILINE | re.DOTALL)
    if not match:
        raise AssertionError("DEPLOY_TREE_ALLOW_UNTRACKED array not found in the remote script")
    return match.group(0)


def git(*args: str, cwd: Path) -> None:
    subprocess.run(["git", "-c", "user.email=t@t", "-c", "user.name=t", *args],
                   cwd=cwd, check=True, capture_output=True)


def make_tree(tmp: Path, files: dict[str, str] | None = None) -> Path:
    """A throw-away repo that mimics the deploy tree layout."""
    repo = tmp / "tree"
    repo.mkdir()
    git("init", "-q", cwd=repo)
    for rel, content in (files or {"backend/README.md": "x\n"}).items():
        target = repo / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    git("add", "-A", cwd=repo)
    git("commit", "-q", "-m", "base", cwd=repo)
    return repo


def run_guard(repo: Path, mode: str = "enforce") -> subprocess.CompletedProcess:
    """Run the REAL guard function (extracted from the workflow) against `repo`."""
    harness = "\n".join([
        "set -uo pipefail",
        allowlist_block(),
        shell_function("deploy_tree_guard"),
        'DEPLOY_TREE_GUARD_MODE="$2"',
        'cd "$1"',
        'deploy_tree_guard pre-anchor',
    ])
    env = dict(os.environ, ARBX_DEPLOY_TREE_GUARD=mode)
    return subprocess.run(["bash", "-c", harness, "guard", str(repo), mode],
                          capture_output=True, text=True, env=env)


class DeployTreeGuardBehaviourTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def test_clean_tree_passes(self):
        result = run_guard(make_tree(self.tmp))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("árbol de deploy limpio", result.stdout)

    def test_incident_file_untracked_under_backend_aborts_fast(self):
        """The exact 2026-10-24 file: aborts with the path and a remediation hint."""
        repo = make_tree(self.tmp)
        incident = repo / "backend/api-server/src/routes/internal-am.ts"
        incident.parent.mkdir(parents=True, exist_ok=True)
        incident.write_text("import { requireBearerToken } from '@arbx/shared';\n", encoding="utf-8")

        result = run_guard(repo)

        self.assertNotEqual(result.returncode, 0, "an untracked code file MUST abort the deploy")
        self.assertIn("DEPLOY-TREE-GUARD", result.stdout)
        self.assertIn("backend/api-server/src/routes/internal-am.ts", result.stdout)
        self.assertIn("/root/arbx-quarantine", result.stdout)     # reversible procedure
        self.assertIn("NO es el SHA", result.stdout)               # why it matters
        self.assertIn("::error::", result.stdout)                  # surfaces in the runner

    def test_allowlisted_infra_entries_do_not_block(self):
        repo = make_tree(self.tmp)
        for rel in ALLOWLISTED:
            target = repo / rel
            if rel.endswith("/"):
                target.mkdir(parents=True, exist_ok=True)
                (target / "keep").write_text("x\n", encoding="utf-8")
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("backup\n", encoding="utf-8")

        result = run_guard(repo)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("árbol de deploy limpio", result.stdout)

    def test_new_untracked_backup_outside_allowlist_aborts(self):
        """A brand-new .pre-* backup in the tree is exactly the noise class that
        accumulated the 9 flagged files — it must not slip through silently."""
        repo = make_tree(self.tmp)
        backup = repo / "scripts/pg_retention.sh.pre-guard99-20990101"
        backup.parent.mkdir(parents=True, exist_ok=True)
        backup.write_text("x\n", encoding="utf-8")

        result = run_guard(repo)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scripts/pg_retention.sh.pre-guard99-20990101", result.stdout)

    def test_tracked_dirty_file_aborts(self):
        """[2/9] `git reset --hard` would DESTROY this work silently."""
        repo = make_tree(self.tmp, {"frontend/lib/hooks/usePricesStream.ts": "clean\n"})
        (repo / "frontend/lib/hooks/usePricesStream.ts").write_text("dirty work\n", encoding="utf-8")

        result = run_guard(repo)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("DIRTY", result.stdout)
        self.assertIn("frontend/lib/hooks/usePricesStream.ts", result.stdout)

    def test_quarantine_restores_a_passing_tree(self):
        """Reversible quarantine: moving the file OUT of the tree makes it pass again."""
        repo = make_tree(self.tmp)
        incident = repo / "backend/api-server/src/routes/internal-am.ts"
        incident.parent.mkdir(parents=True, exist_ok=True)
        incident.write_text("broken\n", encoding="utf-8")
        self.assertNotEqual(run_guard(repo).returncode, 0)

        quarantine = self.tmp / "arbx-quarantine"
        quarantine.mkdir()
        moved = quarantine / (incident.name + ".2026-10-24T1408Z")
        shutil.move(str(incident), str(moved))

        result = run_guard(repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(moved.exists(), "quarantine must be reversible (the file is preserved)")

    def test_warn_mode_reports_but_continues(self):
        """Emergency escape hatch: logged, never silent."""
        repo = make_tree(self.tmp)
        (repo / "backend/new.ts").write_text("x\n", encoding="utf-8")

        result = run_guard(repo, mode="warn")

        self.assertEqual(result.returncode, 0)
        self.assertIn("::warning::", result.stdout)
        self.assertIn("deploy no reproducible", result.stdout)


class DeployWorkflowWiringTests(unittest.TestCase):
    """The guard is worthless if the workflow never calls it before the build."""

    def setUp(self):
        self.script = remote_script()

    def test_guard_runs_before_the_hard_reset(self):
        guard_call = self.script.index("deploy_tree_guard pre-anchor")
        reset = self.script.index('git reset --hard "$TARGET_SHA"')
        self.assertLess(guard_call, reset,
                        "the guard must run BEFORE `git reset --hard` so dirty work is not destroyed")

    def test_guard_runs_after_the_anchor_and_before_the_build(self):
        post = self.script.index("deploy_tree_guard post-anchor")
        build = self.script.index('build --no-cache "$service"')
        self.assertLess(post, build, "the guard must run before the docker build")

    def test_watchdog_is_killed_on_the_exit_path(self):
        guard_body = shell_function("deploy_completion_guard")
        self.assertIn("stop_disk_floor_watchdog", guard_body,
                      "the completion/EXIT guard must reap the disk-floor watchdog")
        for trap in ("trap deploy_completion_guard EXIT", "trap deploy_self_heal EXIT"):
            self.assertIn(trap, self.script)
        self.assertIn("deploy_completion_guard", shell_function("deploy_self_heal"),
                      "deploy_self_heal must end in the completion guard")

    def test_watchdog_stop_kills_a_live_child(self):
        """Behavioural: the extracted cleanup function really kills the PID."""
        harness = "\n".join([
            "set -uo pipefail",
            'DISK_FLOOR_PID=""',
            shell_function("stop_disk_floor_watchdog"),
            "( while true; do sleep 1; done ) &",
            "DISK_FLOOR_PID=$!",
            'stop_disk_floor_watchdog',
            'if kill -0 "$DISK_FLOOR_PID" 2>/dev/null; then echo ORPHAN_ALIVE; exit 9; fi',
            "echo WATCHDOG_REAPED",
        ])
        # NOTE: the function clears DISK_FLOOR_PID, so keep our own copy to probe.
        harness = harness.replace(
            'DISK_FLOOR_PID=$!',
            'DISK_FLOOR_PID=$!; PROBE_PID=$!',
        ).replace(
            'if kill -0 "$DISK_FLOOR_PID" 2>/dev/null; then echo ORPHAN_ALIVE; exit 9; fi',
            'if kill -0 "$PROBE_PID" 2>/dev/null; then echo ORPHAN_ALIVE; exit 9; fi',
        )
        result = subprocess.run(["bash", "-c", harness], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("WATCHDOG_REAPED", result.stdout)

    def test_allowlist_matches_the_measured_infra_entries(self):
        block = allowlist_block()
        documented = {line.strip().strip('"') for line in block.splitlines()[1:-1] if line.strip()}
        self.assertEqual(documented, set(ALLOWLISTED),
                         "the allowlist must stay the 11 MEASURED entries — no silent additions")


if __name__ == "__main__":
    unittest.main()
