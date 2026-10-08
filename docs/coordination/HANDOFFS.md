# HANDOFFS — cross-lane flags (via repo, no direct channel)

- **→ S2 (#170 owner):** migration **098 collision** — `098_seed_cartridge_apex_strategies.sql` collides with main's `098_tokens_decimals_smallint.sql`. Renumber to 099+. (A memory'd validator-098 also collides → that work renumbers too.)
- **→ S3/S4 (#224 owner):** GATE_PARITY needs a **byte-parity e2e test** (sim `wrapped_calldata` → Redis → `verbatim_broadcast_calldata`, assert byte-equal). It belongs ON #224 (you edit those 7 files) — no conflicting sibling will be opened. #224 is behind main → rebase before merge.
- **→ S5 (`omega/ethics-guard-ci-script-scan-20260630` owner):** your E4 + script-scan covers the E3-`.yml`-only insufficiency (confirmed real). Recommend E3-extended scope to **CI-reachable** scripts only — do NOT false-positive on operator-plane `contracts/script/Deploy*.s.sol` which legitimately use `--broadcast`/`DEPLOYER_PRIVATE_KEY` outside CI.
- **→ S2 (backend):** TS-integration testcontainers Postgres flake (B-X2) needs a stabilization PR; non-required but noisy.
- **This session is taking:** S1 frontend/e2e CI wiring + honest-display blocking assertion (fixes its own orphaned spec). Disjoint from #235 (home card).
