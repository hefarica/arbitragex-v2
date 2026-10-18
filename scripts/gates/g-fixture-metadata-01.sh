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
#   is byte-for-byte over the whole blob.
#
# CI NOTE (declared, not hidden)
#   The CI image ships no `forge`, so `contracts/out` is absent and the guard
#   self-skips there. CI does NOT exercise this gate today. This script is the
#   exercising path: it builds the contracts and runs the test.
#
# Exit: 0 = PASS, 1 = FAIL, 2 = SKIP (no cargo/forge available).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.cargo/bin:$HOME/.foundry/bin:$PATH"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0

echo "G-FIXTURE-METADATA-01 — sim-core fixture vs contracts/out (metadata excluded)"

if ! command -v forge >/dev/null 2>&1 || ! command -v cargo >/dev/null 2>&1; then
  echo "  SKIP  forge/cargo unavailable — declared debt, not silently green"
  exit 2
fi

echo "  ... forge build (contracts/)"
if ! (cd contracts && forge build >/tmp/g-fixture-metadata-01-forge.log 2>&1); then
  echo "  FAIL  forge build failed — cannot produce contracts/out"
  echo "  info  tail of forge output:"
  tail -5 /tmp/g-fixture-metadata-01-forge.log | sed 's/^/        /'
  exit 1
fi
for n in ArbitrageExecutor FlashLoanExecutor; do
  if [ ! -f "contracts/out/$n.sol/$n.json" ]; then
    echo "  FAIL  contracts/out/$n.sol/$n.json missing after build"
    exit 1
  fi
done
echo "  info  contracts/out present for both executors"

# The Rust workspace manifest lives in backend/ — there is NO root Cargo.toml,
# so cargo must be invoked from there (a root invocation fails with
# "could not find Cargo.toml" and would be misread as a fixture failure).
out=$(cd backend && cargo test -p sim-core --lib fixtures_match_forge_output_when_present 2>&1)
rc=$?
printf '%s\n' "$out" | grep -E "^test result:|DECLARED EXCLUSION|EXECUTABLE code drifted|fixtures_match_forge_output_when_present \.\.\. FAILED" | head -8
if [ "$rc" = "0" ]; then
  echo "G-FIXTURE-METADATA-01: PASS"
elif printf '%s' "$out" | grep -q "could not find .Cargo.toml."; then
  echo "G-FIXTURE-METADATA-01: ERROR (cargo could not find the workspace manifest — tooling problem, NOT a fixture verdict)"
  exit 3
else
  echo "G-FIXTURE-METADATA-01: FAIL (see 'EXECUTABLE code drifted' above — real byte drift, metadata policy does not hide it)"
fi
exit "$rc"
