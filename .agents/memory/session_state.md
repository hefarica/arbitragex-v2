# OMEGA CORTEX — Estado de sesión persistido (pre-/compact)

> Generado 2026-05-11 antes de `/compact`. Post-compact, primera acción
> obligatoria: `cat .agents/memory/session_state.md` para recuperar contexto.

## Branch + estado

- **Branch**: `main`
- **Último commit**: `171b621` docs(spec): event-driven multi-strategy orchestrator design (Phase 0)
- **VPS containers**: todos `Up` (api-server 5h, frontend 5h, edge 24h, searcher-rs 25h, postgres 8d healthy)
- **Working tree**: ~12 archivos modificados (mayoría en `backend/searcher-rs/src/*` — refactor en curso por otra sesión / orchestrator design)

## Commits de hoy (cronológico, más reciente arriba)

| Hash | Título | Estado |
|---|---|---|
| `171b621` | docs(spec): event-driven multi-strategy orchestrator design (Phase 0) | committed |
| `741753a` | fix(opps): gate OnChainTruthValidator behind feature flag — RPC quota guard | deployed |
| `e0cb853` | ui(opps): move StrategyBadge to chain-identity line (top-row layout) | deployed |
| `f38cc90` | feat(opps): Token Validation Engine Phase 1 — composite multi-validator safety score | deployed |
| `f1af9e6` | feat(opps): dual-source token verification — Uniswap default + CoinGecko (~9.5K coins) | deployed |
| `3bcaa9e` | fix(opps): always emit verified flag — no token escapes the UNVERIFIED badge | deployed |
| `338bdbc` | feat(opps): UNVERIFIED token badge — defend dashboard from 69420-class memecoins | deployed |
| `0ae9eec` | feat(opps): dual-floor target (USD AND ROI) + ROI-assumed sizing fallback | deployed |
| `1f8fa1e` | feat(opps): target-driven simulation — net column + inverse sizing + base-token badge | deployed |
| `1566133` | feat(opps): on-demand eth_call resolver — every token in feed has a symbol | deployed |
| `6818702` | fix(layout): widen container max-w 7xl→1800px to use widescreen real estate | ✅ deployed |
| `1fce600` | fix(opps): emit token_info when symbol exists even if resolved_via is NULL | deployed |
| `53db656` | feat(opps): DEX path shows BUY/SELL/VIA/TARGET semantics, not just raw names | deployed |
| `1f06bab` | feat(opps): every token shows SYMBOL + address + chain badge | deployed |
| `fab16c6` | fix(credentials): wire missing pieces from f2ed678 (unblocks VPS api-server build) | ✅ **resolved my earlier blocker** |
| `eb01ef7` | fix(opportunities/live): bound window — no more 4-day-old snapshots | ✅ deployed (via fab16c6 unblock) |
| `f2ed678` | feat(credentials): /settings/credentials page wires every external credential | deployed |
| `d577dba` | fix(edge): /strategies 400 invalid_chain_id — proxy double-appended query | deployed |
| `8a27d83` | feat(dex-registry): real chain names + Add/Remove DEX endpoints | deployed |

## Blockers previos: TODOS RESUELTOS

- ❌ ~~api-server build failing on credentials TS errors~~ → **resuelto** por `fab16c6`
- ❌ ~~`eb01ef7` window fix never deployed~~ → **desplegado** tras el unblock
- ✅ Layout container ahora usa `max-w-[1800px]` — `/opportunities` aprovecha pantallas anchas

## Decisiones arquitectónicas tomadas hoy

1. **Token Validation Engine** (`f38cc90`): composite multi-validator safety score
   con dual-source verification (Uniswap default list + CoinGecko ~9.5K coins).
   Defiende el dashboard de tokens 69420-class memecoins con badge `UNVERIFIED`.
2. **Dual-floor target** (`0ae9eec`): el operador puede exigir net_profit ≥ X USD
   AND roi ≥ Y%. Fallback de sizing por ROI cuando no hay USD floor.
3. **OnChainTruthValidator feature-flagged** (`741753a`): defensa de cuota RPC.
4. **Layout 1800px** (`6818702`): respuesta a queja del operador sobre márgenes
   desperdiciados en `/opportunities` en monitores 1920px.
5. **Live window 5min** (`eb01ef7`): `/opportunities/live` no surface ya rows
   históricos cuando no hay viables recientes.
6. **Event-driven orchestrator** (`171b621`): nueva spec Phase 0 — multi-strategy
   workflow. Está en `docs/superpowers/specs/` (no leído aún, pendiente revisar).

## Trabajo en progreso (working tree modificado)

- `backend/searcher-rs/src/amm_math.rs`, `calldata/{mod,univ2,univ3}.rs`,
  `chain_client.rs`, `dedup.rs`, `main.rs`, `patterns.rs` — modificados pero
  no committeados. Probablemente WIP de la otra sesión / parte del orchestrator
  Phase 0 spec.
- `backend/searcher-rs/src/scanner.rs` — system reminder indica que fue
  modificado por linter (re-orden de imports). Mi `route_plan` import sigue.

## Sprint / Phase actual

- **Audit follow-up cycle**: ✅ CERRADO (C1-C6 + arrays hardcoded + DEX CRUD + window fix + layout width)
- **Token Validation Engine Phase 1**: ✅ DEPLOYED (`f38cc90` + auxiliares)
- **Event-driven multi-strategy orchestrator Phase 0**: 📋 SPEC committed (`171b621`), implementación pendiente

## Bugs conocidos activos

- Ninguno crítico abierto post-compactación. Working tree con cambios sin
  commit en searcher-rs sugiere WIP de otra sesión (no investigado).

## Próximo paso

El último mensaje del operador antes del `/compact` era confirmación del fix
de layout (1800px). No hay tarea pendiente solicitada explícitamente. Si la
próxima sesión retoma:
- Revisar `docs/superpowers/specs/` para localizar la spec del orchestrator (`171b621`)
- Verificar que `eb01ef7` window fix funciona en producción:
  ```bash
  ssh arbx "curl -s 'http://localhost:8787/api/opportunities/live?viable_only=true&limit=5' | python3 -c 'import sys,json; d=json.load(sys.stdin); print(\"count:\", d.get(\"count\"), \"window:\", d.get(\"max_age_seconds\"))'"
  # Expected: count: 0 (no recent viables) + window: 300
  ```
- Investigar los archivos modificados en `backend/searcher-rs/src/*` sin commit
  para entender si son WIP intencional o residuos.

## Memoria operativa nueva (este día)

Patrón confirmado de doctrina: **schema drift entre serialización y consumo**
sigue siendo la fuente #1 de bugs silenciosos en el sistema. Esta sesión
añadió 3 capas defensivas:
1. `normalizeStrategyConfigs()` en api-server (re-emite shape canónico siempre)
2. Frontend Zod `enabled_pool_ids` ahora `.nullable().optional()` (tolera missing)
3. Edge proxy two-mode contract (no double-append cuando path pre-built tiene query)

Lo persistido en `.agents/memory/anti_reincidencia.md` debe complementarse
con: **edge proxy query duplication** (incidente `d577dba`) y **bounded live
window** (incidente `eb01ef7`).
