# WO-E5 · VERIFY — AG5 cross-adversarial RULE 00 (ruta del dato vivo)
Fecha: 2026-09-17 · Agente: ecc:security-reviewer (Gang Omniscience) · Board: `audits/hft-1000-20260917/GOAL-WORKORDERS.md`

## VEREDICTO: **PASS con 3 GAPS** (ninguno es fabricación de datos; 1 es deploy-lag)

`100% real` SOBREVIVE como afirmación en términos R8: **real ≠ rentable; real = sin dato fabricado**.
El sistema que muestra 2.630 rechazadas honestas, 0 accepts y passed=0 en 2.03M sims es
PRECISAMENTE un sistema real: cada rechazo lleva su razón verbatim del productor, cada
profit no-computado viaja como `null` (nunca 0 maquillado), y `window_total=111.162`
detecciones/1h son filas PG reales. Un sistema fabricado mostraría accepts. La ausencia de
rentabilidad es un veredicto de mercado, no un defecto de veracidad.

---

## 1. Ruta auditada (lectura completa de los 4 archivos claim + caza transversal)

| Capa | Archivo | Resultado RULE 00 |
|---|---|---|
| Producer (Rust) | `backend/searcher-rs/src/opportunity_emitter.rs` (1.114 L, leído íntegro) | LIMPIO |
| Producer contadores | `backend/searcher-rs/src/counters.rs` (466 L, leído íntegro) | LIMPIO |
| API | `backend/api-server/src/routes/opportunities-live.ts` (920 L, leído íntegro) | LIMPIO |
| FE store | `frontend/lib/store/realtime-slices.ts` (270 L, leído íntegro) | LIMPIO |
| FE client | `frontend/lib/api-client.ts` (1.114 L, leído íntegro) | LIMPIO |

Caza adversarial (grep exhaustivo): `Math.random` → solo `geometric-background.tsx`
(decoración visual del canvas, NO dato) y jitter de backoff inyectable
(`apex/schemas/realtime.ts:100`, parámetro `rand` — no dato). `mock/sample/fixture` →
exclusivamente `*.test.ts` y comentarios de contrato. Sentinel addresses (`0x…dEaD`) → 0 hits
en la ruta. `Date.now()` maquillando freshness → el store exige `lastMessageAt` del último
payload ACEPTADO; `lastMessageAt=null` es `connecting`, jamás `live` (realtime-slices.ts:31-33,
179-191). Fixtures condicionales por entorno → 0 encontrados en la ruta exhibida.

## 2. Precedentes re-verificados — NINGUNO regresa

### CARDS-MIRROR-01 (#445) — CONFIRMADO sin regresión (código + tests + card viva)
- `backend/searcher-rs/src/persistence.rs:53-58` `status_from_rejection_reason()`: derivación
  pura `None→'detected'`, `Some(_)→'rejected'`; usada en el INSERT único (:144), sin literales inline.
- Espejo API: `opportunities-live.ts:37` `VIABLE_STATUSES` (misma fuente: CHECK constraint
  migration 003) + `viable_only` default **false** (:665) — los rechazos son visibles por diseño.
