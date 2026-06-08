# OMEGA CORTEX — REPORTE SESIÓN C (Integrador + Guardián) · 2026-06-06

> Broadcast a las sesiones A (searcher-rs/math-engine) y B (shared-rs/frontend).
> Branch activo: `feat/cartridge-hotpath-shadow`. Remote `github` sincronizado en **`88ed161`**.
> (El histórico de 2026-05-12 [branch main] queda ARCHIVADO más abajo — no borrar.)

## Estado del Integration Gate (`scripts/integration-gate.sh`)
- Steps 1-6 **GREEN**: doctrine · OMEGA SEAL · Clause-34 (no adoptada) · **Rust 122 passed** (`cargo test -p searcher-rs -p shared-rs -p math-engine --lib`) · tsc api+frontend · invariante (echo).
- Step 7 (E2E live) **RED quirúrgico → solo `SHADOWED:/status`**. Matriz: 34 PASS / 1 SHADOWED / 8 RATE_LIMITED / **0 FAIL** (43 rutas).
- La suite live tenía falsos positivos (`/`, `/recon`, `/admin/signin`, `/chains`, `/live-readiness`) por regex amplia / h1 obligatorio / 429 in-page / palabra "mock" en copy anti-mock. **CORREGIDO test-only en `88ed161`** (detección estructural EdgeState role=alert dentro de `<main>`, h1 relajado, 429→RATE_LIMITED, FORBIDDEN→solo "lorem ipsum"). NO se relajó ningún defecto real.

