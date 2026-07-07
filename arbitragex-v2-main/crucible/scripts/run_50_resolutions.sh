#!/usr/bin/env bash
# =============================================================================
# OMEGA Crucible — 50 Holonomic Resolutions across 3 testnets
# =============================================================================
# Drives 50 end-to-end Holonomic Resolutions across Holesky, Arb Sepolia
# and Polygon Amoy.  Records every TxHash into PostgreSQL and emits
# runtime_ack to the api-server for the WAITING_RUNTIME_ACK → VERIFIED
# UI transition.
#
# Pre-conditions:
#   • DeterministicFactory + ResolutionCore + Adapters deployed (run deploy_crucible.sh)
#   • api-server reachable at $RUNTIME_ACK_URL
#   • sed-core operating in shadow + crucible mode
# =============================================================================
set -euo pipefail
source .env.crucible

TOTAL_RESOLUTIONS="${CRUCIBLE_MIN_RESOLUTIONS:-50}"
TARGETS=("$HOLESKY_CHAIN_ID" "$ARB_SEPOLIA_CHAIN_ID" "$POLYGON_AMOY_CHAIN_ID")

i=0
success=0
fail=0
declare -A per_chain_success per_chain_total

while (( i < TOTAL_RESOLUTIONS )); do
  chain="${TARGETS[$(( i % ${#TARGETS[@]} ))]}"
  per_chain_total[$chain]=$(( ${per_chain_total[$chain]:-0} + 1 ))

  echo "[$((i+1))/$TOTAL_RESOLUTIONS] Resolution → chain $chain"
  result=$(curl -sS -X POST "http://api-server:8080/api/crucible/resolutions" \
    -H 'Content-Type: application/json' \
    -H "Idempotency-Key: crucible-$(date +%s%N)-$chain" \
    -d "{\"chain_id\":$chain,\"capital_cap_usd\":${CRUCIBLE_CAPITAL_CAP_USD:-0.00}}" \
    | jq -r '.status')

  if [[ "$result" == "applied" ]]; then
    success=$((success+1))
    per_chain_success[$chain]=$(( ${per_chain_success[$chain]:-0} + 1 ))
  else
    fail=$((fail+1))
  fi
  i=$((i+1))
  sleep 0.5
done

echo "========================================================================"
echo "OMEGA Crucible Resolutions — Final Report"
echo "========================================================================"
printf "Total:       %d\n" "$TOTAL_RESOLUTIONS"
printf "Applied:     %d  (%.2f%%)\n" "$success" "$(python3 -c "print($success/$TOTAL_RESOLUTIONS*100)")"
printf "Rejected:    %d\n" "$fail"
echo
echo "Per-chain:"
for ch in "${TARGETS[@]}"; do
  s=${per_chain_success[$ch]:-0}
  t=${per_chain_total[$ch]:-0}
  rate=$(python3 -c "print($s/max($t,1)*100)")
  printf "  chain_id=%-7s  %d/%d  (%.2f%%)\n" "$ch" "$s" "$t" "$rate"
done

# Crucible Survival gate
min_rate="${CRUCIBLE_MIN_SUCCESS_RATE_PCT:-95}"
actual_rate=$(python3 -c "print($success/$TOTAL_RESOLUTIONS*100)")
if python3 -c "import sys; sys.exit(0 if float('$actual_rate') >= float('$min_rate') else 1)"; then
  echo "CRUCIBLE SURVIVAL: PASSED ($actual_rate% ≥ $min_rate%)"
  exit 0
else
  echo "CRUCIBLE SURVIVAL: FAILED ($actual_rate% < $min_rate%)"
  exit 1
fi