- Test: `opportunities-live.test.ts:192-199` ("rejected/failed sin razón no se vuelve
  paper_viable") — **17/17 PASS** (vitest, ejecutado esta sesión).
- **Card viva** (request HTTP 1): fila triangular `status:"rejected"`,
  `rejection_reason:"spot_product_le_one"`, `paper_status:"paper_rejected"` — el rechazo ES
  visible con su razón. El bug de #445 (status hardcodeado 'detected' + razón poblada) NO reaparece.

### TRIANGULAR-PRICE-SCALE-01 (commit 6d034e54) — CONFIRMADO sin regresión (código + tests + card viva)
- `backend/searcher-rs/src/engines/triangular_engine.rs:606-616`: usa `token_a_price_usd`
  REAL de `extract_pricing`; precio ausente/≤0 → **no conversión fabricada**,
  `expected_amount_out` queda igual a `amount_in` (R8 fail-honest).
- Tests: `price_scale_t1..t4` — **4/4 PASS** (cargo test, ejecutado esta sesión; incluye
  stablecoin-cycle, WETH-cycle, None-price y edge epsilon).
- **Card viva**: la fila triangular USDC→DAI→USDT→USDC muestra `expected_profit_usd:null`
  (rechazada en spot-check antes del cómputo económico) — ningún número a escala 3000× en el wire.

## 3. Card viva (dominio https://arbx.ape-tv.net — 2/5 requests usados)

- `GET /api/opportunities/live?limit=3&max_age_seconds=3600` → `window_total:111.162`,
  filas con pools reales (`0xae461ca6…`, `0xb20bd5d0…`, `0x3041cbd3…`), leg_symbols DAI/USDT
  resueltos, `token_info.validation` con score y razones reales (incluye el aviso honesto
  `onchain-unavailable: skipped (TOKEN_VALIDATION_ONCHAIN_ENABLED=false)`).
- `GET /api/scanner/heartbeat?chain_id=1` → `redis_stream_total:10000`, `passed_all_gates:0`,
  `pg_period_inserted:-1` (ver GAP-2).

## 4. GAPS (observaciones, no violaciones RULE 00)

- **GAP-1 · Deploy-lag WO-G2-PARITY**: el wire VIVO devuelve `block_number:"25998598"`
  (string) — el fix local `normalizeBlockNumber()` (opportunities-live.ts:435-439) existe y
  pasa tests (WO-G2-PARITY suite incluida en los 17/17), pero el api-server desplegado en VPS
  NO lo incluye. Con `z.number()` estricto en el frontend desplegado, el feed live podría
  estar degradado en prod ("Opportunity feed unavailable"). NO-GIT respetado: no deployo;
  el operador decide el deploy. Evidencia: wire vivo vs test local.
- **GAP-2 · Heartbeat prod**: `pg_period_inserted:-1` es el centinela documentado
  (heartbeat_worker.rs:221-233: "DB ausente o unhealthy → -1, nunca bloquea observabilidad")
  — fail-honest DECLARADO pero numérico en el wire (R8 estricto preferiría null); además PG
  claramente SÍ recibe filas (111.162/1h), así que la lectura PG del heartbeat falla en prod
  mientras los inserts del emitter prosperan. Y `redis_stream_total=10000` = tope MAXLEN del
  stream: a saturación, `redis_stream_delta` subconta la tasa real de publicación (artefacto
  R9). Ambos afectan la lectura del funnel, no la veracidad de los datos de card.
- **GAP-3 · Ceros estructurales documentados**: `sim-ctl/src/consumer.rs:605-606`
  (`expected_amount_out: 0.0, gross_profit: .0`) — comentario "honest 0.0 for the fields the
  encoder does not consume; R8: never fabricated". Correcto HOY (el simulador REVM computa sus
  propios montos), pero si algún scorer llegara a leer esos campos, el 0.0 estructural se
  volvería un dato fabricado. Vigilancia, no acción.

## 5. Pregunta de ataque a WO-E6

> **¿La "razón exacta" que afirmas ver es la del productor o la traducida por la API?**

Evidencia de MI mitad de la ruta: el API pasa `rejection_reason` VERBATIM
(opportunities-live.ts:589 — copia directa de la columna PG; sin traducción ni normalización).
El productor también la guarda verbatim (opportunity_emitter.rs:518). PERO existen tres costuras
de traducción que E6 debe nombrar antes de afirmar "razón exacta":
1. **Taxonomía de contadores**: `emit_rejected` clasifica por `contains()` a minúsculas
   (opportunity_emitter.rs:466-479) — `spot_product_le_one` cae en `gate_other_rejected`: la
   razón EXACTA solo sobrevive en la fila PG, no en el contador.
2. **Label Prometheus**: `build_rejected_outcome_label()` (CamelCase→snake_case) es una
   traducción derivada, no la razón cruda.
3. **Vocabularios cruzados**: la ruta sim-wire emite razones con otro vocabulario
   (`candidate_incomplete:amount_in_wei_unparseable`, `flashloan_executor_unresolved:…`) que
   NUNCA pasa por el emitter — si E6 agrega "razones" mezclando opportunities.rejection_reason
   con gap-counters, está mezclando dos lenguajes de productor distintos.
   Además `/api/rejections/breakdown` agrupa raw→family (traducción de segundo orden con
   `raw_groups_truncated` posible).
Conclusión de ataque: la razón exacta SOLO es canónica en la fila `opportunities.rejection_reason`;
toda agregación (counter, label, family) es traducción con pérdida.

## 6. Mesa redonda
- Primer reporte en `audits/e2e-demo-20260917/` (dir vacío al iniciar). Board
  `hft-1000-20260917/GOAL-WORKORDERS.md` leído (FEE-TIER-AWARE-QUOTING en vuelo, prerrequisito
  de accepts). Contexto citado: `audits/live-activation-package-20260917/` (G2/G3 FAIL
  sims=execs=0) — consistente con passed=0 observado: el cero es honesto y esperado hasta que
  el quoting/cuello #1 se resuelva.
- Para WO-E6: usar §5. Para funnel WOs: GAP-2 (delta subcontado a MAXLEN + lectura PG del
  heartbeat fallando en prod con inserts sanos).

## 7. Evidencia de verificación ejecutada
- `cargo test --lib engines::triangular_engine::tests::price_scale` → 4/4 PASS.
- `npx vitest run src/routes/opportunities-live.test.ts` → 17/17 PASS.
- 2 requests HTTP al dominio (presupuesto 5 respetado). CERO git/deploy/VPS-mutación.
