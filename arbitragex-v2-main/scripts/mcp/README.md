# MCP secrets bridge — Excel SSOT → `.env.mcp` (no hardcode)

Wires the MCP servers in [`.mcp.json`](../../.mcp.json) to the operator's secrets
**without any hardcoded literal** in the repo. The single source of truth is the
Excel workbook `ArbitrageX_Secrets_Config.xlsx`; `.mcp.json` only ever holds
`${VAR}` placeholders.

```
ArbitrageX_Secrets_Config.xlsx   (SSOT — cells, gitignored)
        │  python scripts/mcp/gen-env-mcp.py
        ▼
.env.mcp                          (DERIVED — gitignored, never hand-edited)
        │  scripts/mcp/run-claude-with-mcp.ps1  (loads env)
        ▼
claude  →  ${VAR} in .mcp.json expands from the process environment
```

## Usage

```powershell
# 1. Generate .env.mcp from the Excel (re-run whenever the Excel changes)
python scripts/mcp/gen-env-mcp.py

# 2a. Verify wiring (loads .env.mcp, runs `claude mcp list`)
powershell -File scripts/mcp/run-claude-with-mcp.ps1

# 2b. Or launch VS Code so the Claude Code extension inherits the env
powershell -File scripts/mcp/run-claude-with-mcp.ps1 -Code
```

> Postgres/Redis live on the VPS bound to the Docker bridge. Open an SSH tunnel
> first (`automation/scripts/print-ssh-tunnel.sh`) so `localhost:5432` / `localhost:6379`
> reach them, or set `PGHOST_RO` / `REDIS_HOST_RO` before running the generator.

## Cell → MCP variable mapping

| MCP `${VAR}` (in `.mcp.json` / user config) | Source in the Excel | Notes |
|---|---|---|
| `DATABASE_URL_READONLY` | `ARBX_RO_PASSWORD` (built as `arbx_ro@host/db`) — or a verbatim `DATABASE_URL_READONLY` row | **read-only role** per §33.2; `server-postgres` is also read-only by design |
| `REDIS_URL_READONLY` | `REDIS_URL` (host → tunnel) — or a verbatim `REDIS_URL_READONLY` row | ⚠ Redis currently has no ACL; see warning below |
| `EVM_RPC_URL` | `ALCHEMY_API_KEY` + `EVM_NETWORK` env — or a verbatim `EVM_RPC_URL` row | network is an operator choice (off by default) |
| `ANVIL_FORK_RPC_URL` | add an `ANVIL_FORK_RPC_URL` row | local Anvil fork; **`PRIVATE_KEY` stays empty** |
| `GITHUB_TOKEN` | add a `GITHUB_TOKEN` row | read-only PAT |
| `MAGIC_21ST_API_KEY` | add a `MAGIC_21ST_API_KEY` row | |
| `CONTEXT7_API_KEY` | add a `CONTEXT7_API_KEY` row | optional — context7 also works via its plugin |

To wire any "add a row" item, add a `Variable | Valor` row to the **`.env Production`**
sheet using that exact variable name, then re-run the generator.

## Doctrine guarantees

- **No hardcode:** `.mcp.json` keeps `${VAR}`; real values exist only in the Excel + `.env.mcp` (both gitignored).
- **Read-only:** Postgres uses the `arbx_ro` role; no `PRIVATE_KEY` is ever emitted (Foundry stays `PRIVATE_KEY=""`).
- **Redis ACL gap:** if `REDIS_URL` has no credentials, the generator warns. `server-redis`
  exposes write tools (`set`/`delete`), so create a read-only ACL user and add a
  `REDIS_URL_READONLY` row to comply with §33.2:
  ```
  ACL SETUSER arbx_ro on >STRONGPASS ~* +@read -@write -@dangerous
  REDIS_URL_READONLY=redis://arbx_ro:STRONGPASS@HOST:PORT
  ```
- **Masked output:** the generator never prints secret values, only lengths/prefixes.
