#!/usr/bin/env bash
# PRICE-SEEDER-02: + guard anti-race vs ediciones del operador.
# Si entre el GET y el PUT otro actor (operador/UI) cambio token_prices_usd o
# base_token_price_usd, este ciclo SALE sin escribir (nunca pisar al operador).
set -euo pipefail
DEPLOY=/opt/arbitragex-v2
TOKEN=$(grep "^ARBX_ADMIN_TOKEN=" "$DEPLOY/.env" | cut -d= -f2- | tr -d "\"'")
curl -s "http://127.0.0.1:8080/api/v1/trading-config?chain_id=1" -o /tmp/ps_cur.json
curl -s "http://127.0.0.1:8080/api/v1/prices/live?chain_id=1" -o /tmp/ps_pr.json
DEC=$(python3 - <<'PY'
import json, sys
try:
    cur = json.load(open('/tmp/ps_cur.json')); pr = json.load(open('/tmp/ps_pr.json'))
except Exception:
    print("SKIP"); sys.exit(0)
prices = pr.get('prices') or {}
allowed = cur.get('allowed_token_symbols') or []
tp = {s: prices[s] for s in allowed if s in prices}
if not tp: print("SKIP"); sys.exit(0)
old = cur.get('token_prices_usd') or {}
base = cur.get('base_token_symbol') or 'WETH'
newbase = prices.get(base)
if tp == old and (not newbase or abs(float(newbase) - float(cur.get('base_token_price_usd') or 0)) <= 0.01):
    print("NOCHANGE"); sys.exit(0)
print("PUT")
PY
)
if [ "$DEC" != "PUT" ]; then exit 0; fi
# GUARD anti-race: re-lectura inmediata; si el estado cambió vs el primer GET,
# el operador (u otro escritor) intervino en la ventana -> abortar sin escribir.
python3 - <<'PY'
import json, sys
c1 = json.load(open('/tmp/ps_cur.json'))
c2 = json.loads(__import__('subprocess').run(
    ["curl","-s","http://127.0.0.1:8080/api/v1/trading-config?chain_id=1"],
    capture_output=True, text=True).stdout)
k1 = json.dumps(c1.get('token_prices_usd') or {}, sort_keys=True)
k2 = json.dumps(c2.get('token_prices_usd') or {}, sort_keys=True)
b1 = c1.get('base_token_price_usd'); b2 = c2.get('base_token_price_usd')
if k1 != k2 or b1 != b2:
    print("RACE-SKIP", flush=True)
    sys.exit(42)
PY
RC=$?
if [ "$RC" -eq 42 ]; then
  echo "$(date -u +%FT%TZ) race-guard: operador/otro escritor intervino en la ventana — ciclo abortado sin escribir"
  exit 0
fi
python3 - <<'PY'
import json, subprocess
cur = json.load(open('/tmp/ps_cur.json')); pr = json.load(open('/tmp/ps_pr.json'))
prices = pr.get('prices') or {}
allowed = cur.get('allowed_token_symbols') or []
tp = {s: prices[s] for s in allowed if s in prices}
cur['token_prices_usd'] = tp
base = cur.get('base_token_symbol') or 'WETH'
if base in prices: cur['base_token_price_usd'] = float(prices[base])
cur['updated_by'] = 'pricebus-auto-refresher'
json.dump(cur, open('/tmp/ps_body.json','w'))
PY
R=$(curl -s -X PUT "http://127.0.0.1:8080/admin/trading-config/1" -H "x-arbx-admin-token: $TOKEN" -H 'Content-Type: application/json' -d @/tmp/ps_body.json | head -c 80)
echo "$(date -u +%FT%TZ) seeded: $R"
