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
| `arbx_env_upsert.sh` | the VPS | idempotent, structure-preserving, CRLF-safe upsert (**tested 15/15**) |
| `arbx_deploy_env.ps1` | your Windows box | CLI fallback (no panel): `-Apply` / dry-run via classic scp+ssh |
| `arbx_env_fragment.env` | generated | transient `KEY=VALUE` export (secret-bearing; shredded on the VPS after run) |

## Use
1. Edit values in the **`.env Production`** sheet (A = variable, B = value). That sheet is
   the single source of truth the macro exports. To add `RPC_HTTP_1`/`RPC_WS_1`, add rows
   there with the constructed value (`alchemy=https://eth-mainnet.g.alchemy.com/v2/<KEY>`).
2. Run macro **`DeployEnvDryRun`** → a PowerShell window shows the exact `diff` the VPS `.env`
   would receive. **Nothing changes.** Review it.
3. If correct, run macro **`DeployEnvApply`** → confirm the warning → it backs up the remote
   `.env`, upserts in place, restarts `searcher-rs`+`api-server`, and prints the boot/scanner logs.

## Guarantees (proven by `test_upsert.sh`, 15/15)
- **Idempotent:** running twice with the same fragment → byte-identical `.env`.
- **Structure-preserving:** managed keys replaced *in place* (order kept); comments, blank
  lines, and unmanaged keys are byte-identical; new keys appended at the end.
- **Value-safe:** values are literal strings — multi-`=` RPC values, `/`, `&`, `:` all stored
  verbatim. `#commented` keys are never clobbered.
- **Backup:** every apply writes a timestamped `.env.bak_*` on the VPS first.

## Config / safety
- Host alias + path: edit `-VpsHost` (default `arbx`) / `-EnvPath` (default
  `/opt/arbitragex-v2/.env`) in `arbx_deploy_env.ps1` if yours differ.
- **No SSH credentials are stored anywhere here** — transport uses your `~/.ssh/config` host.
- `paper_mode` is never touched; this only injects credentials (capital stays at 0).
- The fragment holds secrets: it is shredded from the VPS `/tmp` after every run; delete the
  local copy when done. Rotate the 4 creds that leaked to the chat transcript earlier.
