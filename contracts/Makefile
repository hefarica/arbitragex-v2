# ============================================================
# OMEGA Multi-Chain Deployment Makefile
# ArbitrageX-V2 — Foundry-based deployment pipeline
# ============================================================
# Usage:
#   make build          — Compila todos los contratos
#   make test           — Ejecuta tests unitarios
#   make fuzz           — Ejecuta fuzzing (10k runs)
#   make deploy-all     — Deploy secuencial en 6 chains
#   make verify-all     — Verificacion en 6 explorers
#   make all            — Pipeline completo: build -> test -> deploy
#
# Pre-requisitos:
#   1. source ../.env.edge cargado en el shell
#   2. forge instalado (Foundry)
#   3. Wallets fondeadas (ver FUNDE_WALLETS.md)
# ============================================================

.PHONY: build test fuzz snapshot \
        deploy-ethereum deploy-arbitrum deploy-optimism \
        deploy-base deploy-polygon deploy-bsc deploy-all \
        verify-ethereum verify-arbitrum verify-optimism \
        verify-base verify-polygon verify-bsc verify-all \
        status-all clean help

# ============================================================
# CONFIGURACION
# ============================================================
FOUNDRY_PROFILE ?= default
SCRIPT_PATH     ?= script/DeployMultichain.s.sol

# Optimizer
OPTIMIZER_RUNS  ?= 200
SOLC_VERSION    ?= 0.8.23

# Flags comunes de forge
FORGE_FLAGS     := --optimizer-runs $(OPTIMIZER_RUNS) \
                   --use solc:$(SOLC_VERSION) \
                   --via-ir

# Flags de broadcast (gas/priority)
BROADCAST_FLAGS := --broadcast \
                   --verify \
                   --slow

# ============================================================
# 1. BUILD
# ============================================================
build:
	@echo "=== OMEGA: Compilando contratos ==="
	forge build $(FORGE_FLAGS) --extra-output-files abi bin
	@echo "✓ Build completado"

# ============================================================
# 2. TEST
# ============================================================
test:
	@echo "=== OMEGA: Ejecutando tests ==="
	forge test -vvv $(FORGE_FLAGS)
	@echo "✓ Tests completados"

# Tests con gas report
test-gas:
	@echo "=== OMEGA: Tests con Gas Report ==="
	forge test -vvv $(FORGE_FLAGS) --gas-report
	@echo "✓ Tests + Gas Report completados"

# ============================================================
# 3. FUZZING (10,000 runs por defecto)
# ============================================================
fuzz:
	@echo "=== OMEGA: Iniciando fuzzing (10,000 runs) ==="
	forge test --fuzz-runs 10000 -vvv $(FORGE_FLAGS)
	@echo "✓ Fuzzing completado"

# Fuzzing intensivo (100k runs)
fuzz-deep:
	@echo "=== OMEGA: Fuzzing intensivo (100,000 runs) ==="
	forge test --fuzz-runs 100000 -vvv $(FORGE_FLAGS)
	@echo "✓ Deep fuzzing completado"

# ============================================================
# 4. SNAPSHOT DE GAS
# ============================================================
snapshot:
	@echo "=== OMEGA: Generando snapshot de gas ==="
	forge snapshot $(FORGE_FLAGS) --snap gas-snapshots/.gas-snapshot-latest
	@echo "✓ Snapshot guardado en gas-snapshots/"

# ============================================================
# 5. DEPLOYS POR CHAIN
# ============================================================

# --- Ethereum Mainnet (Chain ID: 1) ---
deploy-ethereum:
	@echo "=== OMEGA DEPLOY: Ethereum Mainnet (ChainID: 1) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_1) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		--chain-id 1 \
		--priority-gas-price $(PRIORITY_FEE_GWEI_1)gwei \
		--with-gas-price $(GAS_PRICE_GWEI_1)gwei \
		-vvvv
	@echo "✓ Deploy Ethereum completado"
	@echo "  Explorer: $(EXPLORER_1)/address/$$CONTRACT_ADDRESS"

# --- Arbitrum One (Chain ID: 42161) ---
deploy-arbitrum:
	@echo "=== OMEGA DEPLOY: Arbitrum One (ChainID: 42161) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_42161) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(ARBISCAN_API_KEY) \
		--chain-id 42161 \
		--priority-gas-price $(PRIORITY_FEE_GWEI_42161)gwei \
		--with-gas-price $(GAS_PRICE_GWEI_42161)gwei \
		-vvvv
	@echo "✓ Deploy Arbitrum completado"
	@echo "  Explorer: $(EXPLORER_42161)/address/$$CONTRACT_ADDRESS"

