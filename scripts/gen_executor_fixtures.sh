#!/usr/bin/env bash
# G-SIM-1 WO-LR22.13 PR-B — regenerate the paper-executor bytecode fixtures.
#
# Rebuilds contracts/ with forge and copies the REAL creation bytecode of
# ArbitrageExecutor + FlashLoanExecutor into the committed hex fixtures under
# backend/sim-core/src/fixtures/. The drift test in
# backend/sim-core/src/paper_executors.rs enforces fixture == forge output
# whenever contracts/out exists.
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
for name in ("ArbitrageExecutor", "FlashLoanExecutor"):
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
