# ArbitrageX v2 — Data Requirements Matrix (per module)

**Companion to:** [NO-HARDCODE-DOCTRINE.md](./NO-HARDCODE-DOCTRINE.md)
**Scope:** Every module that would act on a real value. Canonical protocol constants (router address catalog) live in code and are not listed here.

Columns:
- **Datum** — the value the module needs.
- **Type** — string / secret / url / address / number / enum / jsonb.
- **Source** — allowed source from the doctrine (1–8).
- **Sensitivity** — public / internal / confidential / secret.
- **Phase** — progressive solicitation phase (1–5).
- **Validation** — how we accept/reject values at intake.
- **Storage** — where it rests.
- **Activation condition** — what the feature needs to turn on.
- **Block if missing** — what the UI/logs say when absent.

Key for Source: `Env`, `Vault`, `DB`, `Form`, `File` (validated config file), `ExtAPI`, `Onboarding`, `UserInput`.

---

## M1 — searcher-rs (mempool detection)

| Datum | Type | Source | Sensitivity | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------------|-------|------------|---------|-----------|------------------|
| `RPC_WS_<chain_id>` | url | Env ← Vault `secret/arbitragex/<env>/rpc/ws_<chain>` | secret | 2 | `wss://` scheme + successful `eth_chainId` probe matches expected `chain_id` | Vault + env at container boot | chain enabled in `configs/app.toml` AND env present | scanner stays in `idle_chain_loop`, gauge `arbx_service_up{...}=1` but no detection; UI `/status` shows chain as "idle, no RPC" |
| `RPC_HTTP_<chain_id>` | url | Env ← Vault | secret | 2 | `https://` + `eth_blockNumber` probe | Vault | RPC-dependent lookups (tx status, receipts) | non-fatal; recon marks tx status as `unknown` |
| `DATABASE_URL` | url (creds) | Env ← Vault | secret | 1 | connection probe at boot | Vault | persistence on | service runs in "publish-only" mode (stream, no DB); `arbx_db_unavailable=1` |
| `REDIS_URL` | url | Env | internal | 1 | connection + PING | `.env` rendered from Vault template | always | fail-fast at boot (required) |

## M2 — sim-ctl (simulation)

| Datum | Type | Source | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------|------------|---------|-----------|------------------|
| `ANVIL_FORK_URL` | url | Env ← Vault (may alias `RPC_HTTP_1`) | 3 | `eth_blockNumber` probe | Vault | sim provider = `anvil` | sim-ctl returns `501 {error:"not_implemented", requires:["ANVIL_FORK_URL"]}` |
| `TENDERLY_PROJECT`, `TENDERLY_API_KEY` | string, secret | Vault | 3 | Tenderly `/auth/info` probe | Vault | sim provider = `tenderly` | provider falls back to `not_implemented` |
| simulation parameters (`fork_block`, `snapshot_pool_size`, `sim_timeout_ms`, `gas_limit_safety_factor`, `max_slippage_for_pass_pct`) | number | File `configs/app.toml` | 3 | JSON Schema `configs/schemas/app.schema.json` | file + reloaded via `SIGHUP` | always | defaults at example values marked non-productive; UI shows "review before S4" |

## M3 — selector-api (scoring + token safety)

| Datum | Type | Source | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------|------------|---------|-----------|------------------|
| `token_safety.provider` | enum(`goplus`/`honeypot_is`/`internal_only`) | File `configs/app.toml` | 3 | schema enum | file | always | defaults to `internal_only` (no external calls) |
| `GOPLUS_API_KEY` | secret | Vault | 3 | provider-specific probe | Vault | provider=`goplus` | 501 from selector when goplus chosen but key missing |
| `HONEYPOT_IS_API_KEY` | secret | Vault | 3 | provider probe | Vault | provider=`honeypot_is` | idem |
| `scoring.weight_*` | number | **DB table** `scoring_weights` (created in Phase 0.5) backed by admin UI | 3 | sum-to-1 constraint | DB (audit-logged) | always | fail-fast boot if row missing; operator must seed via `/config/scoring` page |
| token blacklist | set(address) | Redis `arbx:blacklist:tokens:<chain>` populated via admin UI + CSV import | 3 | EVM address regex | Redis | always | empty set allowed; UI shows "no blacklist configured" |

