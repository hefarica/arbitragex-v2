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
# ONE writer only. Never call arbx_env_upsert.sh directly on /opt: that path has
# no backup, no rollback, no flock and no verify (2026-10-19: the old example
# here did exactly that, so the manual route was unprotected).
scp -p scripts/arbx-env-deploy/rpc_env_generated.env arbx:/tmp/arbx_env_fragment.env
scp -p scripts/arbx-env-deploy/arbx_env_upsert.sh     arbx:/tmp/arbx_env_upsert.sh
scp -p scripts/arbx-env-deploy/arbx_remote.sh         arbx:/tmp/arbx_remote.sh
ssh -o BatchMode=yes arbx 'bash /tmp/arbx_remote.sh dryrun /opt/arbitragex-v2/.env'   # review the diff
ssh -o BatchMode=yes arbx 'bash /tmp/arbx_remote.sh apply  /opt/arbitragex-v2/.env'   # backup+upsert+verify+rollback
```
- **Fragment lifecycle:** it holds secrets on BOTH sides. On the VPS it is
  0600 on arrival (the runner forces `umask 077` + `chmod 600`) and shredded on
  exit; on Windows delete it after a successful apply — it is not a keepsake.
- **Host key:** `BatchMode=yes` prevents prompts, but the key trust comes from
  your `~/.ssh/config` (`StrictHostKeyChecking`), not from these scripts. Keep
  that file reviewed.
- Emits per-**mainnet** `RPC_HTTP_<chainId>`/`RPC_WS_<chainId>` in the exact `token=url,token=url`
  format the hot-path consumes (`shared-rs/rpc_failover.rs`); keyless public endpoints only.
  Testnets keep their dedicated `*_RPC_URL` vars. Re-run = byte-identical (idempotent).
- Override the workbook path with `--xlsx <path>` or `ARBX_CONFIG_XLSX`. The fragment is
  gitignored (`*.env`); edit the **catalog** sheets and regenerate — never hand-edit the fragment.

## El flujo real, y quién es dueño de qué

El camino que se usa hoy **no pasa por el repo**: el macro exporta el fragmento
desde el `.xlsm`, PowerShell lo copia por `scp` a `/tmp` del VPS, y
`arbx_remote.sh apply` lo aplica sobre `/opt/arbitragex-v2/.env` y recrea los
servicios. Git aporta los scripts, no los valores. Eso hay que decirlo así, no
como si el repo fuera la fuente de verdad.

Con ese flujo, la propiedad queda:

- **El Excel es el EDITOR.** Nadie más lo edita, y nunca es canónico: no se
  puede diffear, revisar ni revertir.
- **El canónico de los valores es el `.env` del host**, escrito por UN solo
  escritor (`arbx_remote.sh apply`), con copia `.rollback_*` y un recibo
  `.env.apply.log` que anota **fecha + sha256 antes/después + copia de rollback**
  —sin valores y sin secretos—. Ése es el rastro de auditoría, porque git no ve
  este archivo.
- **El repo es dueño de lo que sí puede revisarse:** los scripts, el esquema del
  fragmento, y el catálogo de proveedores **sin claves**. Una hoja con una clave
  viva dentro es una fuga, no un canónico.
- **El transporte no es el canónico.** Escribir por SSH sin validar → aplicar →
  verificar → revertir es una edición a mano con mejor interfaz. `apply` es el
  único escritor y no corre sin sus guardas.

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