# --- Optimism (Chain ID: 10) ---
deploy-optimism:
	@echo "=== OMEGA DEPLOY: Optimism (ChainID: 10) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_10) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(OPTIMISTIC_ETHERSCAN_KEY) \
		--chain-id 10 \
		--priority-gas-price $(PRIORITY_FEE_GWEI_10)gwei \
		-vvvv
	@echo "✓ Deploy Optimism completado"
	@echo "  Explorer: $(EXPLORER_10)/address/$$CONTRACT_ADDRESS"

# --- Base (Chain ID: 8453) ---
deploy-base:
	@echo "=== OMEGA DEPLOY: Base (ChainID: 8453) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_8453) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(BASESCAN_API_KEY) \
		--chain-id 8453 \
		--priority-gas-price $(PRIORITY_FEE_GWEI_8453)gwei \
		-vvvv
	@echo "✓ Deploy Base completado"
	@echo "  Explorer: $(EXPLORER_8453)/address/$$CONTRACT_ADDRESS"

# --- Polygon PoS (Chain ID: 137) ---
deploy-polygon:
	@echo "=== OMEGA DEPLOY: Polygon PoS (ChainID: 137) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_137) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(POLYGONSCAN_API_KEY) \
		--chain-id 137 \
		--priority-gas-price $(PRIORITY_FEE_GWEI_137)gwei \
		--legacy \
		-vvvv
	@echo "✓ Deploy Polygon completado"
	@echo "  Explorer: $(EXPLORER_137)/address/$$CONTRACT_ADDRESS"

# --- BNB Smart Chain (Chain ID: 56) ---
deploy-bsc:
	@echo "=== OMEGA DEPLOY: BSC (ChainID: 56) ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_56) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		$(BROADCAST_FLAGS) \
		--etherscan-api-key $(BSCSCAN_API_KEY) \
		--chain-id 56 \
		--with-gas-price $(GAS_PRICE_GWEI_56)gwei \
		--legacy \
		-vvvv
	@echo "✓ Deploy BSC completado"
	@echo "  Explorer: $(EXPLORER_56)/address/$$CONTRACT_ADDRESS"

# ============================================================
# 6. DEPLOY SECUENCIAL EN TODAS LAS CHAINS
# ============================================================
deploy-all: build test
	@echo ""
	@echo "╔══════════════════════════════════════════════════════════╗"
	@echo "║     OMEGA: INICIANDO DEPLOY MULTI-CHAIN SECUENCIAL      ║"
	@echo "╚══════════════════════════════════════════════════════════╝"
	@echo ""
	@echo "⚠️  Asegurate de tener .env.edge cargado:"
	@echo "   source ../.env.edge"
	@echo ""
	@read -p "Presiona ENTER para continuar con Ethereum Mainnet..."
	$(MAKE) deploy-ethereum
	@echo ""
	@read -p "Presiona ENTER para continuar con Arbitrum..."
	$(MAKE) deploy-arbitrum
	@echo ""
	@read -p "Presiona ENTER para continuar con Optimism..."
	$(MAKE) deploy-optimism
	@echo ""
	@read -p "Presiona ENTER para continuar con Base..."
	$(MAKE) deploy-base
	@echo ""
	@read -p "Presiona ENTER para continuar con Polygon..."
	$(MAKE) deploy-polygon
	@echo ""
	@read -p "Presiona ENTER para continuar con BSC..."
	$(MAKE) deploy-bsc
	@echo ""
	@echo "╔══════════════════════════════════════════════════════════╗"
	@echo "║     ✓ DEPLOY MULTI-CHAIN COMPLETADO EN 6 CHAINS         ║"
	@echo "╚══════════════════════════════════════════════════════════╝"

# ============================================================
# 7. VERIFICACION EN EXPLORERS
# ============================================================

verify-ethereum:
	@echo "=== OMEGA VERIFY: Ethereum ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_1) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		--chain-id 1 \
		-vvv

verify-arbitrum:
	@echo "=== OMEGA VERIFY: Arbitrum ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_42161) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(ARBISCAN_API_KEY) \
		--chain-id 42161 \
		-vvv

verify-optimism:
	@echo "=== OMEGA VERIFY: Optimism ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_10) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(OPTIMISTIC_ETHERSCAN_KEY) \
		--chain-id 10 \
		-vvv

verify-base:
	@echo "=== OMEGA VERIFY: Base ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_8453) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(BASESCAN_API_KEY) \
		--chain-id 8453 \
		-vvv

verify-polygon:
	@echo "=== OMEGA VERIFY: Polygon ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_137) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(POLYGONSCAN_API_KEY) \
		--chain-id 137 \
		--legacy \
		-vvv

verify-bsc:
	@echo "=== OMEGA VERIFY: BSC ==="
	forge script $(SCRIPT_PATH) \
		--rpc-url $(RPC_HTTP_56) \
		--private-key $(GAS_SPONSOR_PRIVATE_KEY) \
		--verify \
		--etherscan-api-key $(BSCSCAN_API_KEY) \
		--chain-id 56 \
		--legacy \
		-vvv

