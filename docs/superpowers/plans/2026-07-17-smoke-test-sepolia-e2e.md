# Plan de Prueba de Humo E2E — SimulatorV2 en Sepolia

> **Gate 4** `arbx-pre-execute-checklist` aplicado: este documento es un PLAN, no una ejecución. El deploy a Sepolia requiere OK explícito del operador. Claude NO ejecutará deploy ni broadcast.

---

## 1. Objetivo

Validar que `simulator-v2` + `sim-ctl` + `sim-core::execute_multistep_revm` pueden ejecutar una simulación REVM completa (flash-funded round-trip) contra contratos deployados en **Sepolia** y retornar `SIM_SUCCESS` con `wrapped_calldata` no vacío.

---

## 2. Pre-requisitos (Operator-gated)

| # | Requisito | Origen | Estado |
|---|-----------|--------|--------|
| 1 | Contratos deployados en Sepolia (ArbitrageExecutor, FlashLoanExecutor, AllowanceManager, AdminTimelock) | `forge script` adaptado para Sepolia | **PENDIENTE** — requiere OK operador |
| 2 | `FLASHLOAN_EXECUTOR_11155111` env var | Deploy Sepolia | **PENDIENTE** |
| 3 | `ARBITRAGE_EXECUTOR` env var | Deploy Sepolia | **PENDIENTE** |
| 4 | Sepolia RPC endpoint (public node o Alchemy) | `.env` / Excel | **PENDIENTE** |
| 5 | Pool de liquidez Sepolia real (WETH/USDC) | Uniswap V2 Sepolia | **Ya existe** |
| 6 | Caller con ETH en Sepolia (para gas en sim) | Faucet Sepolia | **PENDIENTE** |
| 7 | `SIM_BACKEND=revm` en sim-ctl | `.env` / docker compose | **PENDIENTE** |

---

## 3. Adaptación del Deploy Script para Sepolia

El script `contracts/script/DeployMainnet.s.sol` necesita mínimos cambios para Sepolia:

```solidity
// NUEVO: DeploySepolia.s.sol
// Cambios respecto a DeployMainnet:
// 1. chainid == 11155111 (no 1)
// 2. AAVE_V3_POOL_SEPOLIA = 0x6Ae43d3271d1bB2bD0B19dF999473B5Bb40eF162
// 3. minDelay = 3600 (1h, no 24h) — Sepolia = testnet
// 4. CONFIRM_SEPOLIA_DEPLOY en lugar de CONFIRM_MAINNET_DEPLOY
// 5. Multisig puede ser EOA (no requiere contract.code.length > 0 en testnet)
```

**Riesgo:** Sepolia no tiene Aave V3 real con liquidez WETH/USDC suficiente. Si Aave no funciona, usar **Balancer Vault** como fallback de flash loan.

---

## 4. Script de Prueba de Humo

El script envía un `OpportunityCandidate` construido manualmente al endpoint `/simulate` de sim-ctl y verifica la respuesta.

```bash
#!/usr/bin/env bash
# scripts/smoke-test-sepolia.sh
# Prueba de humo E2E: simulator-v2 contra Sepolia
# Uso: ./scripts/smoke-test-sepolia.sh

set -euo pipefail

SIM_CTL_URL="${SIM_CTL_URL:-http://localhost:3003}"
SEPOLIA_RPC="${SEPOLIA_RPC_URL:-https://ethereum-sepolia-rpc.publicnode.com}"

# ── 1. Health check ────────────────────────────────────────────────
echo "=== 1. Health check ==="
hc=$(curl -sf "${SIM_CTL_URL}/health" | jq -r '.status // "DOWN"')
if [[ "$hc" != "UP" ]]; then
  echo "FAIL: sim-ctl health=$hc"
  exit 1
fi
echo "OK: sim-ctl UP"

# ── 2. Fork status (opcional, solo Anvil) ──────────────────────────
echo "=== 2. Fork status ==="
fs=$(curl -sf "${SIM_CTL_URL}/fork-status" 2>/dev/null | jq -r '.metrics.status // "NO_FORK"' || echo "NO_FORK")
echo "Fork status: $fs"

# ── 3. Construir OpportunityCandidate de prueba ────────────────────
echo "=== 3. Construir candidate ==="
# WETH y USDC en Sepolia (direcciones canónicas)
WETH="0xfff9976782d46cc05630d1f6ebab18b2324d6b14"
USDC="0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"
# Router Uniswap V2 en Sepolia
UNI_V2_ROUTER="0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3"

# Direcciones de contratos deployados (OPERADOR: reemplazar tras deploy)
ARBITRAGE_EXECUTOR="${ARBITRAGE_EXECUTOR:?Requerido: dirección del ArbitrageExecutor en Sepolia}"
FLASHLOAN_EXECUTOR="${FLASHLOAN_EXECUTOR:?Requerido: dirección del FlashLoanExecutor en Sepolia}"
CALLER="${TEST_CALLER:?Requerido: dirección con ETH en Sepolia para simular gas}"

CANDIDATE=$(cat <<EOF
{
  "route_source": "simctl_lookup",
  "candidate": {
    "opportunity_id": "$(uuidgen || python3 -c "import uuid; print(uuid.uuid4())")",
    "chain_id": 11155111,
    "block_number": 0,
    "route_fingerprint": "sepolia_smoke_weth_usdc_v2",
    "pool_addresses": ["0x0000000000000000000000000000000000000000"],
    "token_addresses": ["$WETH", "$USDC"],
    "dex_adapters": ["uniswap_v2", "uniswap_v2"],
    "amount_in": "1000000000000000000",
    "expected_amount_out": "0",
    "gross_profit": "0",
    "decimals": {
      "${WETH,,}": 18,
      "${USDC,,}": 6
    }
  }
}
EOF
)

# ── 4. Enviar a /simulate ──────────────────────────────────────────
echo "=== 4. Simulación REVM ==="
resp=$(curl -sf -X POST "${SIM_CTL_URL}/simulate" \
  -H "Content-Type: application/json" \
  -d "$CANDIDATE" 2>/dev/null || echo '{"error": "curl_failed"}')

echo "Response:"
echo "$resp" | jq .

# ── 5. Validaciones ────────────────────────────────────────────────
echo "=== 5. Validaciones ==="

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
if [[ "$wrapped" == "null" || "$wrapped" == "" ]]; then
  echo "FAIL: wrapped_calldata empty"
  exit 1
fi
echo "OK: wrapped_calldata length=${#wrapped}"

profit=$(echo "$resp" | jq -r '.simulated_profit_token_in // "0"')
if [[ "$profit" == "0" || "$profit" == "null" ]]; then
  echo "WARN: simulated_profit_token_in=$profit (expected > 0 for a real arb)"
  # No fallamos — puede ser mercado sin arbitraje real
else
  echo "OK: simulated_profit_token_in=$profit"
fi

# ── 6. Paridad byte-a-byte (opcional, avanzado) ────────────────────
echo "=== 6. Paridad byte-a-byte ==="
# Comparar wrapped_calldata contra lo que el encoder produce offline
# Esto requiere un script Rust/TS separado; marcamos como TODO.
echo "TODO: comparar wrapped_calldata contra encode offline"

echo ""
echo "=== PRUEBA DE HUMO: PASS ==="
```

