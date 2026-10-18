#!/usr/bin/env bash
# G-FIXTURE-METADATA-01 — sim-core bytecode-fixture vs contracts/out drift gate.
#
# WHAT IT ENFORCES
#   The committed fixtures in `backend/sim-core/src/fixtures/*.hex` (the REAL
#   forge creation code that sim-core deploys inside the in-process fork) must
#   match a fresh `forge build` of `contracts/`, **excluding only the trailing
#   solc CBOR metadata section** — a build artifact (compiler version +
#   ipfs/keccak of the metadata), not executable semantics.
#
#   The exclusion is EXPLICIT (R8 — nothing silent): `split_solc_metadata`
#   strips a well-formed trailing section only, and the test asserts every byte
#   BEFORE it is identical, naming the ignored ranges in the failure message.
#   If the trailing length does not fit, nothing is stripped and the comparison
#   is byte-for-byte over the whole blob. A whole-blob "metadata" split
#   (`declared + 2 == code.len()`) is rejected too (R-1): executable is never
#   empty, so two degenerate splits cannot pass trivially.
#
# RUNNING IT IN CI
#   R-2 (2026-10-18): a job in `.github/workflows/ci.yml` (`fixture-metadata-gate`)
#   installs the Foundry toolchain + Rust and runs THIS script, so the gate is
#   actually exercised on every PR/push. Previously the CI image shipped no
#   `forge`, `contracts/out` was absent and the test self-skipped — the gate is
#   no longer only local. A MANUAL-ONLY fallback with an expiry date is recorded
#   in `docs/reaintegration/gates/G-FIXTURE-METADATA-01.md` in case the job is
#   ever removed.
#
# NOTE (fix 2026-10-18): the cargo invocation used to run from the repo root,
# which has NO `Cargo.toml` — `cargo test -p sim-core` failed with
# "could not find Cargo.toml" before it ever reached the test. It now points at
# `backend/Cargo.toml` via `--manifest-path`, and `forge build` output is no
# longer swallowed.
#
# Exit: 0 = PASS, 1 = FAIL, 2 = SKIP (no cargo/forge available).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.cargo/bin:$HOME/.foundry/bin:$PATH"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0

echo "G-FIXTURE-METADATA-01 — sim-core fixture vs contracts/out (metadata excluded)"

if ! command -v forge >/dev/null 2>&1 || ! command -v cargo >/dev/null 2>&1; then
  echo "  SKIP  forge/cargo unavailable — declared debt, not silently green"
  exit 2
fi

build_log="$(mktemp)"
echo "  ... forge build (contracts/)"
if ! (cd contracts && forge build) >"$build_log" 2>&1; then
  echo "  FAIL  forge build failed — cannot produce contracts/out (full log follows)"
  cat "$build_log"
  rm -f "$build_log"
  exit 1
fi
rm -f "$build_log"
for n in ArbitrageExecutor FlashLoanExecutor; do
  if [ ! -f "contracts/out/$n.sol/$n.json" ]; then
    echo "  FAIL  contracts/out/$n.sol/$n.json missing after build"
    exit 1
  fi
done
echo "  info  contracts/out present for both executors"

# `set -e` must not abort before we can classify the cargo result, so the
# failure is captured explicitly (still no silent `|| true`).
set +e
out="$(cargo test --manifest-path backend/Cargo.toml -p sim-core --lib fixtures_match_forge_output_when_present 2>&1)"
rc=$?
set -e
printf '%s\n' "$out" | grep -E "^test result:|DECLARED EXCLUSION|EXECUTABLE code drifted|EMPTY executable|fixtures_match_forge_output_when_present \.\.\. FAILED" | head -8
if [ "$rc" = "0" ]; then
  echo "G-FIXTURE-METADATA-01: PASS"
else
  echo "G-FIXTURE-METADATA-01: FAIL (see 'EXECUTABLE code drifted' above — real byte drift, metadata policy does not hide it)"
fi
exit "$rc"
