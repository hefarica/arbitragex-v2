# ArbitrageX v2 — Operational Runbook (the hard-won knowledge)

Everything below was learned by operating the live system. Read it before touching prod. Convert
relative dates to absolute when reasoning; verify any file/flag still exists before relying on it.

---

## 1. Architecture & infrastructure

- **Stack (Option-C hybrid)**: Rust hot-path (`searcher-rs`, `sim-ctl`, `recon`, `relays-client`,
  `token-enricher`, `selector-api`, the `prioritization-spine` crate) · TypeScript control-plane
  (`api-server`, Express, dual-path `/api` + `/api/v1`) · Cloudflare Workers edge (`edge`) ·
  Next.js frontend (`frontend`, QuantumX dark theme).
- **VPS**: `<VPS_IP>` (Hetzner) = ssh alias **`arbx`** (native Windows OpenSSH at
  `C:\Windows\System32\OpenSSH`). Deploy path `/opt/arbitragex-v2` = a git checkout of `main`.
  Stack runs via `docker compose -f docker/compose.prod.yml` (21 containers).
- **Repos / remotes**:
  - GitHub: `github.com/hefarica/arbitragex-v2` (the VPS `origin` = `git@github.com:hefarica/...`,
    the VPS can fetch from it). Local working repo `C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full`
    has remotes `github` (GitHub) and `origin` (a VPS bare repo `arbx-git:/opt/git/...`).
  - Use the `github` remote for GitHub ops from the local repo, `gh` CLI for PR/CI.
- **21 services** (all should be `running healthy`): alertmanager, anvil, api-server, edge, frontend,
  grafana, loki, minio, postgres, prometheus, promtail, recon, redis, relays-client, searcher-rs,
  selector-api, sim-ctl, thanos-query, thanos-sidecar, token-enricher, vault.
- **Pipeline**: detection (`searcher-rs` mempool scanner) → price enrich (`token-enricher` writes
  DexScreener prices to Redis hash `arbx:token_prices:1`) → scoring (advisory) → dex_engine +
  size_optimizer → **emit a paper row ONLY when `rejection_reason=None`** → sim (anvil fork via
  `sim-ctl`). DB: postgres/`arbitragex`. Migration table is NOT `_sqlx_migrations` (migrations are
  applied out-of-band, not via sqlx-migrate-on-boot — a rebuild does NOT auto-run new migrations).

---

## 2. Paper-shadow truth (READ THIS before "fixing" detection)

The 2026-06-14 backend audit doc ("Brechas Paper-Shadow") is **mostly OUTDATED**. Verified-live truth:

**Already done / not blockers:**
- RPC (3 providers), Anvil sim is REAL (`fork_ready`, eth_call/snapshot/revert, not a stub),
  DexScreener oracle WORKS (the old `no_price_oracle x83` snapshot was STALE), JWT/tokens real,
  `paper_mode=true` correctly parked, all 21 healthy.
- **`scoring_weights=0` does NOT block paper emission.** No Rust/TS hot-path reads `FROM
  scoring_weights`; scoring is advisory (`ARBX_SCORING_ENABLED` defaults true, `HARD_GATE` defaults
  false, `should_emit_downstream()` never blocks); `strategy_scores` empty → fail-honest proxy
  fallback. Seeding it (ONBOARDING §3b) is hygiene only — do NOT chase it expecting paper trades.

**The REAL reason it detects millions of opps but emits 0 paper trades:** the emitter only writes a
row when `rejection_reason=None`, and ~0 candidates reach that. Live rejection mix:
- ~57% `no_price_oracle` = COVERAGE CEILING (only ~224/3238 tokens priced/cycle; a pair needs BOTH).
  Mitigate with a second price source (GeckoTerminal — but free tier is hard rate-limited, see creds).
- ~39% `non_positive_profit` + `spot_product_le_one` = HONEST "no real arbitrage at this size". DO
  NOT force these. **0 paper trades is then the CORRECT fail-honest market reality, not a bug.**
- `cap_clamp_failed` = was a real bug (below).