## ACCIÓN REQUERIDA POR EQUIPO
- **B / edge → ÚNICO bloqueador del Step 7:** `/status` está SHADOWED — el edge worker `app.get("/status", …)` ([edge/worker/src/index.ts:290](edge/worker/src/index.ts#L290)) eclipsa la ruta SPA `/status`; deep-link/refresh devuelve JSON. Fix: renombrar la health route a `/api/status` o content-negotiate por `Accept`. Mientras siga, Step 7 = RED (correcto).
- **B / frontend-contract:** el contract-drift de `/pools` (schema `{success,data}` vs API `{count,items}`) **YA SE RESOLVIÓ upstream** — `/api/pools` vivo ahora devuelve `{success,data}` y la página renderiza. Confirmar que `DefiPoolsResponseSchema` quedó alineado en el build desplegado para que no regrese.
- **A / Block 2:** spec listo en [docs/superpowers/specs/2026-06-05-block2-factory-resolution-uuid-drift.md](docs/superpowers/specs/2026-06-05-block2-factory-resolution-uuid-drift.md) (commit `f8057fb`). **P0 obligatorio antes de Block 2:** reconciliar drift UUID↔i64/u64 en `pool_discovery.rs` (get_factories L97 decodifica `f.id` UUID como i64 → factories NUNCA cargan; upsert_pool/upsert_token mismo bug). Luego Block 2 = **RESOLVE on-chain** (leer `pool.factory()` → lookup `factories.id` por (chain,address) → token-safety → hydrate_and_persist; skip+observation si no seeded). Fork BYPASS (`arbx:config:pools`) descartado (canal sin consumidor).
- **A o B / Block 3 IO:** `risk_ledger_io.rs` NO existe aún; `compute_breakers` (math pura, `shared-rs/src/risk_ledger.rs`) está; falta el shell que lea `paper_trade_runs` y publique snapshot a `arbx:risk:breakers`.

## Gates de merge→main (sin cambios — el operador es el único driver)
merge→main + deploy = (A Block 2 verde) + (B Block 3 IO verde) + **fix `/status` (B/edge)** + disparo del operador. `opps:detected` invariante intacto (no se deployó nada). WIP ajeno en árbol (token-enricher/compose.prod/.env.example) — NO tocado por C.

---

# OMEGA CORTEX — Estado de sesión persistido (pre-/compact 2026-05-12 evening) [ARCHIVADO]

> Post-compact: primera acción obligatoria → `cat .agents/memory/session_state.md`.
> El chat es volátil. Los archivos persisten. NUNCA compactar sin persistir.

## Branch + estado

- **Branch**: `main`
- **Último commit**: `25e9e1c` feat(searcher-rs): Wire CycleRegistry to TriangularEngine via from_mvp_cycles
- **VPS**: todos `Up`; searcher-rs Up 7h (PRE-mi rebuild — la nueva imagen está built pero searcher-rs NO fue restarted con `up -d`), api-server 21h, frontend 21h, edge 39h, postgres+redis healthy
- **Working tree**: 4 archivos modificados (sin commit):
  - `.agents/memory/session_state.md` (este archivo)
  - `.claude/CLAUDE.md`
  - `CLAUDE.md`
  - `GEMINI.md`

## Commits de hoy (2026-05-12)

| Hash | Autor probable | Título |
|---|---|---|
| `25e9e1c` | otra sesión | Wire CycleRegistry to TriangularEngine via from_mvp_cycles (post-mio) |
| `0c56e33` | otra sesión | Finalize dynamic discovery, fix clippy warnings and tests |
| `6c25e14` | **mío** | on-the-fly pool discovery + alloy-native decoding |
| `773da80` | otra sesión | implement dynamic pool discovery and integrate with orchestrator |
| `4f3a553` | otra sesión | V2 pipeline observability + OptimizeRejectReason + shadow-replay tests |
| `65dddb7` | otra sesión | pool_sync_watcher cursor uuid→timestamptz |
| `4efa9b9` | otra sesión | wire V2 path end-to-end (SQL + reserves + DexEngine + config snapshot) |
| `9905659` | otra sesión | kill strategy_kind literal — derive from DecodedSwap.protocol_type |
| `bf55263` | otra sesión | Phase 16-17 — per-strategy Prometheus + ImpactIndex pool-sync |

## Capas defensivas activas en producción

1. `StrategyConfigGate` doble-pasada (pre-math + post-math) — con `enabled_pool_ids` enforcement
2. `normalizeStrategyConfigs()` en api-server (shape canónico siempre)
3. Edge proxy two-mode contract (no double-append en pre-built queries)
4. Live window 5min en `/opportunities/live` (no stale-snapshot)
5. Token Validation Engine Phase 1 (UNVERIFIED badge + dual-source Uniswap + CoinGecko ~9.5K)
6. OnChainTruthValidator feature-flagged (RPC quota guard)
7. Layout `max-w-[1800px]` (aprovecha pantallas anchas)
8. `OrchestratorMode` feature flag (rollback inmediato del event-driven)
9. Per-strategy Prometheus metrics (granularidad de observabilidad)
10. Parallel-run regression test (orchestrator vs legacy paridad)
11. **NEW** On-the-fly pool discovery con alloy-native decoding (`6c25e14`) — sin synthetic "unit reserves"
12. **NEW** CycleRegistry wired to TriangularEngine (`25e9e1c`) — triangular_arb candidates ahora habilitados

## Sprint / Phase actual

- Orchestrator Phase 0–17+: ✅ implementado + en parte deployado
- Dynamic pool discovery: ✅ código en main (3 commits sucesivos: 773da80 → 6c25e14 → 0c56e33)
- CycleRegistry wiring: ✅ commit 25e9e1c
- **Pendiente operacional**: restartear searcher-rs en VPS con `docker compose up -d` para que tome la nueva imagen (la imagen YA está built; el container sigue corriendo el código viejo de hace 7h)

## Trabajo en progreso al momento del compact

1. **Plan persistido** en `C:\Users\HFRC\.claude\plans\floating-baking-brooks.md` (no en este repo) — describe la integración del conocimiento del doc "Resolving ArbitrageX WebSocket Connectivity" (1.4MB / 28K líneas)
2. **Plan mode** estaba activo cuando el usuario invocó /compact. El usuario rechazó ExitPlanMode, sugiriendo que aún no acepta ejecutar el plan
3. Mi commit `6c25e14` se pusheó + VPS pulled + image rebuilt (background task completed exit 0), pero searcher-rs NO fue restarted con `docker compose up -d`

## Conocimiento integrado del doc WebSocket Connectivity

El doc establece (extracto crítico de líneas 22600–22950 + 23351–23352):

1. **`b960606`** (commit del 3-May-2026): eliminó Socket.IO de `OpportunitiesClient.tsx` porque el Cloudflare Worker de producción NO tiene handler de WebSocket Upgrade
2. **Doctrina inmutable** (incorporada en CLAUDE.md §3 R2):
   - REST → Edge Worker (`NEXT_PUBLIC_EDGE_URL`)
   - WebSocket → api-server DIRECTO (`NEXT_PUBLIC_WS_URL`). **NUNCA via Edge.**
3. **Realidad operativa actual**: en VPS, api-server está bound a `127.0.0.1:8080` (loopback only), no es alcanzable directamente. Por eso `NEXT_PUBLIC_WS_URL` apunta al edge por necesidad — pero el edge de producción (CF Worker) no soporta WS, generando el storm.
4. **Mi fix C4** (de sesiones recientes) RE-INTRODUJO el WS con admin-token auth + degrade-to-polling. El degrade existe pero ocurre tras 3 errores visibles en consola.
5. **Plan de integración** está en `C:\Users\HFRC\.claude\plans\floating-baking-brooks.md` — propone añadir env var `NEXT_PUBLIC_WS_DISABLED=true` + hostname pattern detection en `useOpportunitiesStream` para skip inmediato del WS en producción.

## Bugs conocidos activos

- **🟡 WS storm en producción** (regresión por C4 fix de audit follow-up): plan listo en `floating-baking-brooks.md`, pendiente aprobación del operador
- **🟡 searcher-rs container desactualizado en VPS**: imagen built con `6c25e14` + commits posteriores, pero container sigue Up 7h con código viejo. Restartear: `ssh arbx "cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs"`

## Próximo paso (post-compact)

1. **Restartear searcher-rs en VPS** para tomar la nueva imagen con orchestrator + dynamic pool discovery + CycleRegistry wiring:
   ```bash
   ssh arbx "cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs && sleep 5 && docker logs --tail 30 arbitragex-v2-searcher-rs-1"
   ```
2. **Decidir sobre el plan WS** (`floating-baking-brooks.md`): el operador rechazó ExitPlanMode antes del compact, así que sigue pendiente de decisión sobre:
   - Opción A (recomendado): skip WS en producción vía env var + hostname detection
   - Opción B: exponer api-server WS publicamente con nginx/CF tunnel
   - Opción C: implementar WS handler en CF Worker
3. **E2E verify orchestrator funcionando**: query opportunities by strategy_kind en última hora para confirmar que triangular_arb + dex_arb candidatos están emergiendo:
   ```bash
   ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \"SELECT strategy_kind, COUNT(*) FROM opportunities WHERE detected_at > NOW() - INTERVAL '1 hour' GROUP BY strategy_kind ORDER BY 2 DESC\""
   ```

## Memoria operativa nueva (sin commit aún a anti_reincidencia.md)

- **Doc WS Connectivity integrado**: 28K líneas leídas, las secciones técnicas críticas (22600+) decodificadas. El conocimiento está en este state file + en el plan file.
- **Patrón "imagen built pero container no restarted"**: el flow estándar `docker build --no-cache` produce nueva imagen pero NO afecta container running. Siempre seguir con `docker compose up -d <service>` para forzar el pickup. Pattern a documentar.
- **Cloudflare Worker WS limitation**: el worker de producción `edge-arbx.ape-tv.net` no implementa `WebSocketPair` API. Cualquier código frontend que asuma WS funcional en producción genera storm. Mitigación: detección por hostname / env explícito.
