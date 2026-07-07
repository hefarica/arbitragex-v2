#!/usr/bin/env bash
# ArbitrageX v2 — smoke-test.sh
# Exercises public contracts. Honest about 501 endpoints (they MUST be 501, not fake 200).
set -u

PASS=0; FAIL=0
assert_http() {
  local name="$1" method="$2" url="$3" expected="$4" body="${5:-}"
  local code
  if [[ "$method" == "GET" ]]; then
    code=$(curl -o /dev/null -s -w '%{http_code}' --max-time 5 "$url")
  else
    code=$(curl -o /dev/null -s -w '%{http_code}' --max-time 5 -X "$method" -H 'content-type: application/json' -d "$body" "$url")
  fi
  if [[ "$code" == "$expected" ]]; then
    printf '  \033[32mPASS\033[0m %-45s %s %s\n' "$name" "$code" "$url"
    PASS=$((PASS+1))
  else
    printf '  \033[31mFAIL\033[0m %-45s got=%s expected=%s %s\n' "$name" "$code" "$expected" "$url"
    FAIL=$((FAIL+1))
  fi
}

# 1. Health checks
echo "== Health =="
for endpoint in \
  "searcher-rs|http://localhost:9001/health|200" \
  "selector-api|http://localhost:3002/health|200" \
  "sim-ctl|http://localhost:3003/health|200" \
  "recon|http://localhost:3004/health|200" \
  "relays-client|http://localhost:3005/health|200" \
  "api-server|http://localhost:8080/health|200" \
  "edge|http://localhost:8787/health|200" \
; do
  IFS='|' read -r name url exp <<< "$endpoint"
  assert_http "health.$name" GET "$url" "$exp"
done

# 2. Metrics endpoints exist
echo "== Metrics =="
for svc in searcher-rs:9001 selector-api:3002 sim-ctl:3003 recon:3004 relays-client:3005 api-server:8080 edge:8787; do
  name="${svc%%:*}"; port="${svc##*:}"
  assert_http "metrics.$name" GET "http://localhost:$port/metrics" 200
done

# 3. sim-ctl 501 (by design)
echo "== 501 endpoints (MUST be 501, never 200) =="
assert_http "sim-ctl.simulate.501" POST "http://localhost:3003/simulate" 501 '{"opportunity_id":"00000000-0000-0000-0000-000000000000"}'
# relays-client: 501 (no killswitch), 503 (killswitch ON)
assert_http "relays.execute.501_or_503" POST "http://localhost:3005/execute" 501 '{"opportunity_id":"00000000-0000-0000-0000-000000000000"}' \
  || assert_http "relays.execute.503" POST "http://localhost:3005/execute" 503 '{}'

# 4. recon returns 404 when empty
echo "== recon (404 on empty) =="
assert_http "recon.pnl.missing.404" GET "http://localhost:3004/pnl/00000000-0000-0000-0000-000000000000" 404

# 5. selector-api scoring
echo "== selector-api.score =="
SCORE_BODY='{"opportunity":{"id":"00000000-0000-0000-0000-000000000001","chain_id":1,"strategy_kind":"dex_arb","dex_a":"uniswap-v3","dex_b":"curve","pair_symbol":"WETH/USDC","token_in":"0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2","token_out":"0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48","amount_in_wei":"1000000000000000000","expected_profit_usd":30,"roi_pct":null,"risk_score":null,"block_number":null,"detected_at":"2026-04-20T00:00:00.000Z","trace_id":"00000000-0000-0000-0000-0000000000aa"},"simulation":null,"safety_score":80}'
assert_http "selector.score" POST "http://localhost:3002/score" 200 "$SCORE_BODY"

# 6. api-server /status aggregates (200 even if some upstream degraded)
echo "== api-server.status =="
assert_http "api.status" GET "http://localhost:8080/status" 200

# 7. admin endpoints require token
echo "== admin auth =="
assert_http "admin.killswitch.no_token" POST "http://localhost:8080/admin/killswitch" 401 '{"enabled":false}'

# 8. edge end-to-end
echo "== edge end-to-end =="
assert_http "edge.status" GET "http://localhost:8787/status" 200

echo
echo "Summary: $PASS passed, $FAIL failed"
exit $FAIL
