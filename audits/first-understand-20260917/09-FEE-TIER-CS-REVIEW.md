# CS REVIEW — FEE-TIER-AWARE-QUOTING (WO-06 / WO-V2)

> Reviewer: agent-cs (gang PROVE-IT, Oleada 1). Dictamen de la mesa master Hermes 2026-09-17.
> Scope: diff SIN commit en branch `fix/sim-fund01b-slot-padding-20260917`. Solo lectura.
> Contrato revisado: `07-FEE-TIER-BUILD.md` (mismo directorio).

## 1. Clasificación del working tree (`git status --porcelain` + `git diff --stat`)

### (a) ESENCIAL al fee-tier — PR-1 DEBE incluir exactamente estas 9 rutas

| Ruta | Estado | Contenido verificado |
|---|---|---|
| `backend/searcher-rs/src/v3_fee_catalog.rs` | NUEVO (untracked, 339 líneas) | Catálogo + `FeeResolution` + tests T1/T7 (pin encoding). Solo fee-tier. |
| `backend/searcher-rs/src/state_projector.rs` | M (+412/-... neto) | `ProjectV3Error`, `project_v3_quote_checked`, eliminación de `unwrap_or(500)`, `LegQuote::Unavailable(&'static str)`, mocks + T2-T6. Solo fee-tier. |
| `backend/searcher-rs/src/size_optimizer.rs` | M (+148) | `V3PoolNotCatalogued`/`V3PairNoPools` + `from_v3_unavailable_label`, captura de label en el grid V3, ~20 call sites `StateProjector::new` actualizados. Todas las líneas añadidas son fee-tier (verificado por filtrado). |
| `backend/searcher-rs/src/v3_quote_provider.rs` | M (+69/-...) | Split `rpc_tier_revert` vs `rpc_error`, reestructuración map/unwrap_or_else. Solo fee-tier. |
| `backend/searcher-rs/src/scanner.rs` | M (+80) | Boot `V3FeeCatalog` + timeout 30s + timer 60s + wiring `StateProjector::new` 3-arg. Solo fee-tier. |
| `backend/searcher-rs/src/metrics.rs` | M (+56) | `V3_FEE_RESOLUTION_TOTAL` + `V3_FEE_CATALOG_POOLS` + seed de 4 series + doc `rpc_tier_revert`. Solo fee-tier. |
| `backend/searcher-rs/src/amm_math.rs` | M (2 líneas) | `fn` → `pub(crate) fn encode_quote_calldata` (doc añadido). Cuerpo del encoding intacto. Solo fee-tier. |
| `backend/searcher-rs/src/lib.rs` | M (2 líneas) | `pub mod v3_fee_catalog;` |
| `backend/searcher-rs/src/main.rs` | M (2 líneas) | `mod v3_fee_catalog;` + `#[allow(dead_code)]` (árbol dual lib/bin). |

### (b) AJENO / ACOPLAMIENTO — PR-1 DEBE EXCLUIR

**searcher-rs pero de otro WO (WO-FUNNEL-01 2026-09-17 — verificado por contenido, NO fee-tier):**
- `backend/searcher-rs/src/counters.rs` (counter `block_intents_dispatched`)
- `backend/searcher-rs/src/opportunity_emitter.rs` (per-chain bucket `chain_counters`)
- `backend/searcher-rs/src/workers/heartbeat_worker.rs` (`block_intents_dispatched` en snapshot)
- `backend/searcher-rs/src/workers/route_scanner_worker.rs` (wire del counter por bloque)

La afirmación del builder ("cambios pre-existentes, este builder no los editó") es CONSISTENTE con el
contenido: todos los comentarios/hunks de esos 4 archivos citan WO-FUNNEL-01, ninguno WO-06.

**Dominios ajenos (otras sesiones/WOs, sin relación):**
- api-server: `index.ts`, `routes/opportunities-live.ts`(+test), `routes/paper-trade-archiver.ts`(+test), `websocket.ts`(+test), `websocket-lifecycle-logging.test.ts` (nuevo), `__debug/` (nuevo).
- frontend: 17 archivos (`layout.tsx`, `ArchivePanel`, `PipelineFunnelCard`, `RouteDiscoveryFunnelCard`, `StatusPill`, `ArbxRealtimeProvider`, `RouteDiscoveryOutcomesPanel`, `api-client.ts`, `useTokenIcon.ts`(+test), `operations-schemas.ts`, `schemas.ts`(+test), `realtime-slices.ts`(+test), `NavigationSentinel`(+test), `RuntimePostureBar.test`).
- Config/meta: `CLAUDE.md`, `.claude/settings.json`, `.claude/skills/*` (2), `.claude/HERMES_SANCHO_POLICY.md` (nuevo), `.mcp.json`, `.gitignore`, `contracts/lib/openzeppelin-contracts{,-upgradeable}` (submodules).
- Artefactos: `audits/*` (5 directorios), 2 PNG, `.claude/skills/arbx-live-engineering/references/biblioteca/`.

