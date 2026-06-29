# Operator Runbook — Paper-Shadow Activation (FASE 1/2/6/7/9)

**Audience:** HFRC (operator) · **Date:** 2026-06-14 · **Companion to:** `feat/code-brechas-paper-shadow`

> These steps require **operator secrets and/or VPS access** and change runtime
> behavior. The agent cannot and must not run them (no-hardcode doctrine; no
> fabricated keys; capital blast radius). The code brechas (4 endpoints + FE
> wiring + Rust allocator) are DONE in the branch; this runbook closes the
> credential/config brechas so the new panels light up with real data.
>
> **INVARIANT:** `configs/app.toml` `paper_mode = true` stays `true` until a formal,
> manual GO-LIVE. Nothing here flips it.

---

## FASE 1 — Credentials & infra (P0, blocks everything)

### 1.0 Secure injection — secrets spreadsheet → VPS `.env` (values never leave your VPS)

> **GOLDEN RULE.** Secret values travel: **spreadsheet → your eyes → the VPS `.env` editor.**
> Never paste a value into a chat/agent session or a shell command that echoes it
> (`echo $VAR`) — it lands in bash history and the session transcript. The agent never
> sees the values; it only supplies the canonical variable names below. Verify by
> **presence, never by value** (`grep -c`, never `cat`). Keep `.env` gitignored + `chmod 600`.
> This file is committed with **placeholders only** — never commit real values
> (`arbx-no-hardcode-doctrine`; `SECURE_BOOT` rejects `REPLACE_ME`/placeholder tokens at boot).

Map your spreadsheet columns to these canonical env var names (the literals the code
actually consumes — verified in `backend/api-server/src/index.ts`, the readiness verifiers,
and `searcher-rs`/`sim-ctl`):

```bash
# ── Block A — Detection (lights up scanner.subscribed; P0) ──────────────────
# Format is EXACT: provider=url,provider=url  (≥2 providers/chain — doctrine G-RPC-1)
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<TU_KEY_ALCHEMY>,infura=https://mainnet.infura.io/v3/<TU_KEY_INFURA>
RPC_WS_1=alchemy=wss://eth-mainnet.g.alchemy.com/v2/<TU_KEY_ALCHEMY>,infura=wss://mainnet.infura.io/ws/v3/<TU_KEY_INFURA>
ARBX_DEXSCREENER_ORACLE=active            # free, no key

# ── Block B — Security tokens (SECURE_BOOT validates; GENERATE, don't copy a placeholder) ──
ARBX_EDGE_TOKEN=<openssl rand -base64 48>
ARBX_ADMIN_TOKEN=<openssl rand -base64 48>
ARBX_SERVICE_TOKEN=<openssl rand -base64 48>
JWT_SECRET=<openssl rand -base64 64>

# ── Block C — Simulation (ForkValidationPanel → HEALTHY) ────────────────────
ANVIL_FORK_URL=<same URL as the alchemy entry in RPC_HTTP_1>   # Anvil option, $0
# also configs/app.toml → [simulation] provider = "anvil"

# ── Block D — Scoring archiver (PaperShadowPanel / ConfidenceScoringPanel) ──
ARBX_SCORING_ARCHIVER_MODE=on

# ── Block E — LIVE ONLY (NOT for paper-shadow; defer) ───────────────────────
# FLASHBOTS_SIGNER_KEY=<...>  BLOXROUTE_AUTH=<...>  GOPLUS_API_KEY=<...>
# SIM_SIGNER_ADDRESS=<...>    ARBX_SUBGRAPH_URL_1=<...>  CLOUDFLARE_API_TOKEN=<...>
```

**Procedure (manual paste on the VPS — keeps values off shell history and out of any transcript):**
```bash
ssh arbx                                  # <VPS_IP>
cd /opt/arbitragex-v2                      # adjust to the real path
git check-ignore .env && echo ".env gitignored ✓"   # must print .env
nano .env                                  # PASTE values from the spreadsheet here, by hand
chmod 600 .env
```

**Verification (presence, never value):**
```bash
grep -cE '^(RPC_HTTP_1|RPC_WS_1)=' .env                 # → 2
grep -c 'alchemy\|infura' .env                          # → > 0
grep -E 'REPLACE_ME|<.*KEY.*>|placeholder|TU_KEY' .env  # → EMPTY (no placeholders left)
```
Then proceed to 1.1's restart + log check.

### 1.1 Inject RPCs (S2 — detection)
On the VPS `.env` (`ssh arbx` → `/opt/arbitragex-v2/.env` or compose env):
```bash
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<KEY>,infura=https://mainnet.infura.io/v3/<KEY>
RPC_WS_1=alchemy=wss://eth-mainnet.g.alchemy.com/v2/<KEY>,infura=wss://mainnet.infura.io/ws/v3/<KEY>
```
Doctrine G-RPC-1: **≥2 providers/chain**. Then:
```bash
docker compose -f docker/compose.prod.yml restart searcher-rs
sleep 15
docker logs searcher-rs --tail 30 | grep -E 'scanner.subscribed|scanner.no_rpc|ERROR'
```
**PASS:** log shows `event="scanner.subscribed" chain_id=1`, no RPC errors.

