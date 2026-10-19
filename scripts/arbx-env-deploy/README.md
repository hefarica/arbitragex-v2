# ArbitrageX `.env` deploy pipeline

Push credentials from `ArbitrageX_Unified_Config.xlsm` (`.env Production` sheet) to the
VPS `.env` **idempotently**, without ever breaking the file's structure.

## Files
| File | Runs where | What it does |
|---|---|---|
| `ArbxEnvDeploy.bas` | Excel (embedded in the `.xlsm`) | `ExportEnvFragment` / `DeployEnvDryRun` / `DeployEnvApply` — launches the floating panel |
| `arbx_deploy_gui.ps1` | your Windows box | **Apple-style floating panel** (QuantumX dark). Runs the deploy, auto-closes with a confirmation on success; on error shows the log at the bottom. No PowerShell console. |
| `arbx_fastdeploy.sh` | your Windows box | **fast transport: ONE ssh connection** (tar piped over ssh) — `tar … \| ssh host 'untar && remote.sh'` |
| `arbx_remote.sh` | the VPS | remote runner: `dryrun` (diff) / `safediff` (key-name summary) / `apply` (backup + upsert + restart + verify); self-cleans the fragment |
| `arbx_env_upsert.sh` | the VPS | idempotent, structure-preserving, CRLF-safe upsert (**17/17**, `test_upsert.sh`) |
| `arbx_deploy_env.ps1` | your Windows box | CLI fallback (no panel): `-Apply` / dry-run via classic scp+ssh |
| `arbx_env_fragment.env` | generated | transient `KEY=VALUE` export (secret-bearing; shredded on the VPS after run) |
| `gen_rpc_env_from_xlsx.py` | your Windows box | reads the `RPC Providers` + `_RED_lookup` catalog sheets → emits `RPC_HTTP_<chainId>`/`RPC_WS_<chainId>` (multi-provider, keyless public URLs) as a fragment. **The Excel is the EDITOR; the emitted, versioned artifact is the SSOT** (see below). |
| `rpc_env_generated.env` | generated | the RPC fragment the generator writes (gitignored `*.env`); feed it to `arbx_env_upsert.sh`. |

## Use
1. Edit values in the **`.env Production`** sheet (A = variable, B = value). That sheet is
   the single source of truth the macro exports. To add `RPC_HTTP_1`/`RPC_WS_1`, add rows
   there with the constructed value (`alchemy=https://eth-mainnet.g.alchemy.com/v2/<KEY>`).
2. Run macro **`DeployEnvDryRun`** → a PowerShell window shows the exact `diff` the VPS `.env`
   would receive. **Nothing changes.** Review it.
3. If correct, run macro **`DeployEnvApply`** → confirm the warning → it backs up the remote
   `.env`, upserts in place, restarts `searcher-rs`+`api-server`, and prints the boot/scanner logs.

## Generating `RPC_HTTP_*`/`RPC_WS_*` from the catalog (no hand-typing)
Instead of hand-constructing RPC values in the `.env Production` sheet (step 1 above),
generate them from the **`RPC Providers`** (Chain · Protocolo · Proveedor · URL) +
**`_RED_lookup`** (Proveedor → token) catalog sheets — **Excel = EDITOR, one-way, read-only**:

```bash
python scripts/arbx-env-deploy/gen_rpc_env_from_xlsx.py        # -> rpc_env_generated.env (review it)
# apply idempotently to the VPS .env, then recreate the env-consuming services:
scp scripts/arbx-env-deploy/rpc_env_generated.env arbx:/tmp/rpc.env
ssh arbx 'bash /opt/arbitragex-v2/scripts/arbx-env-deploy/arbx_env_upsert.sh /opt/arbitragex-v2/.env /tmp/rpc.env --backup \
  && docker compose -f /opt/arbitragex-v2/docker/compose.prod.yml up -d --force-recreate --no-deps searcher-rs api-server \
  && rm -f /tmp/rpc.env'
```
- Emits per-**mainnet** `RPC_HTTP_<chainId>`/`RPC_WS_<chainId>` in the exact `token=url,token=url`
  format the hot-path consumes (`shared-rs/rpc_failover.rs`); keyless public endpoints only.
  Testnets keep their dedicated `*_RPC_URL` vars. Re-run = byte-identical (idempotent).
- Override the workbook path with `--xlsx <path>` or `ARBX_CONFIG_XLSX`. The fragment is
  gitignored (`*.env`); edit the **catalog** sheets and regenerate — never hand-edit the fragment.

## Source of truth — who owns what

- The **Excel is an EDITOR**. It never is the canonical state: a workbook that
  lives on a Windows box cannot be diffed, reviewed, rolled back or audited, and
  it is how live credentials ended up inside a shared file.
- The **SSOT is the versioned artifact** the editor emits (`rpc_catalog.json` /
  the generated fragment in the repo), and on the host the `RPC_HTTP_<chain>` /
  `RPC_WS_<chain>` keys of `/opt/arbitragex-v2/.env` in the G-RPC-1 format
  (`.env.example:58-73`: `token=url,token=url`, at least TWO providers per chain).
- **Secrets never live in the sheet.** The workbook carries `proveedor` + a URL
  template; the value comes from the secret store and lands in the host `.env`
  only. A sheet with a live key in it is a leak, not a canonical file.
- **Transport is not canon.** Writing over SSH without validate → apply →
  verify → rollback is a hand edit with a nicer UI. `arbx_remote.sh apply` is the
  only writer, and it refuses to run without the guards below.

## Guarantees (proven by `test_upsert.sh` — run it, do not cite it)
Run: `bash scripts/arbx-env-deploy/test_upsert.sh` → `17/17 checks passed`.
The previous "15/15" was an assertion without a file in the tree; the test now
exists and names each check.
- **Idempotent:** running twice with the same fragment → byte-identical `.env`.
- **Structure-preserving:** managed keys replaced *in place* (order kept); comments, blank
  lines, and unmanaged keys are byte-identical; new keys appended at the end.
- **Value-safe:** values are literal strings — multi-`=` RPC values, `/`, `&`, `:` all stored
  verbatim. `#commented` keys are never clobbered.
- **Backup:** every apply writes a timestamped `.env.bak_*` and a
  `.env.rollback_*` copy on the VPS first.
- **Rollback (2026-10-19):** if the recreate fails, or if `searcher-rs` and
  `api-server` are not both `running` after it, the pre-apply bytes are restored
  AND the stack is recreated again. A backup that is never restored is not a
  rollback.
- **Single writer (2026-10-19):** `apply` takes `flock -n` on
  `/tmp/arbx_env_apply.lock`. A second concurrent apply (Excel macro + frontend)
  exits 7 instead of interleaving two fragments into one `.env`.

## Config / safety
- Host alias + path: edit `-VpsHost` (default `arbx`) / `-EnvPath` (default
  `/opt/arbitragex-v2/.env`) in `arbx_deploy_env.ps1` if yours differ.
- **No SSH credentials are stored anywhere here** — transport uses your `~/.ssh/config` host.
- `paper_mode` is never touched; this only injects credentials (capital stays at 0).
- The fragment holds secrets: it is shredded from the VPS `/tmp` after every run; delete the
  local copy when done. Rotate the 4 creds that leaked to the chat transcript earlier.
