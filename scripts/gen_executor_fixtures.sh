#!/usr/bin/env bash
# G-SIM-1 WO-LR22.13 PR-B — regenerate the paper-executor bytecode fixtures.
#
# Rebuilds contracts/ with forge and copies the REAL creation bytecode of
# ArbitrageExecutor + FlashLoanExecutor (+ ERC1967Proxy, PR-D1) into the
# committed hex fixtures under backend/sim-core/src/fixtures/. The drift test in
# backend/sim-core/src/paper_executors.rs enforces fixture == forge output
# whenever contracts/out exists.
#
# ERC1967Proxy (PR-D1, 2026-10-18): the paper stack deploys each executor
# BEHIND an ERC1967Proxy whose constructor delegatecalls `initialize`, because
# the implementation's own constructor calls `_disableInitializers()` and a
# direct `initialize` on the impl reverts with InvalidInitialization
# (0xf92ee8a9). The proxy creation code is therefore a first-class fixture.
#
# Requires forge (NOT available in the VPS rust:1.91 image — run locally).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/contracts"

forge build

python3 - "$repo_root" <<'PY'
import json, pathlib, sys
repo = pathlib.Path(sys.argv[1])
fixtures_dir = repo / "backend" / "sim-core" / "src" / "fixtures"
fixtures_dir.mkdir(parents=True, exist_ok=True)
for name in ("ArbitrageExecutor", "FlashLoanExecutor", "ERC1967Proxy"):
    art_path = repo / "contracts" / "out" / f"{name}.sol" / f"{name}.json"
    with art_path.open() as f:
        artifact = json.load(f)
    code = artifact["bytecode"]["object"]
    if code.startswith("0x"):
        code = code[2:]
    if not code:
        raise SystemExit(f"{art_path}: bytecode.object is empty")
    out = fixtures_dir / f"{name}.bytecode.hex"
    out.write_text("0x" + code)
    print(f"{name}: {len(code)//2} bytes -> {out.relative_to(repo)}")
PY