---

## 5. Métricas a Verificar Post-Prueba

| Métrica | Ubicación | Valor Esperado |
|---------|-----------|----------------|
| `arbx_simulation_total{simulator="revm",passed="true"}` | Prometheus | > 0 después de la prueba |
| `arbx_simulation_total{simulator="revm",passed="false"}` | Prometheus | >= 0 (fallos honestos) |
| Logs `event="multistep.profit"` | Loki/Grafana | Aparece con `accepted=true` |
| `simulator_revm_success` (counter interno) | searcher-rs metrics | +1 |

---

## 6. Checklist de Cierre G-SIM-1

Una vez que la prueba de humo pase:

- [ ] `ARBX_SIMULATOR_V2_READY` puede setearse a `"true"` en el VPS
- [ ] `g-sim-1.ts` Layer 2 pasa a green
- [ ] Layer 3 (métricas) requiere **fix**: `arbx_simulation_total` no se incrementa en código (hallazgo de esta auditoría) — necesita un `.with_label_values(&["revm", "true"]).inc()` en `sim-ctl/src/sim_runner.rs` o `searcher-rs/src/scanner.rs`
- [ ] G-SIM-1 global pasa a green (sim-ctl alive + v2_ready=true + métricas recientes)

---

## 7. Hallazgo de Auditoría: `arbx_simulation_total` Fantasma

**Problema:** La métrica `arbx_simulation_total` está registrada en `shared-rs/src/metrics.rs:49` e inicializada en `init_metrics()`, pero **ningún archivo del backend la incrementa** (no hay `.with_label_values(...).inc()` en ningún crate).

**Impacto:** G-SIM-1 Layer 3 siempre reporta "no recent samples" incluso cuando simulaciones fluyen. La métrica está muerta.

**Fix requerido:** Agregar en `sim-ctl/src/sim_runner.rs` (después de `run_real_simulation`) o en `searcher-rs/src/scanner.rs` (después de SIM_SUCCESS):

```rust
use shared_rs::metrics::SIMULATIONS_TOTAL;

// En el path de éxito:
SIMULATIONS_TOTAL
    .with_label_values(&["revm", "true"])
    .inc();

// En el path de fallo:
SIMULATIONS_TOTAL
    .with_label_values(&["revm", "false"])
    .inc();
```

---

## 8. Riesgos y Mitigaciones

| Riesgo | Mitigación |
|--------|------------|
| Sepolia no tiene liquidez real para arbitrar | Usar cantidades pequeñas (1 WETH), aceptar `SIM_SUCCESS` con profit=0 como "sim funciona, mercado no" |
| Aave V3 no disponible en Sepolia | Usar Balancer Vault como fallback de flash loan |
| Caller sin ETH en Sepolia | Pedir faucet Sepolia o usar un address con ETH conocido |
| `SIM_BACKEND` no está seteado a `revm` | Verificar `.env` antes de probar |

---

## 9. Próximo Paso

**¿Operador autoriza deploy a Sepolia?**

Si SÍ → adapto `DeployMainnet.s.sol` → Sepolia, genero el script de deploy, y el operador ejecuta con `forge script --ledger` o `cast wallet`.

Si NO → el plan queda como documento de referencia. G-SIM-1 permanece RED hasta que se complete la validación E2E.

> **Capital expuesto en esta prueba: 0.** Es simulación REVM contra fork de Sepolia. No hay broadcast, no hay firma, no hay pérdida de fondos.