## M4 — relays-client (execution)

| Datum | Type | Source | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------|------------|---------|-----------|------------------|
| relay catalog | list of `{name, enabled, chains, endpoint, auth_secret_ref}` | **DB table** `relays` (created in Phase 0.5), editable via admin UI | 4 | URL + chain-id valid + auth scheme enum | DB + Vault for secrets | any relay `enabled=true` | relay skipped with `relay.disabled` log; if no relay enabled, relays-client stays idle + `arbx_service_up{service="relays-client"}=1` but `arbx_relays_available=0` |
| `FLASHBOTS_SIGNER_KEY` | secret (priv key) | Vault `secret/arbitragex/<env>/relays/flashbots_signer_key` | 4 | `secp256k1` key parse + derive address + check balance = 0 for phase 4 | Vault | Flashbots enabled | relay error `missing_signer`; UI shows "sign with 0-balance key" CTA |
| `BLOXROUTE_AUTH`, `EDEN_AUTH`, etc. | secret | Vault | 4 | per-relay probe | Vault | relay enabled | relay disabled until set |
| `execution.paper_mode` | bool | File `configs/app.toml` | 4 → 5 flip | schema bool | file | always | **defaults to `true` and stays true through S8**. Flipping to false is an explicit phase-5 operator action with an audit-log entry and a second confirmation. |
| `execution.max_value_eth`, `max_gas_price_gwei`, slippage caps | number | DB `execution_policy` (created Phase 0.5), editable via admin UI | 4 | schema + operator sign-off flag | DB | execution path | fail-closed if row missing |

## M5 — recon (reconciliation + learning)

| Datum | Type | Source | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------|------------|---------|-----------|------------------|
| `recon.pnl_source_default` | enum | File `configs/app.toml` | 3 | schema | file | always | default `native_only` (no oracles) |
| Chainlink/TWAP oracle addresses | address | DB `price_oracles` editable by admin | 3–4 | EVM address + ABI probe | DB | `pnl_source = oracle_*` | recon falls back to `native_only` with warning banner |
| `recon.anomaly_*` thresholds | number | DB `risk_policy` (Phase 0.5) | 4 | bounds check | DB | always | fail-fast on missing row |

## M6 — api-server + edge

| Datum | Type | Source | Phase | Validation | Storage | Activation | Block if missing |
|-------|------|--------|-------|------------|---------|-----------|------------------|
| `ARBX_EDGE_TOKEN` | secret | Vault `tokens/edge` | 1 | length ≥ 32 bytes base64 | Vault | always | fail-fast boot |
| `ARBX_ADMIN_TOKEN` | secret | Vault `tokens/admin` | 1 | length ≥ 32 bytes base64 | Vault | admin endpoints | 401 on admin routes |
| `JWT_SECRET` | secret | Vault `tokens/jwt` | 1 | length ≥ 64 bytes | Vault | frontend auth | frontend `/signin` returns `auth_unavailable` |
| `ALLOWED_ORIGINS` | csv | File `configs/app.toml` + env override | 2 | URL list | file | CORS | defaults to empty (no origins) — UI shows CORS misconfig notice |
| `API_SERVER_URL` (in Worker var) | url | Env var in wrangler `[env.production]` | 4 | URL format | wrangler | production deploy | dev uses local container via compose; prod requires explicit value |

## M7 — edge Worker + Cloudflare Tunnel