**THE bug that was fixed (2026-06-16) — tokens.decimals INT2/INT4 drift:** the live DB column was
`integer` (INT4) but ALL Rust code is `i16`/SMALLINT (reader `sim_encoder_pg.rs:118` decodes
`Option<i16>`; writers bind `as i16`). The strict sqlx READ failed →
`pg_decimals_provider.refresh_failed` every 60s → starved V3 decimals → `v3_spot_price_from_sqrt`
garbage → phantom ~$9M profit → `cap_clamp_failed`. **Fix: `ALTER TABLE tokens ALTER COLUMN decimals
TYPE smallint;`** (3240 rows, 0..36, idempotent). Captured in **migration 098**
(`098_tokens_decimals_smallint.sql`, idempotent DO-block). Verified: `refresh_failed=0`,
`cap_clamp_failed` fell 116→24. All `decimals as i32` in code are arithmetic casts (`10f64.powi(d as
i32)`) on already-loaded u8 — NOT DB decodes, so the ALTER is safe.

**Levers enabled (paper-safe):** `ARBX_GECKOTERMINAL_ORACLE=active` (rate-limited free → ~0 gain),
`ARBX_POOL_ENUM_MODE=shadow` (pool discovery worker spawns). Onboarding seeds applied: `scoring_weights`
(weights sum=1.0, min_accept=55) + `execution_policy` (`paper_mode=TRUE, private_only=TRUE,
max_value_eth=0.0` fail-closed) — from ONBOARDING §3b/§3c verbatim, paper-safe, no emission change.

---

## 3. CODE deploy — via CI/CD (`deploy-vps.yml`)

- **Canonical path. Manual only** (`workflow_dispatch`; the `push:[main]` auto-trigger was REMOVED
  2026-06-06 — "no deploy without explicit operator OK"). Trigger:
  `gh workflow run deploy-vps.yml --repo hefarica/arbitragex-v2 -f reason="..."`.
- **What it does on the VPS**: `git fetch origin main` → `git reset --hard origin/main` →
  `docker compose -f docker/compose.prod.yml pull` → `up -d --build --remove-orphans` → in-VPS
  healthcheck `curl localhost:8080/health` (15×6s). Full Rust `--release` rebuild on cold cache can
  hit ~45m; warm ~10-16m. `git reset --hard` keeps UNTRACKED files (operator WIP survives); `.env`
  is gitignored so secrets/oracles survive; the DB (decimals) survives.
- **Merge gate**: PR auto-merge is DISABLED on the repo → wait for CI green, then
  `gh pr merge <N> --merge`. CI must be 100% green first (see CI gotchas).
- **A separate mechanism advances the VPS git ref to main without rebuilding** (HEAD moves but images
  stay old). So "git is at main" ≠ "deployed". ALWAYS verify image build times + a live endpoint
  after deploy, not just HEAD.
- **Post-deploy verification (always do)**: image `CreatedSince` ~minutes ago; 21/21 healthy; a real
  endpoint returns 200 (e.g. `GET http://localhost:8080/api/v1/metrics/paper-shadow → 200`); decimals
  still `smallint` + `refresh_failed=0`; oracles + `paper_mode=true` preserved; relays still gated.
- **`docker compose restart` vs `up -d --force-recreate`**: `restart` keeps the container's
  ORIGINAL create-time env → `.env` changes never reach the process. Use `up -d --force-recreate
  --no-deps <svc>` to load new env. (The local deploy-pipeline `arbx_remote.sh` was fixed from
  `restart` → `up -d --force-recreate`.)

---

## 4. SECRETS deploy — via the Excel `.env Production` macro

- **Workbook**: `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm` (13 sheets incl `RPC
  Providers` = catalog of public RPC endpoints per chain, `_RED_lookup`, `.env Production`). The
  `.env Production` sheet: col A=Variable, B=Valor, C=Notas; header row 2, data row 3+; ListObject
  `tbl_env_production`. It holds the full requested set (~127+ rows) with per-key status in col C.
