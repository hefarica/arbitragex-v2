# DRIFT-REPORT — SCHEMA-DRIFT + DESTINOS-DE-DATOS 2026-09-20

> Órdenes del operador: "busca en donde mas hay schema drift en la dapp. audita corrige e implementa y mejora" +
> "auditoria de todos los destinos a donde viaja o se supone viajaría la información".

## VEREDICTO EJECUTIVO

1. **No hay drift de COLUMNAS real entre repo y VPS.** El esquema desplegado (1139 columnas,
   66 tablas, `information_schema` volcado a `deployed_schema.tsv`) es derivable del repo.
   Todo el ruido "tipo A/B" del parser (`diff_schema.py`) son artefactos: 60 migraciones usan
   SQL dinámico (DO $$/EXECUTE) que el parseo estático no puede resolver. Verificado por grep
   directo: `tvl_usd` (044/045), `evidence_vector`/`evidence_computed_at` (103),
   `last_seen_at` (034), `resolved_via`/`resolved_at` (028b/034), `ops_overhead_usd_per_attempt`
   (047), `backend_route` (068) — todas existen en migraciones.
2. **Drift REAL #1 — el LEDGER de migraciones miente.** `schema_migrations` registra 32
   versiones, última `099`. Las migraciones 100–121 (24 archivos) están APLICADAS en el VPS
   sin registro. Causa raíz: coexisten DOS runners —
   - `automation/scripts/migrate.sh` (ledger-tracked, INSERT por archivo; registró hasta 099
     en su última corrida) — dejó de ser el camino canónico.
   - `database/run_migrations.sh` (canónico del deploy, re-aplica TODO idempotentemente en
     cada deploy) — **jamás escribe el ledger**.
   Riesgo: cualquier tooling/auditor que confíe en `schema_migrations` mal-diagnostica;
   `migrate.sh` (que aún existe y es invocable) re-aplicaría 100–121 sin los lock-guards
   PGOPTIONS del canónico (clase de riesgo FREEZE-01). **FIX: PR — run_migrations.sh ahora
   registra cada archivo aplicado (version + checksum sha256, ON CONFLICT DO UPDATE).**
