# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Simulator-v2 Readiness Checklist

**Gate enforced by**: `backend/relays-client/src/main.rs` (SECURE_BOOT, audit A2, 2026-05-10)

`relays-client` refuses to boot with `paper_mode=false` unless `ARBX_SIMULATOR_V2_READY=true`
is set in the environment. This env var is the operator's written acknowledgement that every
item below has been verified. Do not set it speculatively.

## Pre-conditions

Before setting `ARBX_SIMULATOR_V2_READY=true` in any environment, verify all of the following:

- [ ] `simulator_v2::SimulatorV2::simulate()` returns `Ok(SimResult{..})` for valid inputs,
      not `Err(SimError::NotImplemented)`. Confirm by running the simulator-v2 unit tests:
      `cargo test -p simulator-v2`.

- [ ] Tasks 4.2 (`lazy_db`) and 4.3 (`revm_runner`) are merged to `main` and passing CI.
      Check commit history and the Sprint 4 tracking issue before proceeding.

- [ ] Fork-test suite passes with simulator-v2 against a recent Ethereum mainnet block.
      Reference: `backend/simulator-v2/tests/fork_mainnet.rs` (the `#[ignore]`d fork suite).
      Run with `RPC_HTTP_1=<mainnet_archive_rpc> [FORK_BLOCK=<recent_block>]
      cargo test -p simulator-v2 --test fork_mainnet -- --ignored --nocapture`.
      Automated evidence producer: `.github/workflows/sim-fork-evidence.yml` (see below).

- [ ] Variance between simulator-v2 predicted net profit and observed on-chain execution
      profit is below 5% across a sample of at least 100 historical opportunities.
      Document the benchmark run (block range, sample count, mean/max variance) in a
      comment on the Sprint 4 tracking issue before setting the env var.

- [ ] The revm version in `backend/Cargo.toml` matches the alloy workspace version
      (shared `alloy-primitives` crate). Run `cargo tree -d` and confirm no duplicate
      `alloy-primitives` versions appear in the dependency graph.

- [ ] `eth_callBundle` integration with the Flashbots simulate-bundle endpoint has been
      verified in staging. Confirm that `SubmitEngine::execute()` calls
      `FlashbotsClient::eth_call_bundle()` before signing and that the response
      `bundleGasPrice` is within configured limits.

- [ ] A second engineer has reviewed the simulator-v2 implementation (PR review or
      equivalent) and explicitly signed off on numerical correctness of profit calculation
      (no integer overflow, correct wei/gwei unit handling, fees subtracted, not added).

## Evidence producers

The checklist above is backed by the `readiness_evidence` registry (G-SIM-1 FASE 2):
`POST /admin/readiness-evidence` (admin-gated via the `x-arbx-admin-token` header) upserts
one row per `item_key` with provenance (`evidence_ref` URL + `verified_by`); rows older than
30 days count as NOT fresh (strict), so the evidence must be RE-produced, never set once and
forgotten. Inspect with `GET /admin/readiness-evidence?gate_id=G-SIM-1`.

### Automated producers (CI)

| item_key | Producer | Trigger |
|----------|----------|---------|
| `unit_tests` | `.github/workflows/sim-evidence-unit-tests.yml` — job `unit-tests` runs the FULL package (`cargo test -p simulator-v2 --locked`, lib + `tests/` integration, closing the F0 `--lib`-only gap) | push to `main` touching `backend/simulator-v2/**` + manual `workflow_dispatch` |
| `dep_tree` | `.github/workflows/sim-evidence-unit-tests.yml` — job `dep-tree` runs `cargo tree -d --locked` and greps for duplicates of `alloy-primitives` / `revm` | same |
| `fork_suite` | `.github/workflows/sim-fork-evidence.yml` runs the ignored `backend/simulator-v2/tests/fork_mainnet.rs` suite (anti-hollow guards: `FORK_SUITE_OUTCOME=PASS` marker + ≥1 passing libtest line required before any POST) | manual `workflow_dispatch` (optional `fork_block` input; default = latest via `eth_blockNumber`) |
| `eth_callbundle_staging` | `.github/workflows/sim-staging-callbundle.yml` runs the ignored LIVE test `staging_callbundle_against_flashbots_simulate_endpoint` (`backend/relays-client/src/relay_flashbots.rs`): a REAL `eth_callBundle` round-trip with an EPHEMERAL throwaway signer + zero-value/zero-gas self-transfer probe (simulate-only, NO broadcast); asserts parsed response, `totalGasUsed > 0`, no tx error/revert and derived bundleGasPrice within `ARBX_STAGING_MAX_BUNDLE_GAS_PRICE_GWEI` (default 500) | manual `workflow_dispatch` |
| `variance_benchmark` | `scripts/gsim1_variance_benchmark.sh` (VPS operator macro): exports REAL recent opportunities (`scripts/gsim1_variance_export.sql`) and replays each through the PRODUCTION multi-step REVM path at block B (detection block, resolved by timestamp bisection — PREDICTED) and B+1 (settled — OBSERVED) via `backend/sim-core/tests/variance_benchmark.rs`; PASS requires ≥ `VARIANCE_MIN_SAMPLES` (100) labeled pairs AND mean absolute drift < `VARIANCE_MAX_MEAN_DRIFT_PCT` (5%). Method recorded verbatim (`method: revm_b_vs_revm_b1_fork`); every skip is counted, never imputed | manual, ON the VPS (`bash scripts/gsim1_variance_benchmark.sh`) |

Registry transport: every CI producer POSTs through
`.github/actions/post-readiness-evidence` — direct URL when
`ARBX_READINESS_EVIDENCE_URL` + `ARBX_ADMIN_TOKEN` secrets exist, else over the
`VPS_SSH_*` secrets (the POST runs ON the VPS against `127.0.0.1:8080`; the
admin token is read from the deployment `.env` and never leaves the host), else
warn + skip.

