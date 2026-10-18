#!/usr/bin/env bash
# G-PAPER-STACK-01 — deploy del paper stack vía ERC1967Proxy (PR-D1, 0xf92ee8a9).
#
# This gate EXECUTES the deploy. Before PR-D1 no test did: the only caller was
# `run_multistep`, so the InvalidInitialization revert (0xf92ee8a9) survived in
# `main` untouched. The gate builds a PaperStackSpec, deploys impl + ERC1967Proxy
# (the proxy constructor initializes the implementation in the same tx) and
# requires `Ok` — that is the reachability the impl-only path never had.
#
# Needs an RPC (fork or bare anvil) in PAPER_STACK_RPC. Without one the gate
# SKIPs (exit 2): a gate that cannot measure is never reported as green, and a
# manual gate without a date never runs — hence the script form.
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
  echo "  correr con:  PAPER_STACK_RPC=<url> $0"
  exit 2
fi

export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
echo "G-PAPER-STACK-01 — deploy real del paper stack vía ERC1967Proxy (RPC=$PAPER_STACK_RPC)"
if cargo test -p sim-core --lib -- --ignored --nocapture paper_stack_deploy_initializes_via_erc1967_proxy_ok; then
  echo "G-PAPER-STACK-01 — PASS (deploy del stack = Ok)"
  exit 0
else
  echo "G-PAPER-STACK-01 — FAIL (el deploy no llega a Ok; ver salida arriba)"
  exit 1
fi
