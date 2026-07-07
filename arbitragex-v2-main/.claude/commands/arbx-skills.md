---
description: OMEGA Skill Arsenal — bootstrap + load every ArbitrageX v2-associated skill (doctrinal gates, MEV sources, MEV tooling, ops/SSOT, agents). Shadow/read-only — never activates executor, wallets, capital, or broadcasts.
argument-hint: "[ | gates | full | list ]  (empty = bootstrap+apply-relevant)"
---

# /arbx-skills — OMEGA Skill Arsenal loader

Activa el arsenal de skills de **ArbitrageX v2**. El punto de entrada canónico es
`arbx-cortex-init` (que ya es el ÍNDICE de los 12 gates doctrinales); este command
lo bootstrapea y además expone TODO el catálogo asociado al repo para consulta.

**Modo (de `$ARGUMENTS`):**
- *(vacío)* → **bootstrap**: invoca `arbx-cortex-init`, imprime el catálogo, y luego
  invoca SOLO los gates que aplican a la tarea actual, anunciando cuáles dispararon.
- `gates` → invoca AHORA los 13 skills doctrinales `arbx-*` (bootstrap + 12 gates).
- `full`  → `gates` + todas las fuentes (`arbx-source-*`) + el tooling MEV (`skill_*`).
  ⚠️ Carga pesada: úsalo solo cuando de verdad quieras todo el contexto en memoria.
- `list`  → imprime el catálogo y NO invoca nada (referencia rápida).

## Reglas duras (NO-ACTIVE — inviolables)
- Shadow / read-only / paper. **NUNCA** executor, wallets, claves, capital ni broadcast.
- Prohibido `live:true` / `*_MODE=live`. `arbx:opps:detected` XLEN debe quedar idéntico (delta=0).
- Cero invención: si un skill referido no está en disco, dilo ("no encontrado"), no lo simules.
- Estos slash-commands NO son una autorización de capital (FASE D sigue bloqueada sin KMS + auditoría + autorización humana registrada).

## Paso 0 — Bootstrap obligatorio (siempre)
Invoca el skill **`arbx-cortex-init`** vía la herramienta Skill. Es el índice doctrinal:
re-establece la verdad de sesión (git status/log), carga los 12 gates hermanos, y
reconfirma prohibiciones + solicitud progresiva en 5 fases + formato de entrega de 10 ítems.

## Paso 1 — Catálogo completo (asociado a este repo)

### A) Gates doctrinales — `~/.claude/skills/` (los que importan para seguridad)
| Skill | Dispara cuando |
|---|---|
| `arbx-cortex-init` | inicio de sesión / "ejecuta" / "continúa la fase" (bootstrap + índice) |
| `arbx-pre-edit-audit` | PRIMER Edit/Write de un archivo del repo (hot-path Rust, TS, .sol, >300 líneas) |
| `arbx-no-hardcode-doctrine` | datos productivos como literales (RPC URLs, addresses, claves, umbrales) |
| `arbx-mev-ethics-gate` | sandwich/frontrun/predatorio/manipulación de oráculo |
| `arbx-net-profit-gate` | detección/scoring/decisión de oportunidad sin profit NETO completo |
| `arbx-simulation-mandatory` | ruta de código que termina en broadcast/bundle/firma sin sim en fork |
| `arbx-pre-execute-checklist` | gate FINAL antes de `cast send` / `--broadcast` / submit a relay |
| `arbx-contract-atomicity-rules` | edición de contratos executor/arbitrage/flashloan `.sol` |
| `arbx-flash-loan-discipline` | callbacks/entradas de flash loan (`executeOperation`, `flashLoan(`…) |
| `arbx-rpc-failover-discipline` | cliente RPC / loops calientes `eth_call` / WS subscription |
| `arbx-risk-limits-enforcement` | ejecución sin caps diarios/por-venue/kill-switch |
| `arbx-token-safety-screen` | nuevo allowlist de token / interacción con pool |
| `arbx-paper-trade-first` | promover una estrategia a live sin evidencia fork/paper |

### B) Fuentes canónicas MEV — `~/.claude/skills/arbx-source-*`
`arbx-source-ethereum-mev-docs` · `arbx-source-ethresear-ch` ·
`arbx-source-flashbots-collective` · `arbx-source-flashbots-research` ·
`arbx-source-flashbots-mev-research` · `arbx-source-flashbots-pm` ·
`arbx-source-monad-research`
→ Consulta primaria obligatoria en zona gris de diseño MEV (cita la fuente).

### C) Tooling MEV — `~/.claude/skills/skill_*` (+ `mev-scam-detection`)
`skill_01_artemis_scaffold` · `skill_02_brontes_analyze_range` · `skill_03_suave_kettle_design` ·
`skill_04_mevboost_configure` · `skill_05_relay_operate` · `skill_06_mevshare_publish` ·
`skill_07_inspect_py_run` · `skill_09_reth_embed_node` · `skill_10_revm_simulate` ·
`skill_11_foundry_forktest` · `skill_12_helios_light_verify` · `skill_14_awesome_mev_academic` ·
`skill_16_rusty_sando_study` · `skill_17_mev_template_init` · `skill_18_angstrom_antimev_hook` ·
`skill_19_jito_solana_bundle` · `skill_20_arbv2_solana_arb` · `mev-scam-detection`

### D) Ops / SSOT del repo — `.claude/skills/`
`arbitragex-v2-omni-ssot-operator` · `arbitragex-v2-ops-super` · `arbitragex-v2-ops-en` ·
`arbitragex-v2-ops-es` · `arbitragex-omega-supreme-agent` · `frontend-omni-ssot-analyzer` ·
`agente-resolutivo-total`
(+ índice `.claude/skills/00_INDEX_50_ELITE_SKILLS.md` → `SKILL_001..050` como referencia.)

### E) Política E2E — `git-url-e2e-auditor-scaffold` (CLAUDE.md §32, audit/scaffold/shadow).

### F) Subagentes validadores — `.claude/agents/` (para orquestación OMEGA TEAM)
`rust-mev-engineer` · `solidity-engineer` · `frontend-architect` · `devops-platform` ·
`strategy-architect` · `data-analytics` · `cs-validator` · `math-validator` ·
`economics-validator` · `security-auditor`
→ Matriz: builder + validator (un builder sin validator = sin peer review = inaceptable).

### G) Slash-commands hermanos — `.claude/commands/`
`/cortex-check` · `/audit` · `/status` · `/deploy` · `/fix` · `/omega` · `/team` ·
`/team-parallel` · `/git-url-e2e-auditor-scaffold` · `/agent-{rust,solidity,frontend,devops,strategy,data,cs,math,economics,security}`

## Paso 2 — Aplicación
1. Tras el bootstrap, mapea la tarea actual a los gates de (A) que apliquen e invócalos.
2. En zona gris de diseño MEV, consulta (B) y cita la fuente.
3. Para sim/fork/relay/quoter usa el tooling de (C); para ops/deploy usa (D)/(G).
4. **Anuncia** qué skills dispararon y qué impuso cada uno (formato de entrega de 10 ítems).
5. Si la tarea toca capital/executor/contratos/D → **DETENTE**: requiere los gates de ejecución
   (`arbx-pre-execute-checklist` + `arbx-simulation-mandatory` + autorización humana registrada + KMS).
