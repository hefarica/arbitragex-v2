#!/usr/bin/env bash
# gate-active-outcome-emitter.sh — BUG-003/RD-06/ARBX-0024 regression gate (2026-08-31).
#
# The route-discovery outcomes dataset (arbx:route_discovery:outcomes) feeds the
# ≥2-week hit-rate calibration labels (S4). Telemetry is hot-path MODE-INVARIANT
# (§34.1): it must accrue in Active exactly as in Shadow — only the execution
# terminus differs by mode. The defect: `active_evaluate_and_emit` consumed each
# resolved CartridgeEvalResult and never called `emit_shadow_outcome`, so
# XLEN stayed 0 while cartridges ran Active.
#
# This gate asserts the wiring structurally: inside the body of
# `active_evaluate_and_emit` there MUST be a call to `emit_shadow_outcome(`
# that passes `&eval_result` (the per-cartridge resolved outcome). Removing or
# renaming the call-site re-breaks the dataset and fails CI.
set -euo pipefail

SRC="backend/searcher-rs/src/cartridge_boot.rs"

# Extract the body of active_evaluate_and_emit (from its pub fn line to the
# first line-anchored closing brace at column 0 after it).
BODY="$(awk '/^pub async fn active_evaluate_and_emit/{flag=1} flag{print} flag && /^}/{exit}' "$SRC")"

if [ -z "$BODY" ]; then
  echo "::error::active_evaluate_and_emit not found in $SRC — did it move? Update this gate."
  exit 1
fi

if ! grep -q 'emit_shadow_outcome(' <<<"$BODY"; then
  echo "::error::active_evaluate_and_emit does NOT call emit_shadow_outcome —"
  echo "::error::route-discovery outcome telemetry is mode-invariant (§34.1); removing this"
  echo "::error::call-site starves arbx:route_discovery:outcomes while cartridges run Active."
  echo "::error::RCA: BUG-003/RD-06/ARBX-0024 (workbook Holy_Grail_Audit_20260830)."
  exit 1
fi

if ! grep -q '&eval_result' <<<"$(grep -A6 'emit_shadow_outcome(' <<<"$BODY" | head -40)"; then
  echo "::error::emit_shadow_outcome is called in active_evaluate_and_emit without the resolved &eval_result."
  exit 1
fi

echo "gate-active-outcome-emitter: OK (Active path persists route-discovery outcomes — mode-invariant telemetry)"
