# BR-01 — DESIGN v1.1: telemetría del embudo + contrato de stages canónicos

> **WO:** BR-01 · **kind:** design · **agente:** ecc:database-reviewer (Gang Omniscience)
> **Fecha:** 2026-09-07 (v1 13:18Z · **v1.1 18:2xZ** tras re-medición era-2 madura 4.98h)
> **Reporte completo (evidencia, cifras, refutaciones, §10 actualización):**
> [`BR-01-FORENSE-EMBUDO.md`](./BR-01-FORENSE-EMBUDO.md) — este archivo es el entregable de
> diseño compacto que el orquestador y BR-10 consumen. CERO código de producción editado
> (NO-GIT); diffs = artefacto de diseño con marcador `// BR-01 (2026-09-07)`.
> **SHA desplegado durante toda la medición:** `e65040f1` (sin mover).

## 1. El contrato (v1.1) — 9 stages, IDs estables, BR-10 los renderiza tal cual

Cambios v1→v1.1: S5 es EL cuello (77.9%); S9 gana un canal de pérdida medido (F-1);
heartbeat V1 inhabilitado como fuente (F-4). IDs y labels NO cambian.

| ID | Nombre canónico | Labels de muerte (exact strings de PG) |
|----|-----------------|----------------------------------------|
| S1 | `decode` | (heartbeat counters — **INUTILIZABLE post-#555**, F-4; RDO `source_event`) |
| S2 | `graph` | RDO: `missing_reserves`, `had_reserves=f`, `route_shape_out_of_bounds`, `*_feed_unavailable` |
| S3 | `engine` | `single_pool_no_spread`, `spot_product_le_one`, `spread_zero_equilibrium` |
| S4 | `size` | `missing_reserves_pool_b`, `missing_route_legs`, `non_positive_profit` (+ `_gross_usd`/`_net_usd`, `zero_reserves`, `missing_pool_address`) |
| S5 | `oracle` | `v3_quote_unavailable`, `unknown_token_price`, `no_price_oracle` — **77.87% de la era-2** |
| S6 | `gas` | `gas_floor_breach` (sufijo financing; en era-2 sin sufijo), `kelly_negative_edge` |
| S7 | `spine` | `TokenNotAllowed:*`, `StrategyDisabled:*`, `StrategyConfigGateBlocked:*`, `NoTradingConfig` |
| S8 | `sim` | `strategy_not_simulatable_in_s4`, `anvil_fork_not_configured*`, `build_error:*`, `reverted:*`, `sim_timeout`, `rpc_error:*`, `fork_acquire_failed` |
| S9 | `emit` | (emisión viva a ~50/s; **NUEVO: ~1% de inserts PG cae** — `opportunity_emitter.db_error` F-1, fuera de `rejection_reason`) |

Gates con `file:line` en el SHA desplegado `e65040f1`: ver FORENSE §1 (tabla completa con
`size_optimizer.rs`, `dex_engine.rs`, `triangular_engine.rs`, `config_aware.rs`,
`tx_builder.rs`, `opportunity_emitter.rs`, `route_scanner_worker.rs`).

## 2. Cuotas de muerte de referencia (v1.1 — eras separadas, INV-BR01-3)

- **24h pre-#555 (60,302):** S7 74.64% · S4 21.46% · S5 2.53% · S6 0.70% · S3 0.67%.
- **Era-2 MADURA 4.98h post-#555 (828,548, snapshot 17:45Z):**
  **S5 77.87%** · S3 15.44% · S4 5.18% · S7 1.39% · S6 0.13%. (La captura de 17 min
  en FORENSE §3.2 está SUPERSEDED por §10.1.)
- **Mapa económico cerrado (FORENSE §10.2):** CERO Topological Yield neto real computado
  en era-2 — las 8,400 filas "net>0" son 3-4 unidades crudas de tokens spam 0-decimals
  (≈$0). El Yield muere ANTES de computarse (S5 sin precio) o honestamente ≤0 (S3/S4).
  **Columnas `*_profit_usd` guardan unidades crudas (wei): BR-10/BR-11 NO renderizarlas
  como USD sin dividir por 10^decimals (RULE 00).**
- **S8 sim era-2 (7,797):** `reverted` 41.1% (#1 — probe sin balance/approval) ·
  `strategy_not_simulatable_in_s4` 37.4% (era 96.55% mixta; el mix-shift de
  BR-00-VERIFY siguió: gate <50% a ~12pp de cruzarse SOLO por drift) · build_error 8.9%
  · timeout 8.8% · rpc_error 1.6%. `passed=0`; XLEN `arbx:opps:simulated`=0;
  paper_trade_runs congelada 09-01 16:32Z.
- **S9 emisión:** ~166K/h sostenido (~66× la era-1), 50 XADD/s confirmados por
  `entries-added` Redis (1,454,018); solo 31 pares; multihop 3+ pools 12.1% VIVO;
  self-pair 13.7% (era 97.5%). Volumen ~95% event-driven, canonical_dispatch 25/bloque
  ≈ 4.2% (CORRECCIÓN de la aritmética tentativa 25/bloque≈180K/h — era falsa).
- **S2 graph (RDO 3h, 4.27M):** `had_reserves=f` 56.5% · `missing_reserves` 15.1% ·
  `is_opportunity` 9,866 (0.23%) — el grafo HALLA la señal; S5 no la puede valuar.

## 3. Diffs exactos (diseño; los aplica el orquestador vía PR con ID P-∅ §37)

1. **Vista SQL `v_br01_funnel_quota`** — DDL completo en FORENSE §7-D1 (clasifica cada
   `rejection_reason` a exactamente un stage; ventana rodante 48h; hora UTC truncada).
   Marcador `// BR-01 (2026-09-07)` en el migration file.
2. **Gate de conservación** — query D2 en FORENSE §7: `S9_unmapped == 0` y `Σ == COUNT(*)`
   **de las filas INSERTADAS**. Cualquier no-cero = taxonomy drift → PR con ID (P-∅).
3. **Gauge `arbx_funnel_stage_quota`** (api-server o edge, desde la vista) para que BR-10
   no pantallee PG en el render. Marcador `// BR-01 (2026-09-07)`.
4. **D4 (NUEVO v1.1) — rewire del heartbeat (F-4):** `workers/heartbeat_worker.rs` lee
   `XLEN` del stream capped (`redis_stream_total=10003`, delta sin sentido) y sus counters
   V1 están todos en 0 con 50 emisiones/s. Diff de diseño:
   (a) sustituir `XLEN arbx:opps:detected` por `XINFO STREAM … entries-added` y derivar
   `redis_stream_delta` de la resta entre períodos; (b) `pg_period_inserted` ya es correcto
   — conservarlo; (c) los `gate_*` V1 o se rewiring al path vivo o se marcan
   `deprecated=true` en el payload (fail-honest: un counter muerto renderizado como 0
   MIENTE). Marcador `// BR-01 (2026-09-07)`.
5. **D5 (NUEVO v1.1) — cadena de error del emitter (F-1):** el log
   `opportunity_emitter.db_error` trunca la causa (`error:"insert opportunity"`). Diff de
   diseño: propagar `err.root_cause()`/fuente de sqlx (constraint, timeout, pool) al campo
   `error` y aditivar un contador `arbx_funnel_emit_lost_total` =
   `entries_added_delta − pg_inserted_period`. Sin esto, la pérdida S9 (~1%) es invisible
   para BR-10. Marcador `// BR-01 (2026-09-07)`.
6. **Reorder sizing→spine** (cartridge_boot.rs:1399-1617) —territorio BR-04/WO-06; en
   era-2 su justificación cambió: S7 cayó a 1.39%, el ahorro grande ahora es NO EMITIR
   lo que murió sin precio (645K filas/4.98h con profit NULL en PG+Redis+retención;
   ~4M filas/día proyectadas). BR-04 debe re-derivar su target con §10.4.

## 4. Invariantes (INV-BR01-*, FORENSE §7 + una NUEVA)

- **INV-BR01-1 Conservación (re-alcance v1.1):** Σ(stage quotas) == COUNT(*) de las filas
  INSERTADAS en la ventana; `S9_unmapped == 0` siempre visible. La conservación TOTAL
  (contra lo emitido real) la cierra INV-BR01-5.
- **INV-BR01-2 Fail-honest R8:** razón desconocida → bucket `S9_unmapped` visible, jamás
  reclasificación ad-hoc; `PASS` solo con `rejection_reason IS NULL`.
- **INV-BR01-3 Dos eras:** nunca mezclar pre/post 2026-09-07 12:46:44Z (deploy #555) en un
  agregado — la distribución cambió de asesino (S7 74.6% → S5 77.9%).
- **INV-BR01-4 Read-only:** BR-01 no muta VPS; diffs quedan como diseño.
- **INV-BR01-5 (NUEVA v1.1) Reconciliación Redis↔PG:** para toda ventana,
  `entries_added(arbx:opps:detected).delta − COUNT(opportunities insertadas) ==
  emit_lost` (hoy ≈1%, F-1), y `emit_lost` debe exponerse como gauge — nunca asumirse 0.
  Redis `entries-added` es el contador de verdad de la emisión (XLEN está capped).

## 5. Gate de verificación (re-ejecutable, v1.1)

```bash
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F'|' -c \
 \"SELECT COALESCE(split_part(rejection_reason,':',1),'(PASS)'), count(*) FROM opportunities \
  WHERE detected_at >= '2026-09-07 12:46:44+00' GROUP BY 1 ORDER BY 2 DESC;\" \
 -c \"SELECT count(*) FILTER (WHERE net_expected_profit_usd > 2143721) FROM opportunities \
  WHERE detected_at >= '2026-09-07 12:46:44+00';\""
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli --raw XINFO STREAM arbx:opps:detected | grep -A1 entries-added; redis-cli XLEN arbx:opps:simulated"
# Baseline v1.1 (era-2 12:46:44→17:45Z): Σ taxonomía == 828,548 · v3_quote 77.86%
# · net>umbral-real == 0 filas · entries-added 1,454,018 · XLEN simulated 0.
# Éxito del design cuando se aplique: BR-10 renderiza las dos eras, la suma cierra
# 100.0% en ambas, y `emit_lost` es visible (≠ asumido 0).
```
