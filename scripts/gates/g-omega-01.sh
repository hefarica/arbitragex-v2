#!/usr/bin/env bash
# G-OMEGA-01 — omega-search workspace coverage gate (drift D-04).
#
# WHY: backend/omega-search declares its OWN `[workspace]`, so it is not a
# member of backend/Cargo.toml and therefore sits OUTSIDE the mandatory
# `cargo fmt/clippy/test --workspace` gates: a coverage hole. The crate's own
# manifest documents the standalone choice as deliberate (its promotion is a
# two-line change to a shared manifest that live fix/* branches contest), so
# this gate does not silently promote it — it makes the exclusion explicit and
# closes the hole by testing the crate on its own terms.
#
# WHAT THIS GATE PINS
#   1. The exclusion is DELIBERATE: omega-search declares `[workspace]` and is
#      NOT listed in backend/Cargo.toml members. If either side changes, the
#      gate fails and the ADR must be revisited.
#   2. The property that makes standalone safe still holds: zero external
#      dependencies (std only).
#   3. The crate is actually built and tested standalone (cargo fmt --check +
#      cargo test) when a toolchain is present. Without a toolchain the build
#      check is reported as DEBT — never as a silent pass.
#
# Exit 0 = PASS, exit 1 = FAIL.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

say "G-OMEGA-01 — omega-search workspace coverage gate"

CRATE="backend/omega-search"

# 1. deliberate exclusion, both sides
grep -q '^\[workspace\]' "$CRATE/Cargo.toml"
chk "omega-search declares its own [workspace] (standalone is explicit)" $?
grep -qE '^\s*"omega-search"' backend/Cargo.toml
[ $? -ne 0 ]
chk "omega-search is NOT a member of backend/Cargo.toml (exclusion deliberate)" $?

# 2. zero external dependencies (std only) — the standalone-safety property
python3 - <<'PY'
import re, sys
src = open("backend/omega-search/Cargo.toml").read()
m = re.search(r"^\[dependencies\](.*?)(^\[|\Z)", src, re.S | re.M)
body = m.group(1) if m else ""
deps = [l.strip() for l in body.splitlines() if l.strip() and not l.strip().startswith("#")]
if deps:
    print("  FAIL  external dependencies found:", deps)
    sys.exit(1)
print("  PASS  [dependencies] is empty (std only, no registry pull)")
PY
chk "omega-search has no external dependencies" $?

# 3. build + test standalone (or declare the debt)
if command -v cargo >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/cargo" ]; then
  CARGO="$(command -v cargo || echo "$HOME/.cargo/bin/cargo")"
  ( cd "$CRATE" && "$CARGO" fmt --check >/dev/null 2>&1 )
  chk "cargo fmt --check (omega-search standalone)" $?
  ( cd "$CRATE" && "$CARGO" test --quiet >/tmp/g-omega-01-test.log 2>&1 )
  chk "cargo test (omega-search standalone)" $?
else
  say "  DEBT  cargo toolchain absent — standalone build/test NOT executed (declared debt, P4)"
fi

say ""
if [ "$fail" = "0" ]; then say "G-OMEGA-01: PASS"; else say "G-OMEGA-01: FAIL"; fi
exit "$fail"
