#!/usr/bin/env bash
# G-PAPER-STACK-01 — deploy del paper stack vía ERC1967Proxy, por el camino
# CABLEADO con el gas real de cada call site (PR-D1 = 0xf92ee8a9; PR-D2 = gas) y
# con el Pool de Aave V3 resuelto DINÁMICAMENTE (POOL-DYN-01 = getPool()).
#
# This gate EXECUTES the deploy. Before PR-D1 no test did: the only caller was
# `run_multistep`, so the InvalidInitialization revert (0xf92ee8a9) survived in
# `main` untouched.
#
# PR-D2 (2026-10-18): the gate ran ONLY the hand-built spec at 15_000_000, so it
# stayed green while the REAL wire was broken — `searcher-rs::scanner` passed
# `gas_limit_per_step = 30_000_000`, which exceeds the OSAKA per-tx cap
# (EIP-7825, 2^24 = 16_777_216); the transactor rejected the deploy tx before
# executing it. The gate now ALSO runs `paper_stack_wired_path_deploys_ok_at_call_site_gas`,
# which builds the spec through `PaperStackSpec::for_round_trip` (the exact
# builder the five production call sites use) at each call site's REAL gas —
# `30_000_000` included — and requires `Ok`.
#
# POOL-DYN-01 (2026-10-18): `AAVE_POOL_<chain_id>` is no longer mandatory. The
# Pool is resolved from the chain's `PoolAddressesProvider`
# (`IPoolAddressesProvider.getPool()`) — the stable governance anchor (RULE 00,
# biblioteca §14) — with the env var as an explicit OVERRIDE. The gate now runs
# the offline fail-closed paths (no provider / failing read) AND, against a
# MAINNET FORK, both the override-wins path and the dynamic `getPool()` path
# (mainnet paridad: el Pool leído == 0x87870Bca…).
#
# Needs an RPC in PAPER_STACK_RPC: steps 5-6 read `getPool()`, so it MUST be a
# MAINNET FORK (a bare anvil has no Aave provider and step 6 correctly FAILs).
# Without any RPC the gate SKIPs (exit 2): a gate that cannot measure is never
# reported as green, and a manual gate without a date never runs — hence the
# script form.
#
# Exit 0 = PASS, 1 = FAIL, 2 = SKIP (no RPC / no toolchain).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/backend"

if ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  echo "G-PAPER-STACK-01 — SKIP: cargo ausente (toolchain no disponible)"; exit 2
fi
export PATH="$HOME/.cargo/bin:$PATH"

if [ -z "${PAPER_STACK_RPC:-}" ]; then
  echo "G-PAPER-STACK-01 — SKIP: PAPER_STACK_RPC ausente (no hay RPC que medir)"
  echo "  correr con:  PAPER_STACK_RPC=<fork-de-mainnet> $0"
  exit 2
fi

export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
# The real gas of the DEPLOY-CAPABLE production call sites (PR-D2):
#   searcher-rs/scanner.rs ................... 30_000_000  (the broken one)
#   searcher-rs/candidate_simulation.rs ...... 16_000_000
#   relays-client/execution_admission.rs ..... 16_000_000
# sim-ctl's 500_000 default is OUT of this list on purpose: it is below the
# paper-stack DEPLOY FLOOR (measured >2_000_000; the AE creation code alone
# deposits ~9.5 KB), so a deploy there always fails with OutOfGas — a separate,
# declared limitation of enabling PAPER_STACK_DEPLOY via sim-ctl with defaults,
# NOT the cap defect. The offline unit test still covers 500_000's cap no-op.
export PAPER_STACK_GAS_VALUES="${PAPER_STACK_GAS_VALUES:-30000000,16000000}"

rc=0

echo "G-PAPER-STACK-01 — paso 1/6: camino CABLEADO (for_round_trip) con el gas real de cada call site"
echo "  PAPER_STACK_GAS_VALUES=$PAPER_STACK_GAS_VALUES  (RPC=$PAPER_STACK_RPC)"
if ! cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_wired_path_deploys_ok_at_call_site_gas; then
  echo "  FAIL  el camino cableado no llega a Ok con el gas real de algún call site"
  rc=1
fi

echo "G-PAPER-STACK-01 — paso 2/6: la causa raíz — un gas SIN capar de 30M lo rechaza el tope OSAKA"
if ! cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_uncapped_gas_is_rejected_by_the_osaka_tx_cap; then
  echo "  FAIL  el tope por-tx OSAKA (EIP-7825) NO rechazó el gas sin capar"
  rc=1
fi

echo "G-PAPER-STACK-01 — paso 3/6: deploy vía ERC1967Proxy (spec construida a mano, 15M)"
if ! cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_deploy_initializes_via_erc1967_proxy_ok; then
  echo "  FAIL  el deploy del stack no llega a Ok"
  rc=1
fi

echo "G-PAPER-STACK-01 — paso 4/6: Pool DINÁMICO — fail-closed offline (sin provider / lectura falla)"
if ! cargo test -p sim-core --lib paper_stack_pool_resolution_fails_closed; then
  echo "  FAIL  la resolución del Pool no falla cerrado sin provider / con lectura fallida"
  rc=1
fi

echo "G-PAPER-STACK-01 — paso 5/6: Pool DINÁMICO — el override de env GANA al provider (fork mainnet)"
if ! cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_pool_env_override_beats_provider; then
  echo "  FAIL  el override AAVE_POOL_<chain> no ganó a getPool()"
  rc=1
fi

echo "G-PAPER-STACK-01 — paso 6/6: Pool DINÁMICO — getPool() leído en mainnet == 0x87870Bca… (fork mainnet)"
if ! cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_pool_resolved_from_provider_matches_mainnet_canonical; then
  echo "  FAIL  getPool() no resolvió el Pool canónico de mainnet (¿RPC sin fork de mainnet?)"
  rc=1
fi

if [ "$rc" = "0" ]; then
  echo "G-PAPER-STACK-01 — PASS (camino cableado con gas real + proxy deploy + Pool dinámico getPool() = Ok)"
  exit 0
else
  echo "G-PAPER-STACK-01 — FAIL (ver salida arriba)"
  exit 1
fi
