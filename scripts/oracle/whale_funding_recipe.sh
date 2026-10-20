#!/usr/bin/env bash
# whale_funding_recipe.sh — setup DETERMINISTA del fork para el signer del sim.
#
# POR QUÉ EXISTE (SIM-ORACLE-01)
# -----------------------------
# `sim-ctl/src/signer_funding.rs` resuelve el balance ERC20 del signer SONDEANDO
# slots de storage (`CANDIDATE_SLOTS = [0, 2, 3, 9]`): escribe un centinela,
# comprueba `balanceOf`, y si el token es un proxy o usa otro layout agota el
# loop y devuelve `sim_signer_funding_slot_unresolved` (~89,5 % de las sims en
# el corte post-deploy). El ETH nativo NI SIQUIERA tiene slot: si el harness
# resuelve uno, está resolviendo algo que no existe.
#
# Este guion NO adivina índices. Reemplaza el sondeo por tres operaciones
# deterministas, cada una verificable por lectura independiente:
#
#   1. NATIVO  -> `anvil_setBalance` (override directo de la CUENTA).
#   2. ERC20   -> impersonar una ballena y hacer un `transfer` REAL al signer.
#   3. APPROVE -> tx explícita del signer al spender.
#   4. ASERCIONES de setup: ETH >= 2*gas, balanceOf >= size, allowance >= size.
#      Si falla -> `fixture_*` y la sim NO se intenta.
#
# VERIFICADO 2026-10-18 contra `anvil --fork-url https://eth.drpc.org
# --fork-block-number 26197506`:
#
#   A) inicial:        USDC=0            ETH=2 000 005 329 574 wei
#   B) override:       ETH=2 000 000 000 000 000 000 wei      (2e18, 2 ETH)
#   C) whale+transfer: USDC=40 000 000 unidades base          (40 USDC)
#   D) approve:        allowance=40 000 000 unidades base
#
# USO
# ---
#   anvil --fork-url <ARCHIVE_RPC> --fork-block-number <N> --port 8545 &
#   SIGNER=0x... WHALE=0x... ./whale_funding_recipe.sh
#
# Variables: RPC (default http://127.0.0.1:8545), TOKEN, SIGNER, WHALE, SPENDER,
# SIZE (unidades base del token). Requiere `cast` en el PATH.

set -euo pipefail

RPC="${RPC:-http://127.0.0.1:8545}"
TOKEN="${TOKEN:-0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48}"   # USDC mainnet (6 dec)
SIGNER="${SIGNER:?falta SIGNER (dirección del signer del sim)}"
WHALE="${WHALE:?falta WHALE (una ballena REAL del token, impersonable)}"
SPENDER="${SPENDER:?falta SPENDER (router/executor que va a hacer transferFrom)}"
SIZE="${SIZE:-40000000}"          # 40 USDC en unidades base
NATIVE="${NATIVE:-2000000000000000000}"  # 2 ETH
GAS_LIMIT="${GAS_LIMIT:-250000}"

say() { printf '%s\n' "$*"; }
assert_ge() { # assert_ge <nombre> <obtenido> <minimo>
  if [ "$2" -lt "$3" ]; then
    say "FIXTURE_FAIL $1: $2 < $3 (setup incompleto -> fixture_*, la sim NO se intenta)"
    exit 1
  fi
  say "OK $1: $2 >= $3"
}

say "== 0. estado inicial =="
BAL0="$(cast call "$TOKEN" 'balanceOf(address)(uint256)' "$SIGNER" --rpc-url "$RPC" | awk '{print $1}')"
say "balanceOf(signer)=$BAL0  eth=$(cast balance "$SIGNER" --rpc-url "$RPC")"

say "== 1. nativo por OVERRIDE de cuenta (no hay slot que resolver) =="
cast rpc anvil_setBalance "$SIGNER" "$(cast to-hex "$NATIVE")" --rpc-url "$RPC" >/dev/null

say "== 2. ERC20: ballena + transfer REAL =="
cast rpc anvil_impersonateAccount "$WHALE" --rpc-url "$RPC" >/dev/null
cast rpc anvil_setBalance "$WHALE" "$(cast to-hex "$NATIVE")" --rpc-url "$RPC" >/dev/null
cast send "$TOKEN" "transfer(address,uint256)" "$SIGNER" "$SIZE" \
  --from "$WHALE" --unlocked --gas-limit "$GAS_LIMIT" --rpc-url "$RPC" >/dev/null

say "== 3. approve explícito del signer al spender =="
cast rpc anvil_impersonateAccount "$SIGNER" --rpc-url "$RPC" >/dev/null
cast send "$TOKEN" "approve(address,uint256)" "$SPENDER" "$SIZE" \
  --from "$SIGNER" --unlocked --gas-limit "$GAS_LIMIT" --rpc-url "$RPC" >/dev/null

say "== 4. aserciones de setup (medidas, no supuestas) =="
ETH="$(cast balance "$SIGNER" --rpc-url "$RPC")"
BAL="$(cast call "$TOKEN" 'balanceOf(address)(uint256)' "$SIGNER" --rpc-url "$RPC" | awk '{print $1}')"
ALLOW="$(cast call "$TOKEN" 'allowance(address,address)(uint256)' "$SIGNER" "$SPENDER" --rpc-url "$RPC" | awk '{print $1}')"
BASEFEE="$(cast base-fee --rpc-url "$RPC")"
GAS_COST=$((GAS_LIMIT * 2 * BASEFEE))     # gas_units * (baseFee*2)
assert_ge "signer.ETH >= 2*gas_cost(wei)" "$ETH" "$((2 * GAS_COST))"
assert_ge "balanceOf(signer) >= size" "$BAL" "$SIZE"
assert_ge "allowance(signer,spender) >= size" "$ALLOW" "$SIZE"

cast rpc anvil_stopImpersonatingAccount "$WHALE" --rpc-url "$RPC" >/dev/null
cast rpc anvil_stopImpersonatingAccount "$SIGNER" --rpc-url "$RPC" >/dev/null
say "== SETUP OK (determinista, sin resolver un solo slot) =="