- **Pipeline dir**: `C:\Users\HFRC\Downloads\arbx-env-deploy\`. Key macro
  (`ArbxEnvDeploy.bas`) sub: **`RunFullSyncCycle`** = the one-click cascade: PULL (VPS authority:
  `arbx_cred_manifest.sh` reconciles `.env.example` ∪ `.env` → TSV `KEY⇥VALUE⇥NOTES` with status
  ok/FALTA-LLENAR/REQUERIDA/REEMPLAZAR) → review → PUSH (idempotent CRLF-safe upsert + recreate +
  verify). Gaps preserve operator's local fill; VPS value wins when present. Temp files shredded;
  secrets never printed. Transport = native OpenSSH (`arbx`).
- The macro requires Excel CLOSED to inject/write (COM exclusive; `GetActiveObject` fails
  cross-session with `MK_E_UNAVAILABLE`).

---

## 5. Credential-sourcing map (web-verified 2026-06-17)

Mark **[LOCAL]** = generate yourself, no account · **[PROVIDER]** = account/dashboard.

**[LOCAL] (free, generated with `cast wallet new` at `C:\Users\HFRC\.foundry\bin\cast.exe`):**
- `FLASHBOTS_SIGNER_KEY` — signs the X-Flashbots-Signature header only, holds NO funds. relay
  `https://relay.flashbots.net`. Live-only.
- `EDEN_AUTH` — also a `cast wallet new` SIGNING key (NOT an API key). `api.edennetwork.io/v1/bundle`.
- `CRUCIBLE_DEPLOYER_KEY` — throwaway testnet key; fund its address at an Arbitrum-Sepolia faucet. #18.
- `DEPLOYER_KEY` (mainnet) — do NOT generate a raw key; use **Ledger/Trezor or KMS** (`forge script
  --ledger --broadcast`), ~0.05–0.1 ETH gas. Live-only.
- `HONEYPOT_IS_API_KEY` — **keyless today → leave EMPTY**. `api.honeypot.is/v2/IsHoneypot`.
- Testnet RPCs are PUBLIC/no-key: `ARBITRUM_SEPOLIA_RPC_URL=https://sepolia-rollup.arbitrum.io/rpc`
  (421614). **HOLESKY is DEPRECATED** (EF shutdown 2025-09-01) → use
  `SEPOLIA_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com` (11155111).

**[PROVIDER]:**
- `BLOXROUTE_AUTH` — `portal.bloxroute.com` (Account Details → Authorization Header). **DE PAGO**
  (~$1250+/mo for MEV bundles). The only really-blocking paid one. Live-only.
- `TENDERLY_*` — `dashboard.tenderly.co`. **PAID** (Free gives NO API). Needs THREE vars:
  `TENDERLY_API_KEY` (X-Access-Key) + `TENDERLY_PROJECT` + **`TENDERLY_ACCOUNT`**. Optional (anvil
  already covers sim).
- `GOPLUS_API_KEY` — `console.gopluslabs.io`. It is an **App Key + App Secret** pair → also
  `GOPLUS_APP_SECRET`. Free tier 150K/mo. Bearer token = `sha1(appkey+time+secret)`.
- `COINGECKO_API_KEY` — optional; public GeckoTerminal is free (~30 req/min, hard cap); Analyst plan
  $129/mo to raise. Header `x-cg-pro-api-key`, format `CG-...`.

**The 7 live-secrets still to fill** (the FALTA-LLENAR set): `TENDERLY_PROJECT`, `TENDERLY_API_KEY`,
`FLASHBOTS_SIGNER_KEY`, `BLOXROUTE_AUTH`, `EDEN_AUTH`, `GOPLUS_API_KEY`, `HONEYPOT_IS_API_KEY`. As of
2026-06-17 the 3 LOCAL ones (Flashbots/Eden/Crucible) were generated into the Excel (private keys in
col B, addresses in col C notes); the Crucible address must be funded by the operator.

---

## 6. Windows / SSH / Excel / PowerShell / CI gotchas (these WILL bite)

- **SSH discipline**: do NOT fan out many parallel SSH agents at the VPS — it trips
  fail2ban/sshd-MaxStartups and every connection then times out (a verification workflow once ran
  ~21h retrying). Batch reads into ONE session; serialize; back off on `255`.
