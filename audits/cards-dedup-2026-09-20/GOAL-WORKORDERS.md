# GOAL-WORKORDERS — CARDS-DEDUP-HOPS (2026-09-20)

## /goal (orden directa del operador, sesión a8, 2026-09-20)

> Misma detector+estrategia+ruta+tokens+valor+hops, solo cambia el timestamp → **UNA card** con:
> "Tiempo desde la primera detección" (ej: hace 3h) + "tiempo desde la última ratificación de
> vigencia" (ej: hace 1s), convención discreta anti-saturación. Y **filtro de hops** en el menú
> de filtros. Debatido entre sesiones (-89, -61) y ejecutado por a8.

Debate: -61 aprobó opción B (GROUP BY server + merge client WS) con condición dura clave-única
bit-a-bit; cero colisiones de archivos con su cola (#610-#617). **-89 RESPONDIÓ 2026-09-20**:
aprueba B+A, cero colisiones (sus frentes: searcher-rs/beta_priors #618, edge WO-10, Dockerfile
WO-9), tomará review adversarial del diseño B; pregunta de dirección dex_a/dex_b RESUELTA
(clave DIRECCIONAL, sin normalización — pinneada en test). Sinergia registrada: B reduce el
payload ANTES de su edge cache WO-10 (menos presión 429). Opción C fichada abajo.

**ORDEN ADICIONAL del operador (mid-turn 2026-09-20)**: la card dedup debe ser INTERACTIVA y
actualizar en el lugar SIN parpadeo de renderización — cada re-detección refresca SOLO los
valores de trade/ganancia de esa misma detección (actualización tipo streaming-snapshot sobre
la card existente). Implementación: React key estable = `routeGroupKeyOf` (card NUNCA se
desmonta por re-detección, WO-1 ✅) + store merge por group key preservando posición con
economics del incoming reemplazando in-place (WO-4).

| WO | Descripción | Dueño | Archivos | Gate | Estado |
|---|---|---|---|---|---|
| WO-1 | route-key.ts compartido + ExchangeFilters.hops + predicate + select UI + tests | a8 | frontend/lib/store/route-key.ts, ExchangeFilterBar.tsx, OpportunitiesClient.tsx | vitest + tsc | **DONE** (257b9e04: tests 13/13, tsc baseline 248 pre-existentes = cero nuevos; select hops + key estable routeGroupKeyOf) |
| WO-2 | Wire + mapper: first_seen_at / last_seen_at / confirmations (R8 null) | a8 | frontend/lib/store/types.ts | vitest mapper | **DONE** (759548f7: mapper 6/6, agregados verbatim + null en filas WS sin fabricar) |
| WO-3 | LIVE_QUERY CTE grouping (MIN/MAX/COUNT + latest row) + RowType | a8 | backend/api-server/src/routes/opportunities-live.ts | curl VPS post-deploy (-61) | **DONE local** (089cc263: tests 20/20; review adversarial -89 = APPROVE con WARN-1 → tiebreaker `o.id DESC` amarrado en 2a4b0959; deploy/curl queda para post-merge) |
| WO-4 | omni-store merge por route group key (WS path) + pruneStale por vigencia + actualización in-place de economics sin parpadeo (orden mid-turn) | a8 | frontend/lib/store/omni-store.ts | vitest grouping | **DONE local** (2a4b0959: lib/store 213/213 — grouping 9 nuevos; snapshot SSOT, colapso intra-batch, dups legacy colapsan, prune por last_seen; tsc 0. **Review adversarial -89 2026-09-20: WARN-1 amarrado en 94519259** — batch economics order-independent por latest detected_at (flush del buffer es oldest-first), + test oldest-first 10/10, suite 1326, tsc 0; NOTE-1 comentario reconciliación honesto, NOTE-2 verificado grid key=routeGroupKeyOf ya en WO-1, NOTE-3 contrato SSOT documentado en mapper, NOTE-4 registrado sin acción) |
| WO-5 | Card UI dual time discreto + badge ×N confirmaciones | a8 | frontend/components/OpportunityTradeCard.tsx | vitest + screenshot VPS | **DONE local** (bca54912: formatVigency 7/7, suite 131/131=1325, tsc 0; línea `1ª 3h · ✓ 1s` con tooltips, isStale por last_seen, badge ×N, memo comparator extendido; screenshot VPS queda post-merge) |
| WO-6 | Gates finales + PR + aviso peers | a8 | — | PR verde en cola de -61 | **DONE local** (PR **#620** abierto vs main: gates frontend 131/131=1325 + tsc 0 + api-server 20/20; aviso a -61 (cola merges + deploy/curl VPS post-merge) y -89 (review adversarial WO-4 sobre el PR); screenshot VPS post-merge. **MERGE GATE LIBERADO 2026-09-20**: -89 APPROVE explícito del head 225bcbc8, evidencia = comentario del PR #620 issuecomment-5751024696 — `gh pr review --approve` bloqueado por GitHub (mismo usuario git), el comentario ES el veredicto formal) |
| CARD-C-01 | (FUTURO, fichado) group_key canónico en emitter searcher-rs + PG upsert first/last_seen — root-total, toca hot-path, requiere benchmark propio | a8 (área searcher-rs) | searcher-rs + migrations | PR propio §37 | PARKED |

Plan canónico: `docs/superpowers/plans/2026-09-20-cards-dedup-hops-filter.md`
Branch: `feat/cards-dedup-hops` (desde origin/main post-#617-push).