## 2. Lista de extracción EXACTA para PR-1

```bash
git add -- \
  backend/searcher-rs/src/v3_fee_catalog.rs \
  backend/searcher-rs/src/state_projector.rs \
  backend/searcher-rs/src/size_optimizer.rs \
  backend/searcher-rs/src/v3_quote_provider.rs \
  backend/searcher-rs/src/scanner.rs \
  backend/searcher-rs/src/metrics.rs \
  backend/searcher-rs/src/amm_math.rs \
  backend/searcher-rs/src/lib.rs \
  backend/searcher-rs/src/main.rs
```

Verificación de completitud: `git grep -n "StateProjector::new"` en el árbol retorna 29 sitios —
todos ya actualizados al signature 3-arg dentro de los 9 archivos esenciales (scanner.rs 1,
state_projector.rs 7, size_optimizer.rs 21). Ningún call site fuera de esos archivos → no falta
ningún archivo por wiring. No se encontró ningún archivo esencial olvidado por el builder.

Nota operativa: PR-1 debe crearse desde un checkout/branch limpio que contenga HEAD actual
(93a3d5da) — el `git add` de arriba sobre esta working tree NO arrastra los archivos WO-FUNNEL-01
ni los ajenos porque git add es por-ruta. Riesgo residual: solo si alguien hace `git add -A`.

## 3. Veredicto TRAIT — CONFIRMADO byte-idéntico

`pub trait V3QuoteProvider` (13 líneas, línea 69 del archivo en working tree, ~líneas 68-81 en HEAD):
extracción del bloque trait completo de HEAD (`git show HEAD:...`) vs working tree vía awk
`/pub trait V3QuoteProvider/,/^}/` → `diff` VACÍO. **BYTE-IDÉNTICO.** La afirmación del builder
es exacta. Ningún hunk del diff de state_projector.rs toca la región del trait (primer hunk
relevante posterior: @@ -303, que reemplaza `project_v3_quote` por `project_v3_quote_checked` +
wrapper, bajo el trait).

## 4. Veredicto DESVIACIONES — las 4 son reales, mínimas y sin cambio de semántica del diseño

1. **`record_observed(pool, token0, token1, fee)` (4 args vs 2 del diseño).** CONFIRMADO
   (`v3_fee_catalog.rs:86`, call site `state_projector.rs` post-quote). Justificación válida: el
   wire Redis `V3PoolInfo` no trae tokens → by_pair esrepoblable solo por observación. La semántica
   ("alta pasiva post-RPC exitosa") se conserva; by_pair en frío arranca vacío (documentado).
2. **`LegQuote::Unavailable(&'static str)`.** CONFIRMADO (hunk @@ -507). Necesario para que el
   label honesto llegue a `OptimizeRejectReason` sin tocar el trait (coherente con §3 arriba).
   Ripple mínimo (3 match sites + constructor), todos dentro de los archivos esenciales.
3. **Sin `refresh()` separado — timer 60s llama `load_from_redis` directo.** CONFIRMADO en
   `scanner.rs` (tokio::spawn + interval 60s, primer tick descartado, error no-fatal retiene
   snapshot). Dentro de las opciones que el propio diseño ofrecía.
4. **`encode_quote_calldata` → `pub(crate)`.** CONFIRMADO en `amm_math.rs` (solo firma + doc;
   encoding intacto). Único modo de anclar T1/T7 al byte real.

**Extra-flags observados (sin impacto, informativo):** `#[allow(dead_code)]` en `main.rs` para el
módulo duplicado en el árbol bin — necesario por el dual-target lib/bin, patrón ya usado por los
módulos vecinos (`state_projector`, `v3_quote_provider` idénticos en main.rs).

## 5. Tests — corrida INDEPENDIENTE (no la del builder)

```
cargo test -p searcher-rs --lib  (desde backend/)
test result: ok. 1274 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.59s
exit code 0
```

**1274 passed / 0 failed / 3 ignored** — coincide exactamente con el reporte del builder
(1274/0/3). Los 3 ignored son los `hydrate_*` pre-existentes (requieren Redis vivo).

## 6. Veredicto final

- Extracción PR-1: **9 rutas esenciales** (§2), sin olvidos del builder, sin mezcla ajena dentro
  de ellas (verificado hunk a hunk / línea a línea por filtrado de contenido).
- Excluir: 4 archivos searcher-rs de WO-FUNNEL-01 + todo api-server/frontend/config/audits (§1b).
- Trait byte-idéntico: **SÍ** (evidencia §3).
- Desviaciones: **4/4 mínimas, justificadas, sin cambio de semántica** (§4).
- Tests independientes: **1274/0/3, exit 0** (§5).

Sin bloqueos para PR-1 desde CS. Cero ediciones realizadas al código (solo lectura + este reporte).