### 1.2 Rotate security tokens if placeholders (S7)
```bash
grep -E 'REPLACE_ME|<run:|placeholder' .env   # must be EMPTY
# if any placeholder, generate and replace:
openssl rand -base64 48   # ARBX_EDGE_TOKEN
openssl rand -base64 48   # ARBX_ADMIN_TOKEN
openssl rand -base64 48   # ARBX_SERVICE_TOKEN
openssl rand -base64 64   # JWT_SECRET
docker compose -f docker/compose.prod.yml restart api-server
docker logs api-server --tail 20 | grep -E 'SECURE_BOOT|boot|error'
```
**PASS:** `assertSecureBootTokens()` does not reject; api-server boots.

### 1.3 Price oracle (free, no key)
```bash
echo 'ARBX_DEXSCREENER_ORACLE=active' >> .env
docker compose -f docker/compose.prod.yml restart token-enricher
docker logs token-enricher --tail 20 | grep -i oracle
```

### 1.4 Simulation provider (S4)
Pick **one**. Both need the same RPC as 1.1.
```toml
# configs/app.toml  →  [simulation]
provider = "anvil"     # or "tenderly"
```
```bash
# Anvil ($0): reuse RPC_HTTP_1
ANVIL_FORK_URL=https://eth-mainnet.g.alchemy.com/v2/<KEY>
# Tenderly: TENDERLY_PROJECT=<slug>  TENDERLY_API_KEY=<key>
docker compose -f docker/compose.prod.yml restart sim-ctl
curl -s localhost:3003/health | jq .
```
**Verifies the new `GET /api/sim-ctl/fork-status` endpoint:** once sim-ctl exposes a
numeric `block_number`, the `ForkValidationPanel` flips from amber DEGRADED to green
HEALTHY automatically (the proxy returns `200` only when block_number is numeric).

---

## FASE 2 — Config flags (env, NOT app.toml)
```bash
# scoring archiver is an ENV VAR, not an app.toml key:
echo 'ARBX_SCORING_ARCHIVER_MODE=on' >> .env
echo 'ARBX_POOL_ENUM_MODE=shadow' >> .env            # optional
docker compose -f docker/compose.prod.yml restart api-server searcher-rs
```
> `SIM_SIGNER_ADDRESS`: the `/api/v1/wallets` endpoint reads the `wallets` DB table,
> NOT this env (the codebase deliberately does not surface the dev sentinel as a
> wallet — R8). To show the signer in `/wallets`, seed a row in `wallets` via PSQL.

---

## FASE 6 — Token safety (S3, live-only blocker)
```bash
GOPLUS_API_KEY=<key>          # gopluslabs.io free tier
```
```toml
# app.toml → [token_safety]
provider = "goplus"            # internal_only is OK for paper, BLOCKING for live
```

## FASE 7 — Relays (S5, live-only)
```toml
# app.toml → [[relays]] flashbots: enabled=true, endpoint="https://relay.flashbots.net"
```
```bash
FLASHBOTS_SIGNER_KEY=<private_key_dedicated_no_funds>
BLOXROUTE_AUTH=<token>
# PAPER must NOT submit: docker logs relays-client | grep -E 'paper_mode_skip_submit'
# NEVER expect "bundle_submitted" in paper.
```

## Service-control endpoints (P2, operator decides plane)
`POST /api/v1/admin/services/:name/start|stop` remain **501 stubs by design** — the
operator chooses the control plane (docker socket vs systemd vs k8s) before wiring.
Until then, use `ssh arbx` + `docker compose ... [up -d|stop] <service>`.

---

## FASE 9 — Smoke test (post-injection)
```bash
# Detection alive
docker logs searcher-rs | grep scanner.subscribed
redis-cli xlen arbx:opportunities:stream
curl -s localhost:8080/api/opportunities/live | jq '.items | length'   # > 0

# New endpoints (this branch)
curl -s localhost:8080/api/metrics/paper-shadow   | jq '.metrics.status'   # ACTIVE/INACTIVE/COMPLETED
curl -s localhost:8080/api/sim-ctl/fork-status    | jq '.metrics.status // .error'
curl -s localhost:8080/api/v1/agents/status       | jq '.agents | length'  # already mounted

# Paper invariant holds
curl -s localhost:8080/api/v1/config/current      | jq '.execution.paper_mode'   # MUST be true
curl -s localhost:8080/api/readiness/decision     | jq '.go_live'                # MUST be false
psql $DATABASE_URL -c "SELECT COUNT(*) FROM paper_trade_runs WHERE created_at > NOW() - INTERVAL '1 hour'"
```

**Estimate:** paper-shadow active → GO-LIVE with real capital = **≥7 days** continuous
green accumulation + S8 criteria (calibration ≥100 scored, Vault, contracts deployed,
token-safety=goplus). None of that is in this branch.
