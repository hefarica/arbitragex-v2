# Secret Provisioning — the two planes (grounded 2026-07-02)

> The core rule: **provision each secret to the plane that CONSUMES it.** Do not try to make one store
> mirror the other. The Excel `.xlsm` macro is the master for the *runtime* plane only — it does NOT feed
> GitHub Actions, and it never should for the deployer signer key.

## The two planes

| Plane | Fed by | Consumed by | Direction |
|---|---|---|---|
| **VPS `/opt/arbitragex-v2/.env`** | `ArbitrageX_Unified_Config.xlsm` macro (`RunFullSyncCycle`, over the operator's SSH) | the running containers (searcher, relays-client, api-server, …) | two-way (PULL + PUSH) |
| **GitHub Actions secrets/vars** | `gh secret set` / `gh variable set` / UI | CI/CD workflows (`deploy-vps`, `m5-sepolia-validation`) | **push-only** |

**Technical asymmetry (why a true two-way with GitHub is impossible):** GitHub's API **never returns a
secret value** (secrets are write-only/encrypted). The Excel macro's PULL step reads values back from the
VPS `.env`; that cannot exist for GitHub. So GitHub secrets are, by construction, a **push-only, purpose-
scoped** plane — never part of the Excel's bidirectional sync.

## Hard rule — signer key isolation

🔴 **`DEPLOYER_PRIVATE_KEY` never goes in the Excel or the VPS `.env`.** It is a signing key. It must be a
fresh, isolated, Sepolia-only wallet (faucet ETH only), stored **only** as an **environment secret on
`sepolia-deploy`** (readable solely by the gated LIVE job that hefarica approves). The VPS containers do
not deploy contracts — only CI does — so the deployer key has no business in the runtime plane.

## M5 pipeline — exact consumption map (`.github/workflows/m5-sepolia-validation.yml` @ main)

| Env var (in workflow) | Source | Sensitivity | State |
|---|---|---|---|
| `SEPOLIA_RPC_URL` | `secrets.SEPOLIA_RPC_URL` | low (public RPC) | ✅ set (repo secret) |
| `AAVE_POOL_ADDRESS` | `vars.AAVE_POOL_SEPOLIA` **‖ public fallback** | public | ✅ set (repo var) |
| `SIM_ORCHESTRATOR_GAS_PRICE_WEI` | `vars.M5_GAS_PRICE_WEI` **‖ 25 gwei fallback** | public | optional (fallback ok) |
| `EXECUTOR_1` | `vars.M5_EXECUTOR_1` **‖ dEaD fallback** | public | optional (fallback ok) |
| `DEPLOYER_PRIVATE_KEY` (dry-run) | hardcoded Anvil test key `0xac09…ff80` | public test key, sim-only | n/a (no real key in dry-run) |
| `DEPLOYER_PRIVATE_KEY` (LIVE) | `secrets.DEPLOYER_PRIVATE_KEY` | **CRITICAL (signer)** | ❌ operator → env secret `sepolia-deploy` |
| `ETHERSCAN_API_KEY` (LIVE) | `secrets.ETHERSCAN_API_KEY` | medium | ❌ operator (or run `verify_contracts=false`) |
| `RPC_HTTP_1_ARCHIVE` (A.4 fork) | `secrets.RPC_HTTP_1_ARCHIVE` | low/medium | ❌ operator (mainnet archive RPC) |

**Note:** `RPC_HTTP_11155111` is a VPS-`.env` runtime var — **M5 does NOT read it** (M5 reads
`SEPOLIA_RPC_URL`). Do not set it as a GitHub secret expecting M5 to use it.

## Provisioning order (operator)
1. ✅ `SEPOLIA_RPC_URL` repo secret — done.
2. ✅ `AAVE_POOL_SEPOLIA` repo variable — done.
3. Run dry-run (`mode=dry_run`) — ✅ passed (run 28568548667).
4. For LIVE: generate isolated Sepolia deployer wallet + faucet ETH → `gh secret set DEPLOYER_PRIVATE_KEY --env sepolia-deploy`; set `ETHERSCAN_API_KEY` (or dispatch `verify_contracts=false`).
5. Dispatch `mode=live`+`confirm_live=DEPLOY-SEPOLIA` → GitHub pauses at `sepolia-deploy` → hefarica approves → broadcast.

## Golden rules
No secrets in chat · no secrets in commits/logs · no signer key in the shared Excel · isolated Sepolia-only
key with faucet ETH only · never reuse a mainnet key.