- **Use native Windows OpenSSH** (`C:\Windows\System32\OpenSSH\ssh.exe`/`scp.exe`) — it runs headless.
  MSYS/Git-bash ssh hangs without a console. For any non-trivial remote command, **write a `.sh`,
  `scp` it, run `bash /tmp/x.sh`** — inline commands with nested quotes/process-substitution break
  through the PowerShell→ssh→remote-shell chain.
- **PowerShell 5.1 native-stderr-as-fatal**: when a native exe (ssh.exe) writes to stderr under
  `$ErrorActionPreference='Stop'`, PS turns each line into a terminating `NativeCommandError` → FALSE
  failure even on exit 0 (a benign `docker compose` "GITHUB_TOKEN not set" warning caused a phantom
  `EXIT=99`). **Judge success by `$LASTEXITCODE`, never `$?`/stderr.** Set `Continue` around the call.
- **PowerShell 5.1 reads `.ps1` files as ANSI** (no BOM) → non-ASCII chars (em-dash `—`, `·`, `→`)
  cause parser errors. **Keep `.ps1` ASCII-only.** For data with accents (Spanish notes), write a
  UTF-8 JSON file and read it with `[IO.File]::ReadAllText($p,[Text.Encoding]::UTF8) | ConvertFrom-Json`.
- **Python on Windows console = cp1252** → scripts that `print` emoji crash (`UnicodeEncodeError`).
  Run with `PYTHONIOENCODING=utf-8`.
- **Excel COM safe-mode** (iptv-excel-safe-mode): Excel must be CLOSED (exclusive open); backup
  before save; kill `~$*.xlsm` locks; `DisplayAlerts/EnableEvents=$false`; ReadOnly guard; Save then
  `Close($false)`; release every COM object + double `[GC]::Collect()`. Reading is safe via
  openpyxl (no lock). `AccessVBOM=1` (HKCU…\Office\16.0\Excel\Security) needed to import VBA.
- **NEVER print secret cell values** when reading the workbook — report key + length/presence + the
  non-secret note only.
- **CI gotchas (PR #172 cleared these)**:
  - Rust CI is `-D warnings`: clippy `manual_clamp` fails the build →
    `x.max(a).min(b)` must be `x.clamp(a,b)`.
  - The e2e `cartridge-integration.spec.ts` suite hits a full-stack `/api/v1/cartridges/:slug/evaluate`
    route that is NOT in committed code → 404 in CI. The skip-guard `test.skip(SKIP_CARTRIDGE_TESTS)`
    (where `SKIP_CARTRIDGE_TESTS = process.env.ARBX_ASSUME_NO_RPC === '1'`) must be on EVERY `describe`
    block, not just the first (the PR#170 follow-up). A clean git merge can still break e2e
    semantically — always re-check CI after merging main into a branch.

---

## 7. Current state (update as it changes)

- **PR #172** ("4 endpoints, Bayes allocator, FE wiring") merged to `main` at **`dcd3cf8`** (2026-06-18)
  and DEPLOYED via `deploy-vps.yml` (verified: images rebuilt, 21/21 healthy,
  `GET /api/v1/metrics/paper-shadow → 200`, decimals smallint, oracles preserved, paper_mode true).
  It contains: the 4 paper-shadow endpoints (mounted before `mountStubs` so they shadow the 501
  stubs), the `bayesian_allocator` registration in `prioritization-spine/src/lib.rs` (DEAD-CODE —
  registered but not consumed; consuming it = capital sizing, operator-gated via `cap_usd_ceiling`),
  the anvil config (app.toml `simulation.provider="anvil"` + the anvil compose service), migration 098.
- **Operator-gated remainder** (Claude must NOT do): fill the 7 live-secrets + fund the Crucible
  address; `token_safety → goplus`; enable relays + `FLASHBOTS_SIGNER_KEY`; deploy contracts to
  mainnet (real ETH); Vault unseal (Shamir 3-of-5); Crucible testnet 50 resolutions; the ~7-day
  paper-shadow accumulation; and the **manual `paper_mode=false` flip** (the final go-live act).