| Datum | Type | Source | Phase | Validation | Storage | Activation |
|-------|------|--------|-------|------------|---------|-----------|
| `CF_API_TOKEN`, `CF_ACCOUNT_ID`, `CF_ZONE_ID`, domain | secret, id, id, string | Vault | 4 | CF `/user/tokens/verify` probe | Vault | Worker deploy + Tunnel | deploy step 501s |
| `TUNNEL_TOKEN` | secret | Vault | 4 | CF tunnel API probe | Vault | tunnel up | tunnel container crashloops with explicit `token_missing` error |
| KV namespace id, D1 db id | id | `wrangler.toml` with env placeholder; real id fetched at deploy time via `wrangler kv:namespace create` and written back as an `[env.production]` var by CI | 4 | string non-empty | wrangler + CI env | worker read cache | cache bypass + warning header `x-arbx-cache: disabled` |

## M8 — observability (Prometheus, Grafana, Loki, Alertmanager)

| Datum | Type | Source | Phase | Validation | Storage | Activation |
|-------|------|--------|-------|------------|---------|-----------|
| `GRAFANA_ADMIN_PASSWORD` | secret | Vault | 1 | length ≥ 16 | Vault | Grafana login | Grafana refuses to start |
| `SLACK_WEBHOOK_URL` | url (secret) | Vault | 2 (optional) → 5 (required) | CF probe (HEAD to webhook) | Vault | severity: `warning`/`info` routes | Alertmanager falls back to webhook-log receiver |
| `PAGERDUTY_INTEGRATION_KEY` | secret | Vault | 5 | PagerDuty `/oauth/token` probe | Vault | severity: `critical` | fall back to Slack warning + explicit "no PD configured" banner |

## M9 — backups & storage

| Datum | Type | Source | Phase | Validation | Storage | Activation |
|-------|------|--------|-------|------------|---------|-----------|
| `AGE_RECIPIENT_PUBKEY` | pubkey string | File `configs/backup-recipient.age.pub` (committed — it is a public key) | 5 | age pubkey format | file | encryption | backups remain unencrypted → script refuses to run |
| `B2_APP_KEY_ID`, `B2_APP_KEY`, `B2_BUCKET` | id + secret + string | Vault | 5 | rclone probe | Vault | offsite upload | local backups only; warning emitted daily |
| `DATABASE_READONLY_URL` | url (creds) | Env ← Vault | 1 | probe | Vault | read-only dumps | fall back to RW credentials with audit warning |

## M10 — chain + router + protocol catalog (canonical)

Lives in code **on purpose** (doctrine §"Distinction"):

- Router catalog: `backend/shared-rs/src/chains.rs`.
- Canonical WETH / USDC / … : currently in scattered literals — **to be moved** into `shared-rs/src/tokens.rs` as a catalog in Phase 0.5 remediation.
- Chain RPC method names, ABI fragments — not operator-configurable.

Each catalog entry MUST have a byte-asserting test (example: `univ2_router_mainnet_address_bytes` in chains.rs).

---

## Summary of solicitation UX

The operator console (`/onboarding`) — to be built in Phase 0.5 — walks through phases 1–5 in order:

1. **`/onboarding/1-init`** — generates tokens, captures Vault unseal keys, sets Grafana admin password.
2. **`/onboarding/2-connect`** — captures RPC URLs per chain, optional Slack webhook, test-connects each.
3. **`/onboarding/3-advanced`** — sim provider + key, token-safety provider + key, scoring weights sign-off, initial blacklist CSV import.
4. **`/onboarding/4-testing`** — relay catalog + per-relay auth, Flashbots signer key (0-balance), execution policy sign-off, CF API token + domain + tunnel.
5. **`/onboarding/5-production`** — flips paper-mode off (guarded), PagerDuty key, B2 + age pubkey for backups, incident contacts.

Each step validates on submit, writes to Vault or DB, and the next step is locked until the previous one green-checks.

Re-entering the flow for a configured step shows current values (masked for secrets) and offers rotation.
