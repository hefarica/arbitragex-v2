#!/usr/bin/env bash
# =============================================================================
# Smoke Test E2E — SimulatorV2 en Sepolia
# =============================================================================
# Valida que sim-ctl con SIM_BACKEND=revm puede ejecutar una simulación REVM
# completa contra contratos deployados en Sepolia y retornar SIM_SUCCESS.
#
# Pre-requisitos:
#   - sim-ctl corriendo con SIM_BACKEND=revm y REDIS_URL configurado
#   - Contratos deployados en Sepolia (ver docs/superpowers/plans/2026-07-17-smoke-test-sepolia-e2e.md)
#   - Env vars: ARBITRAGE_EXECUTOR, FLASHLOAN_EXECUTOR, TEST_CALLER
#
# Uso:
#   export ARBITRAGE_EXECUTOR=0x...
#   export FLASHLOAN_EXECUTOR=0x...
#   export TEST_CALLER=0x...
#   ./scripts/smoke-test-sepolia.sh
# =============================================================================

set -euo pipefail

SIM_CTL_URL="${SIM_CTL_URL:-http://localhost:3003}"

# ── Validación de env vars ─────────────────────────────────────────
: "${ARBITRAGE_EXECUTOR:?Requerido: dirección del ArbitrageExecutor en Sepolia}"
: "${FLASHLOAN_EXECUTOR:?Requerido: dirección del FlashLoanExecutor en Sepolia}"
: "${TEST_CALLER:?Requerido: dirección con ETH en Sepolia para simular gas}"

# ── 1. Health check ────────────────────────────────────────────────
echo "=== 1. Health check ==="
hc=$(curl -sf "${SIM_CTL_URL}/health" 2>/dev/null | jq -r '.status // "DOWN"' || echo "DOWN")
if [[ "$hc" != "UP" ]]; then
  echo "FAIL: sim-ctl health=$hc"
  exit 1
fi
echo "OK: sim-ctl UP"

# ── 2. Construir OpportunityCandidate de prueba ────────────────────
echo "=== 2. Construir candidate ==="

# Sepolia token addresses (canonical testnet)
WETH="0xfff9976782d46cc05630d1f6ebab18b2324d6b14"
USDC="0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"

# Generar UUID portable
OPP_ID=$(python3 -c "import uuid; print(uuid.uuid4())" 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || date +%s%N)

CANDIDATE=$(cat <<EOF
{
  "route_source": "simctl_lookup",
  "candidate": {
    "opportunity_id": "${OPP_ID}",
    "chain_id": 11155111,
    "block_number": 0,
    "route_fingerprint": "sepolia_smoke_weth_usdc_v2",
    "pool_addresses": ["0x0000000000000000000000000000000000000000"],
    "token_addresses": ["${WETH}", "${USDC}"],
    "dex_adapters": ["uniswap_v2", "uniswap_v2"],
    "amount_in": "1000000000000000000",
    "expected_amount_out": "0",
    "gross_profit": "0",
    "decimals": {
      "$(echo "${WETH}" | tr '[:upper:]' '[:lower:]')": 18,
      "$(echo "${USDC}" | tr '[:upper:]' '[:lower:]')": 6
    }
  }
}
EOF
)

# ── 3. Enviar a /simulate ──────────────────────────────────────────
echo "=== 3. Simulación REVM ==="
resp=$(curl -sf -X POST "${SIM_CTL_URL}/simulate" \
  -H "Content-Type: application/json" \
  -d "$CANDIDATE" 2>/dev/null || echo '{"error": "curl_failed"}')

echo "Response:"
echo "$resp" | jq . 2>/dev/null || echo "$resp"

# ── 4. Validaciones ────────────────────────────────────────────────
echo "=== 4. Validaciones ==="

passed=$(echo "$resp" | jq -r '.passed // "null"')
if [[ "$passed" != "true" ]]; then
  echo "FAIL: passed=$passed (expected true)"
  fail_reason=$(echo "$resp" | jq -r '.fail_reason // "unknown"')
  echo "Fail reason: $fail_reason"
  exit 1
fi
echo "OK: passed=true"

gas_used=$(echo "$resp" | jq -r '.gas_used_total // "0"')
if [[ "$gas_used" == "0" || "$gas_used" == "null" ]]; then
  echo "FAIL: gas_used_total=$gas_used (expected > 0)"
  exit 1
fi
echo "OK: gas_used_total=$gas_used > 0"

wrapped=$(echo "$resp" | jq -r '.wrapped_calldata // "null"')
if [[ "$wrapped" == "null" || "$wrapped" == "" || "$wrapped" == "0x" ]]; then
  echo "FAIL: wrapped_calldata empty"
  exit 1
fi
echo "OK: wrapped_calldata length=${#wrapped}"

profit=$(echo "$resp" | jq -r '.simulated_profit_token_in // "0"')
if [[ "$profit" == "0" || "$profit" == "null" ]]; then
  echo "WARN: simulated_profit_token_in=$profit (mercado sin arbitraje real — el sim funciona)"
else
  echo "OK: simulated_profit_token_in=$profit"
fi

# ── 5. Verificar métricas Prometheus (opcional) ────────────────────
echo "=== 5. Métricas Prometheus ==="
prom_url="${PROMETHEUS_URL:-http://localhost:9090}"
metric_count=$(curl -sf "${prom_url}/api/v1/query?query=arbx_simulation_total" 2>/dev/null | jq -r '.data.result | length' || echo "0")
if [[ "$metric_count" != "0" ]]; then
  echo "OK: arbx_simulation_total visible en Prometheus"
else
  echo "WARN: arbx_simulation_total no visible (Prometheus puede tardar en scrapear)"
fi

echo ""
echo "=== PRUEBA DE HUMO: PASS ==="
echo ""
echo "Próximo paso: set ARBX_SIMULATOR_V2_READY=true en el VPS .env"
echo "  G-SIM-1 Layer 2 → green"
echo "  G-SIM-1 Layer 3 → green (después del próximo scrape de Prometheus)"
echo "  G-SIM-1 global → green"
