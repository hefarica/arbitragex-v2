# README + Checklist del Operador
## OMEGA Protocol — ArbitrageX-V2
### Guía de Operación para Infraestructura On-Chain

---

**Versión**: 2.0.0-OMEGA  
**Última actualización**: 2026-05-14  
**Nivel de lectura**: Operador técnico  
**Tiempo estimado de lectura**: 20 minutos  
**Prerrequisitos**: Solidity intermedio, Foundry básico, conocimiento de EVM

---

## Tabla de Contenidos

1. [Visión Rápida](#1-visión-rápida)
2. [Stack Tecnológico](#2-stack-tecnológico)
3. [Setup de Entorno](#3-setup-de-entorno)
4. [Comandos Fundidos](#4-comandos-fundidos)
5. [Dry-Run en Fork Local](#5-dry-run-en-fork-local)
6. [Deploy en Producción](#6-deploy-en-producción)
7. [Checklist del Operador](#7-checklist-del-operador)
8. [Troubleshooting](#8-troubleshooting)
9. [Seguridad y Emergencias](#9-seguridad-y-emergencias)
10. [Glosario OMEGA](#10-glosario-omega)

---

## 1. Visión Rápida

```
╔══════════════════════════════════════════════════════════════════════════════╗
║  OMEGA PROTOCOL — Sistema de Estabilización On-Chain                       ║
║                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────┐    ║
║  │  ENTRY POINT: Executor.sol                                          │    ║
║  │  ├── Flash Convergence (Aave V3, Balancer, MakerDAO)              │    ║
║  │  ├── Adapter Registry (Uniswap V2/V3, Curve, Balancer)            │    ║
║  │  ├── Invariantes Termodinámicas (6 validaciones holonómicas)      │    ║
║  │  └── Kill Switch (Active / Suspended / Terminated)                │    ║
║  │                                                                     │    ║
║  │  INVARIANTE FUNDAMENTAL: Y_topo = Y_raw - F_net > 0               │    ║
║  ╰─────────────────────────────────────────────────────────────────────╯    ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

### ¿Qué es OMEGA?

OMEGA es el protocolo on-chain de ArbitrageX-V2 que ejecuta **Resoluciones Holonómicas** — ciclos de convergencia de mercado sobre $N \geq 3$ variedades de liquidez (pools DEX). Cada ejecución:

1. Toma un flashloan (capital sin posesión previa)
2. Ejecuta una secuencia de swaps sobre múltiples DEXs
3. Reembolsa el préstamo + premium
4. Si el **Rendimiento Topológico Neto** es positivo → yield al Cold Treasury
5. Si cualquier invariante falla → **REVERT ATÓMICO**, sin pérdida de capital

### Flujo de una Operación

```
1. Operador firma bundle (off-chain)                    ← Execution Signer
2. Gas Sponsor envía transacción                         ← Gas Sponsor (paga gas)
3. Executor valida prueba holonómica (6 checks)          ← on-chain
4. Flashloan recibido (Aave/Balancer/MakerDAO)           ← atómico
5. Secuencia de N swaps ejecutada                        ← adaptadores DEX
6. Préstamo reembolsado + premium pagado                 ← atómico
7. Yield neto transferido a Cold Treasury                ← automático
8. Señal de convergencia emitida (Redis → WebSocket)     ← telemetría
```

---

## 2. Stack Tecnológico

### 2.1 On-Chain (Smart Contracts)

| Componente | Tecnología | Versión |
|-----------|-----------|---------|
| Framework | Foundry | ≥ 0.2.0 |
| Lenguaje | Solidity | ^0.8.24 |
| EVM Target | Cancun | — |
| Optimizer | Enabled | 200 runs |
| OpenZeppelin | ReentrancyGuard, Pausable | v5 |
| Solmate | SafeERC20 | v6 |

### 2.2 Off-Chain (Infraestructura)

| Componente | Tecnología | Uso |
|-----------|-----------|-----|
| Pipeline SED | Rust (sed-core) | Detección de oportunidades |
| Telemetría | Redis + WebSocket | Señales de convergencia |
| Monitoreo | Grafana + Prometheus | Métricas on-chain |
| Wallets | HSM + Hardware | Firma de transacciones |
| RPC | Alchemy/Infura | Conexión a cadenas |

### 2.3 Cadenas Soportadas

| Red | chainId | Estado | Gas Token |
|-----|---------|--------|-----------|
| Ethereum | 1 | Activo | ETH |
| Arbitrum | 42161 | Activo | ETH |
| Optimism | 10 | Activo | ETH |
| Base | 8453 | Activo | ETH |
| Polygon | 137 | Activo | MATIC |
| BSC | 56 | Activo | BNB |

---

## 3. Setup de Entorno

### 3.1 Instalación de Foundry

```bash
# Instalar Foundry
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Verificar instalación
forge --version  # Debe mostrar ≥ 0.2.0
cast --version
anvil --version
```

### 3.2 Clonar Repositorio

```bash
git clone git@github.com:arbitragex-v2/omega-protocol.git
cd omega-protocol

# Instalar dependencias
forge install

# Verificar que todo compila
forge build --sizes
```

### 3.3 Configurar RPCs

Crear archivo `.env` basado en `.env.example`:

```bash
cp .env.example .env
```

Editar `.env` con tus valores:

```bash
# ═══════════════════════════════════════════════════════════════════════════
# CONFIGURACIÓN OMEGA PROTOCOL — Variables de Entorno
# ═══════════════════════════════════════════════════════════════════════════

# ── RPC Endpoints (Alchemy/Infura requeridos) ──
# Obtener en: https://dashboard.alchemy.com
ETH_RPC=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
ARB_RPC=https://arb-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
OP_RPC=https://opt-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
BASE_RPC=https://base-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
POLYGON_RPC=https://polygon-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
BSC_RPC=https://bsc-dataseed.binance.org

# ── Claves Privadas (ALMACENAR EN HSM, NO EN .env PARA PROD) ──
# En producción, usar: cast wallet sign --account (keystore)
# O: AWS KMS / Azure Key Vault
GAS_SPONSOR_KEY=0x1234567890abcdef...
EXECUTION_SIGNER_KEY=0xabcdef1234567890...
GOVERNANCE_KEY=0xfedcba0987654321...

# ── API Keys para verificación ──
ETHERSCAN_API_KEY=ABC123...
ARBISCAN_API_KEY=DEF456...
OPTIMISTIC_ETHERSCAN_API_KEY=GHI789...
BASESCAN_API_KEY=JKL012...
POLYGONSCAN_API_KEY=MNO345...
BSCSCAN_API_KEY=PQR678...
```

**SEGURIDAD**: Nunca commitear `.env`. El archivo ya está en `.gitignore`.

### 3.4 Verificar Conectividad

```bash
# Verificar que los RPCs funcionan
echo "=== Ethereum ==="
cast block-number --rpc-url $ETH_RPC

echo "=== Arbitrum ==="
cast block-number --rpc-url $ARB_RPC

echo "=== Optimism ==="
cast block-number --rpc-url $OP_RPC

echo "=== Base ==="
cast block-number --rpc-url $BASE_RPC
```

### 3.5 Compilar Contratos

```bash
# Compilación completa con reporte de tamaño
forge build --sizes

# Salida esperada:
# ═══════════════════════════════════════════════════════════════════════════
# ║ Contract               ║ Size (kB) ║ Margin (kB) ║
# ║ Executor               ║ 12.4      ║ 12.2        ║
# ║ UniswapV2Adapter       ║ 3.2       ║ 21.4        ║
# ║ UniswapV3Adapter       ║ 5.1       ║ 19.5        ║
# ║ CurveAdapter           ║ 4.8       ║ 19.8        ║
# ║ BalancerAdapter        ║ 6.2       ║ 18.4        ║
# ║ OmegaDeployFactory     ║ 1.8       ║ 22.8        ║
# ═══════════════════════════════════════════════════════════════════════════
```

---

## 4. Comandos Fundidos

### 4.1 Tabla Maestra de Comandos

| Comando | Descripción | Frecuencia |
|---------|-------------|------------|
| `forge build` | Compilar contratos | Cada cambio |
| `forge build --sizes` | Compilar + tamaño | Pre-deploy |
| `forge test` | Ejecutar tests | Cada cambio |
| `forge test -vvvv` | Tests con logs máximos | Debugging |
| `forge test --match-contract X` | Tests específicos | Focalizado |
| `forge test --fork-url $RPC` | Tests en fork | Integración |
| `forge coverage` | Reporte de cobertura | Pre-commit |
| `forge snapshot` | Snapshot de gas | Baseline |
| `forge snapshot --check` | Verificar gas vs baseline | CI/CD |
| `forge script script/Deploy.s.sol` | Simular deploy | Pre-deploy |
| `forge script script/Deploy.s.sol --broadcast` | Deploy real | Producción |
| `forge verify-contract` | Verificar en explorer | Post-deploy |
| `cast call` | Llamada de lectura | Operación |
| `cast send` | Transacción | Operación |
| `cast balance` | Consultar balance | Monitoreo |
| `anvil --fork-url $RPC` | Nodo local fork | Testing |

### 4.2 forge build

```bash
# Compilación básica
forge build

# Compilación forzada (ignorar cache)
forge build --force

# Compilación + tamaños (verificar límite de 24KB)
forge build --sizes

# Compilación optimizada (producción)
forge build --optimize --optimizer-runs 200
```

### 4.3 forge test

```bash
# Todos los tests
forge test

# Tests con verbosity máxima (logs, stack traces)
forge test -vvvv

# Tests específicos por contrato
forge test --match-contract ExecutorTest

# Tests específicos por función
forge test --match-test test_ExecuteRealSwapSequence

# Tests en fork de mainnet
forge test --fork-url $ETH_RPC --match-contract Integration

# Tests con reporte de gas
forge test --gas-report

# Tests con snapshot de gas
forge snapshot

# Verificar que el gas no incrementó desde el baseline
forge snapshot --check
```

### 4.4 forge script

```bash
# Simular deploy (dry-run)
forge script script/Deploy.s.sol \
    --rpc-url $ETH_RPC \
    -vvvv

# Deploy real en mainnet
forge script script/Deploy.s.sol \
    --rpc-url $ETH_RPC \
    --private-key $GAS_SPONSOR_KEY \
    --broadcast \
    --verify \
    --etherscan-api-key $ETHERSCAN_API_KEY \
    -vvvv

# Deploy con gas multiplier (redes congestionadas)
forge script script/Deploy.s.sol \
    --rpc-url $ETH_RPC \
    --private-key $GAS_SPONSOR_KEY \
    --broadcast \
    --gas-estimate-multiplier 150 \
    -vvvv

# Resume deploy fallido (reutiliza transacciones previas)
forge script script/Deploy.s.sol \
    --rpc-url $ETH_RPC \
    --private-key $GAS_SPONSOR_KEY \
    --broadcast \
    --resume \
    -vvvv
```

### 4.5 cast (interacción con contratos)

```bash
# ── Lectura (gratis, no requiere gas) ──

# Balance de un contrato
cast balance $EXECUTOR_ADDR --rpc-url $ETH_RPC

# Llamada a función view
cast call $EXECUTOR_ADDR "totalExecutions()" --rpc-url $ETH_RPC

# Llamada con parámetros
cast call $EXECUTOR_ADDR "isAuthorized(address)" $OPERATOR_ADDR --rpc-url $ETH_RPC

# Obtener storage slot
cast storage $EXECUTOR_ADDR 0 --rpc-url $ETH_RPC

# ── Escritura (requiere gas + firma) ──

# Enviar transacción
cast send $EXECUTOR_ADDR "pause()" \
    --rpc-url $ETH_RPC \
    --private-key $GAS_SPONSOR_KEY

# Enviar con valor
cast send $TREASURY_ADDR \
    --value "$(cast tw 0.1)" \
    --rpc-url $ETH_RPC \
    --private-key $GAS_SPONSOR_KEY

# Calcular salt para CREATE2
SALT=$(cast keccak "OMEGA_FACTORY_v2_0_0")
echo "Salt: $SALT"

# Keccak256 de string
cast keccak "UNISWAP_V2_1"
```

### 4.6 anvil (nodo local)

```bash
# Fork de mainnet en local
anvil --fork-url $ETH_RPC

# Fork en bloque específico
anvil --fork-url $ETH_RPC --fork-block-number 18000000

# Fork con block time fijo
anvil --fork-url $ETH_RPC --block-time 12

# Fork con 10 cuentas pre-fondeadas
anvil --fork-url $ETH_RPC --accounts 10 --balance 10000

# Puerto personalizado
anvil --fork-url $ETH_RPC --port 8546
```

---

## 5. Dry-Run en Fork Local

### 5.1 Procedimiento Completo de Dry-Run

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# DRY-RUN: Simulación completa en fork local antes de mainnet
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

echo "═══════════════════════════════════════════════════════════════════════"
echo "OMEGA Protocol — Dry-Run en Fork Local"
echo "═══════════════════════════════════════════════════════════════════════"

# ── Paso 1: Iniciar anvil en background ───────────────────────────────────
echo "[1/7] Iniciando anvil con fork de mainnet..."
anvil --fork-url $ETH_RPC --fork-block-number 18000000 &
ANVIL_PID=$!
sleep 5

# Verificar que anvil responde
cast block-number --rpc-url http://localhost:8545
echo "✅ Anvil listo (PID: $ANVIL_PID)"

# ── Paso 2: Compilar contratos ────────────────────────────────────────────
echo "[2/7] Compilando contratos..."
forge build --sizes

# ── Paso 3: Deploy Factory ────────────────────────────────────────────────
echo "[3/7] Deploy del Factory CREATE2..."
LOCAL_RPC="http://localhost:8545"
FACTORY_SALT=$(cast keccak "OMEGA_FACTORY_v2_0_0_2026_05_14")

forge script script/DeployFactory.s.sol \
    --rpc-url $LOCAL_RPC \
    --private-key "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" \
    --broadcast \
    --sig "run(bytes32)" \
    "$FACTORY_SALT" \
    -vvvv

FACTORY_ADDR=$(cat "broadcast/DeployFactory.s.sol/1/run-latest.json" | jq -r '.receipts[0].contractAddress')
echo "✅ Factory desplegado en: $FACTORY_ADDR"

# ── Paso 4: Deploy de contratos core ──────────────────────────────────────
echo "[4/7] Deploy de Executor + Adaptadores..."
export FACTORY_ADDRESS=$FACTORY_ADDR

forge script script/Deploy.s.sol \
    --rpc-url $LOCAL_RPC \
    --private-key "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" \
    --broadcast \
    -vvvv

# ── Paso 5: Verificar direcciones ─────────────────────────────────────────
echo "[5/7] Verificando direcciones..."
EXECUTOR=$(cat "broadcast/Deploy.s.sol/1/run-latest.json" | jq -r '.returns.executor.value')
echo "Executor: $EXECUTOR"

# ── Paso 6: Ejecutar tests de integración ─────────────────────────────────
echo "[6/7] Ejecutando tests de integración en fork local..."
forge test \
    --match-contract Integration \
    --rpc-url $LOCAL_RPC \
    -vvvv

# ── Paso 7: Verificar invariantes ─────────────────────────────────────────
echo "[7/7] Verificando invariantes..."
forge test \
    --match-contract InvariantTest \
    --rpc-url $LOCAL_RPC \
    -vvvv

# ── Limpiar ───────────────────────────────────────────────────────────────
echo "Deteniendo anvil..."
kill $ANVIL_PID 2>/dev/null

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "✅ DRY-RUN COMPLETADO EXITOSAMENTE"
echo "═══════════════════════════════════════════════════════════════════════"
echo ""
echo "Listo para deploy en mainnet. Ejecutar:"
echo "  forge script script/Deploy.s.sol --rpc-url \$ETH_RPC --broadcast"
```

### 5.2 Checklist del Dry-Run

Antes de ejecutar el dry-run, verificar:

- [ ] `anvil` disponible (`anvil --version`)
- [ ] `.env` configurado con `$ETH_RPC` válido
- [ ] `forge build` pasa sin errores
- [ ] `forge test` pasa (sin integración)
- [ ] Puerto 8545 disponible (`lsof -i :8545` vacío)

---

## 6. Deploy en Producción

### 6.1 Pre-Deploy (t-2h)

```bash
# Verificar saldo del Gas Sponsor
cast balance $GAS_SPONSOR_ADDR --rpc-url $ETH_RPC
# Debe ser ≥ 0.5 ETH

# Verificar nonce (debe ser predecible)
cast nonce $GAS_SPONSOR_ADDR --rpc-url $ETH_RPC

# Verificar que el RPC está sincronizado
LOCAL_BLOCK=$(cast block-number --rpc-url $ETH_RPC)
EXPLORER_BLOCK=$(curl -s "https://api.etherscan.io/api?module=proxy&action=eth_blockNumber&apikey=$ETHERSCAN_API_KEY" | jq -r '.result' | cast to-dec)
DIFF=$((EXPLORER_BLOCK - LOCAL_BLOCK))
echo "Diferencia de bloques: $DIFF (debe ser < 5)"

# Predecir direcciones
SALT_EXECUTOR=$(cast keccak $(cast abi-encode "f(uint256,string)" 1 "Executor"))
echo "Salt Executor: $SALT_EXECUTOR"
```

### 6.2 Deploy (t=0)

```bash
# Ejecutar SOP de deploy
source script/deploy.sh
```

### 6.3 Post-Deploy (t+1h)

```bash
# Verificar contratos en etherscan
forge script script/Verify.s.sol --rpc-url $ETH_RPC -vvvv

# Guardar artefactos
cp artifacts/deploy_1.json "artifacts/deploy_1_$(date +%Y%m%d_%H%M%S).json"

# Notificar al equipo
echo "Deploy completado en $(date -u +%Y-%m-%dT%H:%M:%SZ)"
```

---

## 7. Checklist del Operador

### 7.1 Checklist de Setup (primer uso)

- [ ] **1. Foundry instalado** — `forge --version` ≥ 0.2.0
- [ ] **2. Repo clonado** — `git clone` + `forge install`
- [ ] **3. `.env` configurado** — RPCs, claves, API keys
- [ ] **4. `.env` en `.gitignore`** — `grep .env .gitignore` (debe estar)
- [ ] **5. RPCs verificados** — `cast block-number` funciona en todas las redes
- [ ] **6. Compilación exitosa** — `forge build --sizes` sin errores
- [ ] **7. Tests unitarios pasan** — `forge test` al 100%
- [ ] **8. Wallets generadas** — Gas Sponsor, Execution Signer, Governance
- [ ] **9. Wallets almacenadas en HSM** — No en disco, no en .env para prod
- [ ] **10. Kill Switch testeado** — `cast send` pause/resume funciona en anvil

### 7.2 Checklist de Dry-Run (antes de cada deploy)

- [ ] **1. Rama actual** — `git branch` muestra `main` o release tag
- [ ] **2. Código actualizado** — `git pull origin main`
- [ ] **3. Compilación limpia** — `forge build --force --sizes`
- [ ] **4. Tests pasan** — `forge test` al 100%
- [ ] **5. Coverage aceptable** — `forge coverage` > 90%
- [ ] **6. Gas snapshot OK** — `forge snapshot --check`
- [ ] **7. Anvil disponible** — puerto 8545 libre
- [ ] **8. Dry-run completo** — script ejecutado sin errores
- [ ] **9. Invariantes verificadas** — 6 invariantes holonómicas pasan
- [ ] **10. Reporte revisado** — logs revisados manualmente

### 7.3 Checklist de Deploy en Producción (10 puntos)

| # | Check | Comando de Verificación |
|---|-------|------------------------|
| 1 | Gas Sponsor fondeado | `cast balance $GAS_SPONSOR --rpc-url $RPC` ≥ 0.5 ETH |
| 2 | Execution Signer en cero | `cast balance $EXEC_SIGNER --rpc-url $RPC` = 0 |
| 3 | Nonce verificado | `cast nonce $GAS_SPONSOR --rpc-url $RPC` predecible |
| 4 | RPC sincronizado | Diferencia con explorer < 5 bloques |
| 5 | `.env` correcto | Todas las variables seteadas |
| 6 | Contratos compilados | `forge build --sizes` exitoso |
| 7 | Slither limpio | `slither src/` — 0 high/medium |
| 8 | Echidna pasado | `echidna test/invariant/` — 0 invariantes rotas |
| 9 | Dry-run exitoso | Script completo en anvil sin errores |
| 10 | 2FA/Multisig listo | Wallets de governance accesibles |

### 7.4 Checklist Post-Deploy

- [ ] **1. Direcciones guardadas** — `artifacts/deploy_{chainId}.json` creado
- [ ] **2. Código verificado** — Visible en etherscan/arbiscan/etc.
- [ ] **3. Adaptadores registrados** — `cast call adapterRegistry` retorna non-zero
- [ ] **4. AuthorizedExecutors set** — `cast call isAuthorized` retorna true
- [ ] **5. Cold Treasury configurado** — Dirección correcta en Executor
- [ ] **6. Kill Switch en Active** — `cast call killSwitchState` = 0 (Active)
| 7 | Tests de integración | `forge test --match-contract Integration --fork-url $RPC` |
| 8 | Eventos emitidos | `cast logs` muestra eventos de deploy |
| 9 | Telemetría activa | Señales de convergencia en Redis |
| 10 | Monitoreo configurado | Dashboards de Grafana con métricas |

---

## 8. Troubleshooting

### 8.1 Errores de Compilación

| Error | Causa | Solución |
|-------|-------|----------|
| `stack too deep` | Demasiadas variables locales | Usar `via_ir = true` en foundry.toml o refactorizar |
| `contract too large` | > 24KB | Reducir funciones, usar librerías |
| `invalid opcode` | EVM version incompatible | Cambiar `evm_version` en foundry.toml |
| `unknown pragma` | Solidity version | Verificar `pragma solidity ^0.8.24` |

### 8.2 Errores de Deploy

| Error | Causa | Solución |
|-------|-------|----------|
| `nonce too low` | Nonce incorrecto | Esperar sync o especificar `--nonce` |
| `insufficient funds` | Gas Sponsor sin ETH | Fondear wallet |
| `replacement transaction underpriced` | Tx pendiente | Aumentar gas price o esperar |
| `execution reverted` | Constructor falla | Revisir constructor args con `-vvvv` |
| `CREATE2: address collision` | Salt reutilizado | Generar salt nuevo |

### 8.3 Errores de Ejecución

| Error (on-chain) | Causa | Solución |
|-----------------|-------|----------|
| `Executor: unauthorized` | Caller no autorizado | `authorizeExecutor()` desde governance |
| `Executor: open contour` | γ(0) ≠ γ(1) | Recomputar trayectoria off-chain |
| `Executor: trivial holonomy` | \|∮(dp/p)\| < 1e-12 | Oportunidad ya resuelta, esperar |
| `Executor: non-positive yield` | Y_topo ≤ 0 | Aumentar minYieldBp o esperar |
| `Executor: adapter not found` | Key incorrecta en registry | `registerAdapter()` correctamente |
| `Flash: slippage exceeded` | Movimiento de precio durante ejecución | Aumentar minAmountOut o reducir tamaño |
| `Flash: yield insufficient` | F_net > Y_raw | Reducir cantidad o esperar mejor oportunidad |

### 8.4 Errores de Infraestructura

| Error | Causa | Solución |
|-------|-------|----------|
| `RPC timeout` | Nodo sobrecargado | Cambiar RPC o aumentar timeout |
| `Redis connection refused` | Redis caído | Restart Redis o failover |
| `WebSocket disconnected` | Conexión perdida | Reconectar automáticamente |
| `kill switch activated` | Operador activó emergencia | Contactar operator lead |

### 8.5 Comandos de Diagnóstico

```bash
# ── Diagnóstico rápido ──

# Verificar estado del Executor
function executor_status() {
    echo "=== Executor Status ==="
    echo "Total executions: $(cast call $EXECUTOR 'totalExecutions()' --rpc-url $ETH_RPC | cast to-dec)"
    echo "Accumulated yield: $(cast call $EXECUTOR 'accumulatedTopologicalYield()' --rpc-url $ETH_RPC | cast to-dec)"
    echo "Paused: $(cast call $EXECUTOR 'paused()' --rpc-url $ETH_RPC)"
}

# Verificar adaptadores registrados
function adapters_status() {
    echo "=== Registered Adapters ==="
    for key in "UNISWAP_V2_1" "UNISWAP_V3_1" "CURVE_V1_1" "BALANCER_V2_1"; do
        KEY_HASH=$(cast keccak "$key")
        ADDR=$(cast call $EXECUTOR "adapterRegistry(bytes32)" "$KEY_HASH" --rpc-url $ETH_RPC)
        echo "$key: $ADDR"
    done
}

# Verificar estado de wallets
function wallets_status() {
    echo "=== Wallet Balances ==="
    echo "Gas Sponsor: $(cast balance $GAS_SPONSOR --rpc-url $ETH_RPC | cast to-fix 18) ETH"
    echo "Execution Signer: $(cast balance $EXEC_SIGNER --rpc-url $ETH_RPC | cast to-fix 18) ETH"
    echo "Cold Treasury: $(cast balance $TREASURY --rpc-url $ETH_RPC | cast to-fix 18) ETH"
}

# Estado completo
function full_status() {
    executor_status
    adapters_status
    wallets_status
    echo "Block: $(cast block-number --rpc-url $ETH_RPC)"
    echo "Gas Price: $(cast gas-price --rpc-url $ETH_RPC | cast to-gwei) Gwei"
}
```

---

## 9. Seguridad y Emergencias

### 9.1 Kill Switch

```bash
# Verificar estado actual
cast call $EXECUTOR "killSwitchState()" --rpc-url $ETH_RPC
# 0 = Active, 1 = Suspended, 2 = Terminated

# Activar Suspended (pausa temporal)
cast send $EXECUTOR "setKillSwitchState(uint8)" 1 \
    --rpc-url $ETH_RPC \
    --private-key $EMERGENCY_KILL_KEY

# Activar Terminated (parada total)
cast send $EXECUTOR "setKillSwitchState(uint8)" 2 \
    --rpc-url $ETH_RPC \
    --private-key $EMERGENCY_KILL_KEY

# Reactivar (requiere governance)
cast send $EXECUTOR "setKillSwitchState(uint8)" 0 \
    --rpc-url $ETH_RPC \
    --private-key $GOVERNANCE_KEY
```

### 9.2 Rollback de Emergencia

```bash
# Ejecutar script de rollback
forge script script/EmergencyRollback.s.sol \
    --rpc-url $ETH_RPC \
    --private-key $EMERGENCY_KILL_KEY \
    --broadcast \
    -vvvv
```

### 9.3 Escalación

| Nivel | Situación | Acción | Responsable |
|-------|-----------|--------|-------------|
| 1 | Tests fallan | No deploy | Desarrollador |
| 2 | Gas > 100 Gwei | Pausar operaciones | Operador |
| 3 | Invariante falla | Kill Switch → Suspended | Operador on-call |
| 4 | Fondo comprometido | Kill Switch → Terminated | Operator Lead |
| 5 | Bug crítico | Rollback completo + auditoría | Arquitecto + Legal |

---

## 10. Glosario OMEGA

| Término | Definición | Referencia |
|---------|-----------|------------|
| **Resolución Holonómica** | Ciclo de convergencia sobre N≥3 variedades de liquidez | White Paper §2.3 |
| **Rendimiento Topológico** | $Y_{\text{topo}} = \oint_\gamma \frac{dp}{p} - F_{\text{net}}$ | White Paper §2.3 |
| **Trayectoria de Contorno Cerrado** | γ: [0,1] → ℳ con γ(0) = γ(1) | White Paper §2.2 |
| **Variedad de Liquidez** | Par (ℛ, g) de reservas + tensor métrico | White Paper §2.1 |
| **Asimetría de Información** | Diferencial de conocimiento entre operador y mercado | SOP §3 |
| **Ghost Protocol** | Capital en exposición cero fuera de la ventana atómica | White Paper §5.2 |
| **Fail-Honest** | Toda falla produce un error explícito | R8 spec |
| **Flash Convergence** | Mecánica de superposición temporal con flashloans | White Paper §4 |
| **Determinismo CREATE2** | Direcciones idénticas cross-chain | SOP §2 |
| **Varianza Monótona** | $
\sigma_{\text{agg}}$ nunca crece | GateManager spec |
| **Señal de Convergencia** | Evento telemetry con métricas del pipeline | telemetry/mod.rs |
| **Snapshot de Entropía** | (tx/sec, gas price, entropy score, reserve divergence) | telemetry/mod.rs |
| **Manifold** | Pool de liquidez individual (Uniswap, Curve, etc.) | holonomic.rs |
| **Holonomía** | ∮_γ (dp/p) — integral de contorno de precios | holonomic.rs:115 |
| **Fricción de Red** | $F_{\text{net}} = F_{\text{gas}} + F_{\text{slippage}} + F_{\text{LP}}$ | holonomic.rs |
| **CEI Pattern** | Checks → Effects → Interactions (seguridad) | White Paper §5.3 |

---

## Apéndice A: Diagrama de Arquitectura Completa

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        ARQUITECTURA OMEGA COMPLETA                           │
│                                                                              │
│   OFF-CHAIN (sed-core Rust)                    ON-CHAIN (Solidity)           │
│   ═══════════════════════════                  ═══════════════════           │
│                                                                              │
│   ┌─────────────────┐                          ┌─────────────────┐          │
│   │  SED Engine     │──ConvergenceSignal──────▶│  RedisPublisher │          │
│   │  (detection)    │                          │  (arbx:signals: │          │
│   └─────────────────┘                          │   convergence)  │          │
│          │                                     └─────────────────┘          │
│          │                                            │                      │
│          │  EntropySnapshot                           │ WS                   │
│          │  {tx/sec, gas, entropy, divergence}       ▼                      │
│          │                                     ┌─────────────────┐          │
│          │                                     │  SedConvergence │          │
│          │                                     │  Panel (React)  │          │
│          │                                     │  (frontend)     │          │
│          │                                     └─────────────────┘          │
│          │                                                                   │
│   ┌──────▼──────┐    BundlePosition<T>    ┌──────────────────────────────┐  │
│   │  GateManager │───────────────────────▶│      Executor.sol            │  │
│   │  4 barriers  │                        │  ═══════════════════════     │  │
│   │              │                        │  • onlyAuthorizedExecutor   │  │
│   │  1. Infra    │                        │  • validBundle (N≥3)         │  │
│   │  2. KillSw   │                        │  • verifyHolonomicProof     │  │
│   │  3. Stochas  │                        │  • executeAdapterCalls      │  │
│   │  4. Variance │                        │  • postConditions           │  │
│   └──────────────┘                        └──────────┬───────────────────┘  │
│                                                      │                      │
│                           ┌─────────────────────────┼──────────────────┐   │
│                           ▼                         ▼                  ▼   │
│                    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐│
│                    │ UniswapV2   │    │ UniswapV3   │    │   Curve     ││
│                    │ Adapter     │    │ Adapter     │    │   Adapter   ││
│                    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘│
│                           │                   │                   │       │
│                           ▼                   ▼                   ▼       │
│                    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐│
│                    │ Flashloans   │    │   Balancer   │    │   Aave V3   ││
│                    │ (MakerDAO)   │    │   Vault      │    │   Pool      ││
│                    └──────────────┘    └──────────────┘    └──────────────┘│
│                                                                              │
│   TELEMETRÍA:                                                                │
│   • totalExecutions ───────────────────────────────────── on-chain          │
│   • accumulatedTopologicalYield ──────────────────────── on-chain           │
│   • ConvergenceSignal (entropy, latency) ─────────────── Redis → WS        │
│   • Gas consumed ──────────────────────────────────────── on-chain event    │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Apéndice B: Referencias Rápidas

### Documentos OMEGA

| Documento | Archivo | Contenido |
|-----------|---------|-----------|
| White Paper | `OMEGA_WHITE_PAPER_ONCHAIN.md` | Arquitectura, matemáticas, seguridad |
| SOP Deploys | `OMEGA_SOP_DEPLOYS.md` | Despliegues multi-chain, wallets |
| Roadmap | `OMEGA_ROADMAP_CONTRACTS.md` | Plan de implementación paso a paso |
| README | `OMEGA_README_CONTRACTS.md` | Esta guía |

### Enlaces Útiles

- Foundry Book: https://book.getfoundry.sh
- Solidity Docs: https://docs.soliditylang.org
- EVM Opcodes: https://evm.codes
- OpenZeppelin: https://docs.openzeppelin.com
- ANEXOS_V1.2.md: Referencia interna SED Core

---

**Document End — README + Checklist del Operador OMEGA Protocol**

*"La operación segura no es un accidente — es el resultado de checklists ejecutados con disciplina."*
