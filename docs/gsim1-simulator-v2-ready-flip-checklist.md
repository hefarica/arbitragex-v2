# G-SIM-1 — ARBX_SIMULATOR_V2_READY Flip Checklist

> **Operator-only document.** Claude NEVER flips this flag. This checklist
> prepares the evidence the operator needs to decide the flip confidently.
> The flag lives in the VPS `.env`, not in code.

## What the flag gates

`ARBX_SIMULATOR_V2_READY=true` is **Layer 2** of the G-SIM-1 readiness verifier
(`backend/api-server/src/readiness/verifiers/g-sim-1.ts`). When `false`, the
live-flip readiness panel shows G-SIM-1 as **red (HARD BLOCKER)** — the
mandatory-simulation gate is NOT satisfied for any capital flip.

The flag asserts: "simulator-v2 is a REAL fork-backed simulator that validates
the actual broadcast path, not a stub." Flipping it without evidence = lying to
the gate.

## Prerequisites (all must be MERGED on main)

- [x] **#267** — Fase 1+2: OpportunityCandidate contract + RouteMetadata + A1 persist
- [x] **#268** — Fase 3 A2: searcher-rs route API + api-server client
- [x] **#269** — Fase 4 A3: sim-ctl autonomous PG route lookup
- [x] **#270** — Fase 5: frontend route_source selector + sidebar
- [ ] **#271** — B2c: wire `execute_multistep_revm` (REAL multi-step REVM sim)
- [ ] **#272** — Step 6: scanner→emitter route capture wire

## Environment variables the flip DEPENDS on (VPS `.env`)

These must be set BEFORE the flip, or the real-sim path returns 501:

```
SIM_BACKEND=revm                    # selects the REVM backend (not anvil)
REVM_RPC_URL=<ethereum-rpc-url>     # the fork RPC the simulator reads state from
ARBITRAGE_EXECUTOR=<0x-address>     # deployed ArbitrageExecutor proxy (mandatory)
SIM_GAS_LIMIT_PER_STEP=500000       # optional, safe default
SIM_MIN_PROFIT_WEI=0                # optional, default 0
SIM_ROUTE_DEADLINE_SECS=300         # optional, default 300
REDIS_URL=<redis-url>               # for live gas_price_wei read
DATABASE_URL=<postgres-url>         # for A3 route lookup path
ARBX_SIMULATOR_V2_READY=false       # stays false until this checklist passes
```

## E2E validation sequence (operator runs on VPS after deploy)

### 1. sim-ctl health (Layer 1 of G-SIM-1)
```bash
curl -fsS http://localhost:3003/health | jq .
```
Expected: HTTP 200, status `"ok"`.

### 2. Real-sim path responds (B2c wired)
With an enriched OpportunityCandidate (from any of A1/A2/A3 paths), POST to
sim-ctl:
```bash
curl -fsS -X POST http://localhost:3003/simulate \
  -H "content-type: application/json" \
  -d '{"route_source":"simctl_lookup","opportunity_id":"<uuid>"}' | jq .
```
Expected: `passed: true|false` with `gas_used_total > 0`, `gas_price_wei` non-zero,
and `wrapped_calldata` as a `0x...` hex string on pass. If `passed:false` with
`fail_reason` starting with `b2c_`, the path wired but the sim rejected the
route — inspect the reason.

### 3. Simulation metric flowing (Layer 3 of G-SIM-1)
```bash
curl -fsS "http://localhost:9090/api/v1/query?query=sum(increase(arbx_simulation_total[24h]))" | jq .
```
Expected: `result[0].value[1]` > 0 within 24h of the first real sim.

### 4. sim↔broadcast parity check (doctrinal gate)
Before the flip, the operator MUST confirm the `wrapped_calldata` from a passing
simulation is byte-identical to what searcher-rs would broadcast. This is the
simulation-mandatory gate's core invariant. (Layer C harness validates this;
see `[[arbx-layer-c-fork-harness-blueprint]]`.)

## The flip (operator-only, after ALL above pass)

```bash
# On the VPS:
ssh arbx
cd /opt/arbitragex-v2
cp .env .env.bak.$(date -u +%Y%m%d%H%M%S)
sed -i 's/ARBX_SIMULATOR_V2_READY=.*/ARBX_SIMULATOR_V2_READY=true/' .env
# Restart sim-ctl + api-server so they pick up the new env:
docker compose --env-file .env -f docker/compose.prod.yml up -d --force-recreate --no-deps sim-ctl api-server
```

## Post-flip verification

```bash
# G-SIM-1 should now be green (or yellow if market is quiet):
curl -fsS http://localhost:8080/api/v1/readiness/decision | jq '.by_chain."1".blockers'
# The live-readiness panel should no longer list G-SIM-1 as a hard blocker.
```

## Rollback (if validation regresses)

```bash
ssh arbx
cd /opt/arbitragex-v2
cp .env.bak.<timestamp> .env   # restore the false flag
docker compose --env-file .env -f docker/compose.prod.yml up -d --force-recreate --no-deps sim-ctl api-server
```

## What this flag does NOT do

- It does NOT flip `paper_mode`. Paper-mode stays `true` (operator-only,
  separate gate G-PAP-1 requires ≥7d accumulation).
- It does NOT enable live broadcast. `relays-client` live-exec policy has its
  own gates (signer presence, killswitch, etc.).
- It ONLY unblocks the G-SIM-1 hard blocker so the live-flip readiness panel
  can proceed to evaluate the remaining gates.

## Related

- Verifier code: `backend/api-server/src/readiness/verifiers/g-sim-1.ts`
- Real sim wiring: `backend/sim-ctl/src/sim_runner.rs` (PR #271)
- Encoder: `backend/sim-core/src/sim_encoder.rs` (PR #266, merged)
- Doctrine: `arbx-simulation-mandatory` gate