# Verificacion en todas las chains
verify-all: verify-ethereum verify-arbitrum verify-optimism verify-base verify-polygon verify-bsc
	@echo "✓ Verificacion completada en 6 explorers"

# ============================================================
# 8. STATUS / BALANCE CHECKS
# ============================================================
status-ethereum:
	@echo "=== BALANCES: Ethereum Mainnet ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_1) --ether) ETH"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_1) --ether) ETH"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_1) --ether) ETH"

status-arbitrum:
	@echo "=== BALANCES: Arbitrum ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_42161) --ether) ETH"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_42161) --ether) ETH"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_42161) --ether) ETH"

status-optimism:
	@echo "=== BALANCES: Optimism ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_10) --ether) ETH"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_10) --ether) ETH"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_10) --ether) ETH"

status-base:
	@echo "=== BALANCES: Base ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_8453) --ether) ETH"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_8453) --ether) ETH"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_8453) --ether) ETH"

status-polygon:
	@echo "=== BALANCES: Polygon ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_137)) wei"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_137)) wei"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_137)) wei"

status-bsc:
	@echo "=== BALANCES: BSC ==="
	@echo "GasSponsor:  $$(cast balance $(GAS_SPONSOR_ADDRESS) --rpc-url $(RPC_HTTP_56)) wei"
	@echo "ExecutionSigner: $$(cast balance $(EXECUTION_SIGNER_ADDRESS) --rpc-url $(RPC_HTTP_56)) wei"
	@echo "ColdTreasury: $$(cast balance $(COLD_TREASURY_ADDRESS) --rpc-url $(RPC_HTTP_56)) wei"

# Status en todas las chains
status-all: status-ethereum status-arbitrum status-optimism status-base status-polygon status-bsc

# ============================================================
# 9. UTILIDADES
# ============================================================
clean:
	@echo "=== Limpiando artifacts ==="
	forge clean
	rm -rf cache/ out/ broadcast/
	@echo "✓ Limpieza completada"

# Pipeline completo: build + test + deploy en todas las chains
all: build test deploy-all
	@echo ""
	@echo "╔══════════════════════════════════════════════════════════╗"
	@echo "║         OMEGA: PIPELINE COMPLETO FINALIZADO              ║"
	@echo "╚══════════════════════════════════════════════════════════╝"

# Ayuda
help:
	@echo "OMEGA Multi-Chain Deployment — Comandos disponibles:"
	@echo ""
	@echo "BUILD & TEST:"
	@echo "  make build          — Compila contratos"
	@echo "  make test           — Ejecuta tests"
	@echo "  make test-gas       — Tests con gas report"
	@echo "  make fuzz           — Fuzzing (10k runs)"
	@echo "  make fuzz-deep      — Fuzzing intensivo (100k runs)"
	@echo "  make snapshot       — Snapshot de gas"
	@echo ""
	@echo "DEPLOY (requiere .env.edge cargado):"
	@echo "  make deploy-ethereum   — Deploy en Ethereum Mainnet"
	@echo "  make deploy-arbitrum   — Deploy en Arbitrum One"
	@echo "  make deploy-optimism   — Deploy en Optimism"
	@echo "  make deploy-base       — Deploy en Base"
	@echo "  make deploy-polygon    — Deploy en Polygon"
	@echo "  make deploy-bsc        — Deploy en BSC"
	@echo "  make deploy-all        — Deploy secuencial en 6 chains"
	@echo ""
	@echo "VERIFY:"
	@echo "  make verify-ethereum   — Verificar en Etherscan"
	@echo "  make verify-arbitrum   — Verificar en Arbiscan"
	@echo "  make verify-optimism   — Verificar en Optimistic"
	@echo "  make verify-base       — Verificar en Basescan"
	@echo "  make verify-polygon    — Verificar en Polygonscan"
	@echo "  make verify-bsc        — Verificar en Bscscan"
	@echo "  make verify-all        — Verificar en 6 explorers"
	@echo ""
	@echo "STATUS:"
	@echo "  make status-ethereum   — Balances en Ethereum"
	@echo "  make status-arbitrum   — Balances en Arbitrum"
	@echo "  make status-optimism   — Balances en Optimism"
	@echo "  make status-base       — Balances en Base"
	@echo "  make status-polygon    — Balances en Polygon"
	@echo "  make status-bsc        — Balances en BSC"
	@echo "  make status-all        — Balances en 6 chains"
	@echo ""
	@echo "PIPELINE:"
	@echo "  make all              — Pipeline completo (build+test+deploy)"
	@echo "  make clean            — Limpiar artifacts"
	@echo "  make help             — Mostrar esta ayuda"

# ============================================================
# EOF — OMEGA Multi-Chain Deployment Makefile
# ============================================================