Honesty notes: `dep_tree` posts `status:"failed"` with the duplicate version list while the
dedup is unresolved (today `alloy-primitives` resolves 0.4.2 / 0.7.7 / 1.6.0 —
pulled respectively by revm 3.5, simulator-v2 and alloy 1.8; unifying them is a
major dependency migration, tracked separately) — the item
stays effectively pending until the dedup lands; it never fakes a clean tree.

Related observability fix (2026-08-17): the sim-ctl Redis consumer now counts
every consumer-path simulation in `arbx_simulation_total` (labels
`simulator`/`passed`). Before, only the `/simulate` HTTP REVM path incremented
the counter — the anvil/eth_call path the consumer drives is the one with real
24h flow, so G-SIM-1's layer-3 check saw zero flow while simulations were
actually running.

### One-time operator provisioning (required for the automated producers)

1. Repo secret `ARBX_READINESS_EVIDENCE_URL` — the FULL POST URL of the registry endpoint
   (e.g. `https://<host>/admin/readiness-evidence`). OPTIONAL since 2026-08-17: when this
   secret is absent, the shared action falls back to the `VPS_SSH_*` repo secrets and POSTs
   from the VPS itself (registry stays internal-only; the admin token never leaves the host).
2. Repo secret `ARBX_ADMIN_TOKEN` — the api-server admin token (same value the other
   `/admin/*` routes use). Only needed for the direct-URL transport.
3. For `sim-fork-evidence.yml` / `sim-staging-callbundle.yml`: a mainnet RPC secret
   (`ALCHEMY_HTTP_URL` preferred, falling back to `RPC_HTTP_1_ARCHIVE` then `RPC_HTTP_1`) —
   single bare URL.

The registry endpoint is **internal-only by default** (api-server `/admin/*` on
`127.0.0.1:8080`, behind the admin token). The two supported CI transports are the
direct token-guarded public route (operator's choice to expose) and the SSH-tunnel
transport above — which needs no new public exposure.

### Manual item procedures

Items 2, 6 and 7 (and 4 until a benchmark producer lands) are recorded by hand with the
admin token. `status` is `evidenced` or `failed`; `verified_by` uses `operator:<id>` or
`reviewer:<id>`.

**Item 2 — `modules_merged`** (F0 SHAs on `main`):

```bash
export ARBX_READINESS_EVIDENCE_URL="https://<host>/admin/readiness-evidence"
export ARBX_ADMIN_TOKEN="<admin-token>"

curl --fail-with-body -sS --max-time 20 -X POST "$ARBX_READINESS_EVIDENCE_URL" \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{
        "gate_id": "G-SIM-1",
        "item_key": "modules_merged",
        "status": "evidenced",
        "evidence_ref": "F0 merges on main: lazy_db 00352836, revm_runner 74cfbf48, validators fix f2241c16",
        "detail": { "lazy_db": "00352836", "revm_runner": "74cfbf48", "validators_fix": "f2241c16" },
        "verified_by": "operator:<id>"
      }'
```

**Items 6–7 — `eth_callbundle_staging` (operator staging check) and `second_signoff`
(second-engineer review)**: same POST shape, fill the `<...>` placeholders:

```bash
# Item 6 — operator verified eth_callBundle against the Flashbots simulate-bundle
# endpoint in staging (bundleGasPrice within configured limits):
curl --fail-with-body -sS --max-time 20 -X POST "$ARBX_READINESS_EVIDENCE_URL" \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{
        "gate_id": "G-SIM-1",
        "item_key": "eth_callbundle_staging",
        "status": "evidenced",
        "evidence_ref": "<staging run log / ticket reference, with date>",
        "detail": { "bundle_gas_price_within_limits": true, "staging_ref": "<ref>" },
        "verified_by": "operator:<id>"
      }'

# Item 7 — second engineer signed off on numerical correctness (no integer
# overflow, correct wei/gwei unit handling, fees subtracted, not added):
curl --fail-with-body -sS --max-time 20 -X POST "$ARBX_READINESS_EVIDENCE_URL" \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{
        "gate_id": "G-SIM-1",
        "item_key": "second_signoff",
        "status": "evidenced",
        "evidence_ref": "<PR review / signoff record>",
        "detail": { "scope": "profit-calculation numerical correctness", "signoff_ref": "<ref>" },
        "verified_by": "reviewer:<id>"
      }'
```

## Setting the flag

After all items above are checked, set in the deployment environment (`.env` on VPS,
Vault secret, or Docker Compose env block):

```
ARBX_SIMULATOR_V2_READY=true
```

Restart `relays-client`. Confirm the boot log contains:

```
event="secure_boot.sim_v2_gate_passed" paper_mode=false AND ARBX_SIMULATOR_V2_READY=true
```

If the log line is absent, the guard did not fire â€” inspect why `paper_mode` is still `true`
(check Redis key `arbx:papermode` and `configs/app.toml` `[execution] paper_mode`).

## Reverting to paper mode

If anomalies appear after going live, revert immediately:

```bash
# Via Redis (takes effect on next is_enabled() poll, TTL 1s):
docker exec redis redis-cli SET arbx:papermode '{"enabled":true,"updated_at":"<ISO8601>","updated_by":"operator"}'

# Restart relays-client to re-arm the SECURE_BOOT guard:
docker compose restart relays-client
```

## Audit trail

| Date | Operator | Action | Notes |
|------|----------|--------|-------|
| 2026-05-10 | audit A2 | Guard created | simulator-v2 returns NotImplemented; gate armed |

