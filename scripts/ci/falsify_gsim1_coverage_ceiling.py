#!/usr/bin/env python3
"""FALSIFIER for GSIM-COVERAGE-CEILING-01 — `mutate the fix, the tests must go red`.

Pattern proven in this repo: green (blob A) -> mutate (blob B) -> red with
exit != 0 -> restore (blob A) -> green, reporting all runs with their exit codes,
their blobs and their raw output.

Two independent mutations, because the task names two independent properties:

  M1  neutralize the NAMED reason  -> the discard is an aggregate again
  M2  neutralize the DEGRADATION   -> the coverage verdict can pass as a
                                      result of the market

This script MUTATES `scripts/ci/gsim1_coverage_ceiling.py` in place and restores
it, so it is DELIBERATELY NOT wired into CI (the hermetic guard that CI runs is
`scripts/ci/test_gsim1_coverage_ceiling.py`). A falsifier that mutates the tree
inside a shared pipeline is a new defect; run it by hand, on a worktree, or here.

Usage:  python3 scripts/ci/falsify_gsim1_coverage_ceiling.py
Exit:   0 the falsifier passed (both mutations turned the suite red, and the
          restored blob is byte-identical to the pristine one)
        != 0 the falsifier itself failed — a mutation left the suite GREEN
          (the test does not guard the property), the pristine blob was not
          green, or the restore did not reproduce blob A
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

CI_DIR = Path(__file__).resolve().parent
REPO = CI_DIR.parents[1]
MODULE = CI_DIR / "gsim1_coverage_ceiling.py"
TESTGLOB = "test_gsim1_coverage_ceiling.py"
BACKUP = CI_DIR / "gsim1_coverage_ceiling.py.pristine-blobA"


def blob(path: Path) -> str:
    """`git hash-object` of the file's bytes (never a PowerShell re-encode: it adds CRLF)."""
    out = subprocess.run(
        ["git", "-C", str(REPO), "hash-object", "--", str(path)],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip()


def run_tests() -> tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/ci", "-p", TESTGLOB, "-v"],
        cwd=str(REPO),
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout + proc.stderr


def tail(text: str, n: int = 22) -> str:
    lines = [ln for ln in text.splitlines() if ln.strip()]
    return "\n".join(lines[-n:])


def failing_tests(out: str) -> list[str]:
    return [ln.strip() for ln in out.splitlines() if ln.startswith(("FAIL:", "ERROR:"))]


def main() -> int:
    shutil.copyfile(MODULE, BACKUP)
    blob_a = blob(MODULE)
    print("=" * 78)
    print(f"BLOB A (pristine) = {blob_a}")
    print("=" * 78)

    rc, out = run_tests()
    print(f"[RUN 1 | blob A] exit={rc}  (expect 0)")
    print(tail(out, 6))
    if rc != 0:
        shutil.copyfile(BACKUP, MODULE)
        os.remove(BACKUP)
        print("FALSIFIER ABORTED: the pristine blob is not green, so nothing below can prove anything.")
        return 2
    green_a = rc

    results: list[tuple[str, str, int, list[str]]] = []

    # ------------------- M1: the discard loses its name --------------------
    src = MODULE.read_text(encoding="utf-8")
    anchor_m1 = 'return f"{CODE_UNSUPPORTED_ADAPTER}:" + "+".join(unknown)'
    assert src.count(anchor_m1) == 1, f"M1 anchor appears {src.count(anchor_m1)} times"
    MODULE.write_text(
        src.replace(anchor_m1, "return CODE_UNSUPPORTED_ADAPTER  # M1: aggregate without a reason"),
        encoding="utf-8",
        newline="\n",
    )
    blob_b1 = blob(MODULE)
    rc, out = run_tests()
    print("=" * 78)
    print(f"[RUN 2 | blob B1 = {blob_b1}] M1: `unsupported_adapter` loses its named reason")
    print(f"  exit={rc}  (expect != 0)")
    print(tail(out))
    fails = failing_tests(out)
    print("  failing tests:", fails)
    results.append(("M1 named reason neutralized", blob_b1, rc, fails))
    if rc == 0:
        shutil.copyfile(BACKUP, MODULE)
        os.remove(BACKUP)
        print("  !!! FALSIFIER FAILED: the suite stayed GREEN with the reason removed.")
        return 3

    # ------------------- M2: the verdict stops degrading --------------------
    src = BACKUP.read_text(encoding="utf-8")
    anchor_m2 = '        verdict = "NO_COMPUTADO"\n        reason = (\n            f"encoder_scope:'
    assert src.count(anchor_m2) == 1, f"M2 anchor appears {src.count(anchor_m2)} times"
    MODULE.write_text(
        src.replace(
            anchor_m2,
            '        verdict = "COMPUTADO"  # M2: publish it as a market result\n'
            '        reason = (\n            f"encoder_scope:',
        ),
        encoding="utf-8",
        newline="\n",
    )
    blob_b2 = blob(MODULE)
    rc, out = run_tests()
    print("=" * 78)
    print(f"[RUN 3 | blob B2 = {blob_b2}] M2: the coverage verdict no longer degrades")
    print(f"  exit={rc}  (expect != 0)")
    print(tail(out))
    fails = failing_tests(out)
    print("  failing tests:", fails)
    results.append(("M2 degradation neutralized", blob_b2, rc, fails))
    if rc == 0:
        shutil.copyfile(BACKUP, MODULE)
        os.remove(BACKUP)
        print("  !!! FALSIFIER FAILED: the suite stayed GREEN with the degradation removed.")
        return 4

    # ----------------------- restore blob A ---------------------------------
    shutil.copyfile(BACKUP, MODULE)
    blob_a2 = blob(MODULE)
    rc, out = run_tests()
    print("=" * 78)
    print(f"[RUN 4 | blob A restored = {blob_a2}] exit={rc}  (expect 0)")
    print(tail(out, 6))
    restored = blob_a2 == blob_a
    print(f"  byte-identical to the pristine blob A: {restored}")
    print("=" * 78)
    print("FALSIFIER SUMMARY")
    print(f"  run 1  blob A  = {blob_a}  exit={green_a}  (green)")
    for name, b, r, f in results:
        print(f"  mutate {name}")
        print(f"         blob B  = {b}  exit={r}  (RED)  failing: {f}")
    print(f"  run 4  blob A  = {blob_a2}  exit={rc}  (green)  restored={restored}")
    os.remove(BACKUP)
    ok = green_a == 0 and all(r != 0 for _, _, r, _ in results) and rc == 0 and restored
    print(f"FALSIFIER {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 5


if __name__ == "__main__":
    raise SystemExit(main())
