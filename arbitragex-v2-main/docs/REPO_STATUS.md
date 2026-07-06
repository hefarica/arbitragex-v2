# Repository Status — ArbitrageX v2

**Last updated:** 2026-05-22T22:42:15Z
**Branch:** main
**Status:** Clean — production-ready baseline

## Cleanup performed on 2026-05-22

- All stale branches removed (41 → 1, only `main` remains)
- All open pull requests closed (22 → 0)
- Repository consolidated to single clean baseline for live deployment

## Current sprint: S2 — Detection real (searcher-rs)

Next steps toward live operations:
- Configure `MAINNET_RPC_URL` secret (Alchemy / Infura WS endpoint)
- Set `paper_mode = false` in `configs/app.toml` after S8 criteria met
- Enable relays in `configs/app.toml` (currently all `enabled = false`)
- Deploy contracts via `contracts/DEPLOY.md`
- Initialize Vault: `bash scripts/vault-operator-init.sh`

## VPS Status (195.201.235.70)

- api-server: ✅ UP (uptime ~28h at last check)
- frontend: ✅ UP (Next.js — QuantumX Control Plane)
- kill-switch: ⚠️ ENABLED (paper_mode active)
- opportunities detected: 0 (searcher-rs needs live RPC)
