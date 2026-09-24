# 02-WO02 — Buffet de gates hop-econ (2026-09-19, orquestador inline)

> Gang 429-sistemático (3 muertes: WO-02, WO-02A, WO-02B — proveedor zai/GLM saturado
> por el run Hermes concurrente). Circuit-breaker v1.2 aplicado: cero agentes,
> investigación hecha por el orquestador en línea. Evidencia verificable abajo.

## Veredicto E2E contra el dominio vivo (https://arbx.ape-tv.net)

Sonda read-only `GET /api/opportunities/live` (2026-09-19 ~20:30 local):
- count=50, window_total≈16.8K, **route_metadata presente en 50/50**, leg_symbols 37/50.
- **leg_ledger: 0/50 · simulated_amount_in_usd: 0/50** (campo existe, valor null — R8).
- Distribución de rechazos de la ventana: `v3_quote_unavailable` 39/50 (78%) ·
  `spot_product_le_one` 7 · `non_positive_profit` 2 · `single_pool_no_spread` 2.

## Gates por capa

| Capa | Gate | Estado | Evidencia |
|---|---|---|---|
| searcher→PG | Ledger sólo en filas Sized (by design) | **ACTIVO — causa raíz** | WO-01: attach_leg_ledger en orchestrator.rs:1125-1146; Kelly rescale nullea ledger (size_optimizer.rs:3404). Filas rejected (100% de la ventana viva) nunca llegan a sizing → sin ledger, sin USD in/out |
| searcher→PG | Gate de quotes V3 | **ACTIVO — dominante (78%)** | `v3_quote_unavailable` 39/50. Conecta con la anomalía abierta "2º path de quote no instrumentado" |
| searcher→PG | has_computed_economics | INOFENSIVO | opportunity_emitter.rs:762-764 — reclasifica aceptaciones sin economía (correcto, no dropea datos) |
| searcher→PG | try_insert_pg_with_route legacy None | INACTIVO | opportunity_emitter.rs:717-724; todos los accepted pasan por emit_accepted_with_plan con route_ref (orchestrator.rs:1454, cartridge_boot.rs:1678) |
| api-server | Proyección opportunities-live.ts | INOFENSIVO | WO-01: pasa route_metadata VERBATIM (:288,622-627, leg_symbols :577) |
| edge worker | Reshape/proyección | **INOFENSIVO — transparente** | edge/worker/src/index.ts:666 — proxy() pass-through al api-server, sólo KV-cache 2s, body intacto; walletProxy reenvía body verbatim (:620-647) |
| frontend | types.ts + render | INOFENSIVO | WO-01/WO-03: types.ts:501-550 preserva; tab Ledger extendido DONE local |
| deploy | Version drift | DESCARTADO | VPS HEAD bbfe0ab5; ab880676 (ledger) y cdb4c890 (S1 fee) SON ancestros del árbol desplegado |

## Conclusión

**No existe gate de transporte que dropee hop-econ.** La cadena api-server→edge→WS→FE es
transparente y el código del ledger está desplegado. El bloqueante real para "ver economía
por hop fluir en el dominio vivo" es UPSTREAM: el funnel rechaza el 100% de la ventana antes
del sizing — dominado por `v3_quote_unavailable` (78%). Mientras eso persista, las cards
(WO-03) mostrarán honestamente "—" (R8). El próximo paso de mayor palanca es reparar la
disponibilidad de quotes V3 (segundo path no instrumentado + 93 claves sin reparar), NO
tocar transporte ni frontend.

Nota: sonda inicial parseó mal (payload usa `items`, no `data`) — corregido en la segunda sonda.
