# Incidente: V3 quote transport saturation — pipeline sin candidatos evaluables (2026-09-25)

> ID: V3-QUOTE-TRANSPORT-TUNE · Estado: fix en PR `fix/v3-quote-transport-tune` (sin deploy) ·
> Frontera: sin capital, sin broadcast, sin flips. Evidencia-only (RULE 00/R8).

## 1. Síntoma

- Prometheus (acumulado, post-deploy `882bac0c`): transporte de quotes V3 — unary 5,445
  intentos → 1,572 ok / **3,873 error (71%)**; batch 1,728 → 49 ok / **1,679 error (97%)**;
  negative-cache 196k hits.
- `v3_quote_unavailable` = 16/19 cards vivas; el gate selector rechaza el 100%
  (223,393 oportunidades en la última hora, todas `rejected`).
- `arbx:opps:validated` estancado 8.5 días → `simulated`/`executed` XLEN=0 desde siempre →
  `simulations` = 0 filas en PG. Cero Topological Yield histórico.

## 2. Diagnóstico (cadena causal con evidencia)

1. El quoter despacha **chunks de `DEFAULT_BATCH_SIZE=100`** pools por `eth_call` aggregate3
   (`backend/searcher-rs/src/v3_quote_provider.rs:54`), mientras el mismo codebase limita el
   batching de precios a **5** "to stay under provider request limits"
   (`workers/price_worker.rs:85`). → 20× la carga que los proveedores gratuitos toleran.
2. Timeout rígido de **5s** para el multicall completo (`amm_math.rs` — ahora tunable, R4).
3. Pool de proveedores contaminado: alchemy 429 sin quota, 1rpc 410 Gone, y
   **flashbots/mevblocker son relays MEV, no RPC generales** (403 permanente al `eth_call`
   del quoter). Evidencia: `audits/hop-econ-cards-2026-09-19/06-V3QUOTE-ROOT-CAUSE-2.md:11-18`
   y `audits/live-readiness-2026-09-22` (rpc_ok=4.400 vs rpc_error=15.426).
4. Fallo de transporte → **neg-cache fijo de 2s** (`DEFAULT_QUOTE_NEG_TTL_MS=2000`) →
   reintento cada 2s → 36.6 rechazos/s → el gate recibe `v3_quote_unavailable` en masa →
   100% rechazo → `validated` muerto → 0 sims → 0 ejecuciones.
5. Conclusión: **clase transporte** (saturación/mala configuración de proveedores), no
   cobertura de catálogo. El backfill de cobertura sobre pools vacías habría violado RULE 00.

## 3. Fix de código (este PR)

| Knob | Archivo | Default | Efecto |
|---|---|---|---|
| R4 `ARBX_V3_QUOTE_MULTICALL_TIMEOUT_MS` | `amm_math.rs` | 5000 | Timeout del aggregate3 configurable sin recompilar (parse fail-honest) |
| R5 `ARBX_V3_QUOTE_NEG_TRANSPORT_TTL_MS` | `v3_quote_provider.rs` | 30000 | Negativos de **transporte del UNARY** (failover agotado) con TTL largo; los reverts de pool conservan 2s |
| R5 v2 `ARBX_V3_QUOTE_BATCH_BACKOFF_MS` | `v3_quote_provider.rs` | 30000 | **Backoff de alcance BATCH**: un fallo de transporte del prefetch pausa el prefetch y NO escribe nada en el caché por-key — el unary autoritativo sigue cotizando (peer-review P0) |

**R5 v2 — regla esencial (peer-review P0, aceptada):** *UN BATCH FALLIDO NO DEBE IMPEDIR QUE UNA
COTIZACIÓN UNARIA POTENCIALMENTE RENTABLE SEA INTENTADA.* El batch es best-effort; su fallo de
transporte setea `batch_backoff_until` (prefetch pausado 30s, se limpia en el primer batch OK) y
deja el caché de quotes intacto. Los negativos de transporte de 30s viven SOLO en la ruta unary.

Tests: `parse_multicall_timeout_ms_fail_honest`, `parse_neg_transport_ttl_fail_honest`,
`transport_negative_outlives_pool_revert_negative` (deterministas, sin RPC, sin env en CI).

## 4. Parche env (VPS, junto al deploy — operador)

```env
# R1 — alinear con la disciplina del price_worker (provider request limits)
ARBX_V3_QUOTE_BATCH_SIZE=5
# R4/R5 — knobs nuevos (opcionales; defaults razonables si se omiten)
ARBX_V3_QUOTE_MULTICALL_TIMEOUT_MS=8000
ARBX_V3_QUOTE_NEG_TRANSPORT_TTL_MS=30000
ARBX_V3_QUOTE_BATCH_BACKOFF_MS=30000
```

**R2 — sanear `RPC_HTTP_1`** (lista separada por comas `name=url,...`):
- QUITAR: `1rpc` (410 Gone, proveedor muerto), `flashbots` (`/fast` es relay, 403 a eth_call),
  `mevblocker` (relay, 403 a eth_call).
- CONSERVAR: `drpc`, `publicnode`, `0xrpc`, `blockpi`, `llama`.
- **R3 (decisión del operador, costo)**: Alchemy quota PAYG — único proveedor con headroom
  (hoy 429 incluso en `eth_blockNumber` suelto).

## 5. Verificación post-deploy (artefactos, §34.5.3)

1. `arbx_v3_quote_total{outcome}`: `rpc_ok/rpc` sube de ~22% a >70%; `batch_call_error` cae
   de 97% a <30%.
2. `arbx_rpc_provider_state{service=searcher-rs}`: breakers cierran (Healthy, no Open).
3. Histograma `rejection_reason` (PG, 1h): `v3_quote_unavailable` deja de dominar.
4. `XLEN arbx:opps:validated` crece por primera vez en 8.5 días.
5. `simulations` (PG) recibe su primera fila de la historia; `arbx:opps:simulated` XLEN > 0.

## 6. Rollback

`git revert <commit>` + redeploy, o restaurar env previo (`ARBX_V3_QUOTE_BATCH_SIZE` fuera,
knobs nuevos fuera, `RPC_HTTP_1` original). Sin migraciones, sin cambios de esquema, sin
tocar el terminus `relays-client` (default-deny intacto).

## 7. Backlog de la revisión por pares (aceptado, NO incluido en este PR)

1. **P0 — estado post-transacción**: `eth_call` QuoterV2 cotiza el estado confirmado Q(S₀), no
   el proyectado Q(S₁) tras una tx pendiente. Para oportunidades nacidas de txs pendientes se
   requiere quote con estado proyectado (REVM/fork/state override) y etiquetar ambos como
   económicamente distintos. → WO propio.
2. **P1 — caché positivo block-aware**: `QuoteKey` no contiene bloque; un positivo de 8s puede
   cruzar un cambio de head. → añadir versión de estado al key o invalidación por head.
3. **P1 — adaptive batching por proveedor**: descubrir el batch size tolerado (100→50→25) por
   provider en runtime, en vez de un fijo. → mitigación inmediata vía env `ARBX_V3_QUOTE_BATCH_SIZE=5`.
4. **P2 — test de regresión económico**: batch-fail → unary-success → la oportunidad sobrevive el
   gate (requiere mock del pool RPC en CI). Estructuralmente garantizado por R5 v2 (el batch ya
   no escribe el caché); el test E2E queda para el harness de integración.
