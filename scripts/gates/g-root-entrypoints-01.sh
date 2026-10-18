#!/usr/bin/env bash
# G-ROOT-ENTRYPOINTS-01 — root entrypoint ambiguity gate (drift D-07).
#
# WHY: the repository root carried five README variants with no stated
# relationship, four doctrine .md files tracked at 0 bytes, and stray
# .bak/.backup files. A reader (or a tool) cannot tell which file is the entry
# point, and an empty tracked .md looks like a document that was never written.
#
# WHAT THIS GATE PINS
#   1. Every tracked 0-byte root .md is in the DECLARED allowlist (known
#      placeholder doctrine). A new empty root document fails the gate.
#   2. The README series (README-1..4.md) is referenced from README.md, so the
#      entry point is unambiguous without renaming files that cross-reference
#      each other by name.
#   3. Every tracked root *.bak / *.backup is in the DECLARED allowlist
#      (excluded from counts by G-BACKUP-01; removal is an owner decision).
#
# Exit 0 = PASS, exit 1 = FAIL.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

say "G-ROOT-ENTRYPOINTS-01 — root entrypoint ambiguity gate"

# --- 1 + 3. allowlists ------------------------------------------------------
python3 - <<'PY'
import subprocess, sys

EMPTY_MD_ALLOW = {
    "00_MANIFIESTO_DE_EFICIENCIA_ESTRUCTURAL.md",
    "01_RESOLUCION_ESTADO_ASIMETRICO.md",
    "02_INYECCION_ENTROPIA_TRANSITORIA.md",
    "03_EQUILIBRIO_MULTIVARIABLE_CIEGO.md",
}
STRAY_ALLOW = {"package.json.bak", "frontend/app/page.tsx.backup"}

tracked = subprocess.run(["git", "ls-files"], capture_output=True, text=True).stdout.splitlines()
root_files = [p for p in tracked if "/" not in p]

empty = sorted(
    p for p in root_files
    if p.endswith(".md") and __import__("os").path.getsize(p) == 0
)
stray = sorted(p for p in tracked if p.endswith(".bak") or p.endswith(".backup"))

rc = 0
print("  info  tracked 0-byte root .md:", empty or "none")
new_empty = [p for p in empty if p not in EMPTY_MD_ALLOW]
if new_empty:
    print("  FAIL  new 0-byte root .md not in the declared allowlist:", new_empty)
    rc = 1
else:
    print("  PASS  every 0-byte root .md is a declared placeholder (allowlist)")

print("  info  tracked .bak/.backup:", stray or "none")
new_stray = [p for p in stray if p not in STRAY_ALLOW]
if new_stray:
    print("  FAIL  stray backup file not in the declared allowlist:", new_stray)
    rc = 1
else:
    print("  PASS  every stray backup file is declared (G-BACKUP-01 excludes it from counts)")
sys.exit(rc)
PY
chk "0-byte root .md and stray backups stay within their declared allowlists" $?

# --- 2. README series reachable from the entry point ------------------------
missing=0
for n in 1 2 3 4; do
  grep -q "README-$n\.md" README.md || missing=1
done
chk "README-1..4.md are referenced from README.md (entry point unambiguous)" $((missing == 0 ? 0 : 1))

say ""
if [ "$fail" = "0" ]; then say "G-ROOT-ENTRYPOINTS-01: PASS"; else say "G-ROOT-ENTRYPOINTS-01: FAIL"; fi
exit "$fail"