3. **Drift REAL #2 — tabla `drift_observations` sin ESCRITOR.** Creada por migración 067,
   leída por `backend/api-server/src/routes/system-manifest.ts:112-127` (GET
   `/api/system/drift`) y renderizada por `frontend/components/registries/RegistryCoherenceStrip.tsx`.
   ZERO escritores en todo el codebase (grep backend/*.rs, api-server, edge: 0 resultados).
   El frontend muestra "COHERENT — 0 observaciones" sobre una tabla SIEMPRE vacía: veredicto
   fabricado por ausencia de productor (viola el espíritu §79/R8: el comentario del propio
   componente promete "NO COMPUTADO" solo si la tabla no existe — pero la tabla existe y está
   vacía, cayendo en el caso peor). El "drift engine" del registry-coherence nunca se
   implementó; el drift_tracker de recon escribe `paper_trade_runs` (labels Y), no esta tabla.
4. **Los db_errors del searcher NO son drift** (ya verificado pre-compaction): lock timeouts
   PG clusterizados ~:02-:05 de hora por contención con cron de retención/TRUNCATE +
   `pg_terminate_backend`. → WO-S6. El `column "status" does not exist` es IRREPRODUCIBLE en
   la ventana de logs retenida (R9: no se fixea un fantasma).

## PARTE 1 — SCHEMA DRIFT (detalle con evidencia)

### 1.1 Lo verificado OK
- INSERT del searcher (22 columnas de `opportunities`, persistence.rs:146-192): todas existen
  desplegadas; persiste 4.3K/min.
- `scored_opportunities.evidence_vector` (103): existe, 59K+ rows la poblaron.
- Tablas huérfanas desplegadas (sin CREATE en migraciones): particiones `audit_log_*` (las
  crea 019 con SQL dinámico — correcto), `gate_c_metrics`, `opportunities_archive_pre_sprint3`,
  `tokens_reconciliation_audit`, vistas `v_sed_*` → operacionales/por diseño, NO drift.

### 1.2 Ledger drift (drift real #1) — evidencia
```
VPS: SELECT version FROM schema_migrations → 32 rows, máx = 099_opportunities_route_metadata
repo: database/migrations/ → 111 archivos (001→121; 103 y 107 tienen prefijos DUPLICADOS)
run_migrations.sh: re-aplica todos, no registra (leído completo, líneas 107-162)
migrate.sh:52: INSERT INTO schema_migrations — es el único que registraba
```
Consecuencia medida: 24 archivos (100–121) aplicados sin registro.

## PARTE 2 — DESTINOS DE DATOS (dónde viaja, dónde se atasca, dónde apunta a otra parte)

### 2.1 Redis streams — censo (XLEN capado por trimming; pending = señal dura)
| Stream | XLEN | Veredicto |
|---|---|---|
| `arbx:opps:detected` | 10 000 (cap) | VIVO — searcher → validador/scoring |
| `arbx:opps:validated` | 10 001 (cap) | VIVO |
| `arbx:scoring:scored` | 100 007 (cap) | VIVO |
| `arbx:route_discovery:outcomes` | 1 000 003 (cap) | VIVO |
| `arbx:opps:simulated` | 0 | MUERTO — sin productor real (0 gates pasados) |
| `arbx:hot:simulated` | 0 | MUERTO — paper executor solo publica passed+net>0 |
| `arbx:opps:executed` | 0 | Esperado en paper (§34) |
| `arbx:opps:simulated:dlq`, `arbx:accounting:pending`, `arbx:route_discovery:tick` | type=none | NUNCA creados — wires declarados sin productor |

### 2.2 Consumer groups — atascos (la información que NO llega a su destino)
| Grupo | lag | pending | Destino afectado |
|---|---|---|---|
| `selector-g0` (feed del frontend/cards) | **45 831** | **6 096** | **Cards/WS del dashboard corren con backlog; 6K mensajes claimed-no-acked = posible consumidor muerto mid-batch** |
| `rd-outcome-sink-g0` | **5 589 872** | **10 883** | route_discovery_outcomes PG (17.4M rows ya); sink nunca alcanza |
| `scoring-archiver-g0` | 133 310 | 962 | scored_opportunities PG |
| `paper-archiver-g0` | 44 242 | 326 | paper ledger |
| `enricher` | 1 850 | 0 | enriquecimiento opps |
| `sim-ctl-g0` | 0 | 0 | sano |

### 2.3 PostgreSQL — tumbas y huecos (66 tablas)
- **`drift_observations` = 0 rows, sin escritor, con lector** → drift real #2 (arriba). El
  flujo "motor de drift → /api/system/drift → RegistryCoherenceStrip" viaja SOLO en la mitad
  consumidora: la información "llega" al frontend vacía POR DISEÑO ACCIDENTAL.
- 0 rows por diseño §34 (sin capital/broadcast): `executions`, `relays`, `routers`,
  `paper_trade_runs` (truncada por orden 09-19), etc.
- 0 rows por inanición upstream (0 gates pasados): `math_operator_calibration`,
  `bayesian_priors`, tablas `sed_*`.
- Vivos: `opportunities` 1.7M · `scored_opportunities` 5.4M · `route_discovery_outcomes`
  17.4M · `pool_reserves` 19.8M.

### 2.4 Priorización de fixes (WO-D6)
1. **P0 — productor de `drift_observations`** (o degradar el veredicto del strip a
   NO COMPUTADO mientras no exista): es un verdicto de coherencia fabricado en pantalla.
2. **P0 — `selector-g0` pending 6 096**: revisar consumidor (XAUTOCLAIM/XPENDING autopsy);
   es el feed del dashboard.
3. **P1 — `rd-outcome-sink-g0` lag 5.6M**: backlog estructural del sink; decidir
   poda/rebalanceo de grupo.
4. **P1 — ledger de migraciones** (fix ya en PR).
5. **P2 — streams declarados sin productor** (`route_discovery:tick`, `accounting:pending`,
   `simulated:dlq`): eliminar del código declarativo o implementarlos; hoy son cables muertos
   que confunden auditorías.
6. **P2 — WO-S6 lock timeouts** del INSERT vs cron de retención (reunión de contención, no drift).

## Archivos de evidencia
- `deployed_schema.tsv` (1139 columnas VPS), `redis_census.txt`, `pg_census.txt`, `diff_schema.py` (parser; SU OUTPUT SIN CURAR ES NO-CONFIABLE — usar este reporte).
