# VALUE PIPELINE — cómo tiene que funcionar esto (levantamiento canónico)

> **Para qué existe este archivo.** Responde UNA pregunta, siempre la misma:
> **¿por qué una celda de la card muestra `—` y qué tiene que pasar, eslabón por eslabón, para que muestre el número?**
> Si un cambio toca un productor o un consumidor de cualquier valor de card, **este archivo se actualiza en el mismo PR**.
> No es documentación decorativa: es el mapa contra el que se juzga cada fix (ver §7 Protocolo).
>
> Base verificada: `main` @ `c4b61470` (2026-09-27). Las líneas citadas se leyeron en ese SHA.

---

## 1. EL PIPELINE COMPLETO (ASCII)

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  CAPA 0 · DESCUBRIMIENTO (searcher-rs, hot path)                                                     │
│                                                                                                      │
│   mempool / bloque ──► scanner.rs ──► engines/dex_engine.rs ──► StrategyCandidate{opportunity, legs}  │
│                                          │                                                           │
│                                          │  amount_in_wei = PROBE (10^decimals(token_in), #685)       │
│                                          ▼                                                           │
│   route_discovery/multi_hop_search.rs ──► hop_cycle_bridge.rs ──► ciclos 3..=7 hops                  │
│                                          │  cap por (chain, epoch): ARBX_MULTIHOP_EMIT_MAX_PER_TICK=12│
│                                          │  lanes: sizeable | unpriceable (PH-RESERVES-01)            │
└──────────────────────────────────────────┼───────────────────────────────────────────────────────────┘
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  CAPA 1 · SIZING (searcher-rs/src/size_optimizer.rs)  ← AQUÍ NACE (o no) TODA LA ECONOMÍA           │
│                                                                                                      │
│   optimize_with_reason(candidate)                                                                    │
│     ├─ legs <= 2  + V3  ──► size_two_leg_v3_with_reason      (QuoterV2 real)                         │
│     ├─ TriangularArb, >2 legs, CON V3 ──► size_multileg_v3_or_refuse ──► size_multileg_v3_with_reason│
│     │       └─ (knob ARBX_V3_MULTILEG_SIZING=off ⇒ Rejected(V3MultilegUnsupported))                  │
│     ├─ TriangularArb, resto ──► size_triangular_with_reason  (V2 golden-section)                     │
│     └─ N-leg all-V2 ──► kernel de ciclo genérico                                                     │
│                                                                                                      │
│   SALIDA (esto es lo que decide si la card pinta o no):                                              │
│     OptimizeOutcome::Sized{ gross_usd, net_usd, optimal_amount_in, leg_amounts_in[], leg_amounts_out[] }
│     OptimizeOutcome::Rejected(reason, Option<SizedCandidate>)  ← el Option es el agujero histórico   │
└──────────────────────────────────────────┬───────────────────────────────────────────────────────────┘
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  CAPA 2 · EMISIÓN (searcher-rs/src/orchestrator.rs + opportunity_emitter.rs)                          │
│                                                                                                      │
│   process_candidate()                                                                                │
│     ├─ rama ACEPTADA: opp.expected_profit_usd = gross; opp.net_expected_profit_usd = net              │
│     │                  route_metadata.leg_amounts_in/out  (attach_leg_ledger)                        │
│     └─ rama RECHAZADA:                                                                               │
│          orchestrator::apply_gate_rejection_fields()  ──► opp.roi_pct = None; opp.risk_score = 0.0    │
│          cartridge_boot.rs (path cartuchos)           ──► opp.expected_profit_usd = None  ★FÁBRICA★   │
│                                                                                                      │
│   stamped_for_emit() (opportunity_emitter.rs)                                                        │
│     └─ si hay economía pero amount_in_wei == "0" ⇒ sella el probe de 1 unidad nativa (HOPS-UNITS-01) │
│                                                                                                      │
│   emit*  ──► Redis arbx:opps:detected (MAXLEN 10000)  +  PG opportunities                            │
│              route_metadata = { decimals, dex_adapters, pool_addresses, token_addresses,             │
│                                 [leg_amounts_in, leg_amounts_out]  ← sólo si SÍ se dimensionó }      │
└──────────────────────────────────────────┬───────────────────────────────────────────────────────────┘
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  CAPA 3 · API (api-server)                                                                           │
│                                                                                                      │
│   GET :8080/api/v1/opportunities/live        (routes/opportunities-live.ts)  ← interna               │
│   GET :8787/api/opportunities/live           (edge)  ← ESTA es la que consume la página              │
│     ├─ ventana: ?max_age_seconds (default 300, máx 86400) · ?limit (máx 200) · ?viable_only          │
│     ├─ rowToOpportunity(): sirve los campos del wire tal cual (null = no computado, NUNCA 0)         │
│     └─ forwardSimulate / computeSimulatedNet.ts                                                      │
│          ├─ sin symbol/precio/amount finito  ⇒ return null        (la simulación NO se inventa)      │
│          └─ solveDualFloors: r <= 0 ⇒ required = Infinity  ──► JSON.stringify ⇒ null  ★AGUJERO★      │
│                                     binding_floor = "net-per-usd-nonpositive" | "roi-unreachable"    │
└──────────────────────────────────────────┬───────────────────────────────────────────────────────────┘
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  CAPA 4 · FRONTEND (Next.js)                                                                         │
│                                                                                                      │
│   app/opportunities/page.tsx  (Server Component)                                                     │
│     EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl()                                       │
│     fetch(`${EDGE_URL}/api/opportunities/live?order=profit_usd`)  ──► snapshot                        │
│        └─ fallo ⇒ { source: "server-fetch-failed" }  (fail-honest, no inventa)                        │
│   OpportunitiesClient.tsx  (isMounted ? store : initialSnapshot)   ← SSR-FIRSTPAINT-01               │
│                                                                                                      │
│   lib/store/types.ts::mapToOmniOpportunity(raw)  ──► OmniOpportunity                                  │
│        hop_count = deriveHopCount(route_metadata)  ← DERIVADO del wire, no es columna                 │
│   lib/opportunity-ledger.ts::buildLedger(opp)  ◄── SSOT: qué aritmética cerrada se puede pintar       │
│        simulated  ⇔ el bruto es atribuible a SU propio principal (SANITY_PROFIT_MULT_OF_CAP = 5)      │
│                     y el costo es pagable                                                            │
│        canonical  ⇔ total_cost := gross − net (no se pinta principal; celdas mudas con razón)         │
│        none       ⇔ sin notional ⇒ todo en guion + razón                                             │
│   components/OpportunityTradeCard.tsx                                                                │
│        Ladder: Flash loan in (TLS) · Hop k/N · Gross out (AMM spread) · Repay · 9 costos · Total cost · Net yield
│        Grid:   IN · GROSS · NET · BPS(roi) · RISK · SIM · LATENCIA                                    │
│        TARGET · SIM TAB  ──► targetVerdict (FAIL/PASS/no target)                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. DE DÓNDE SALE CADA CELDA (y quién la puede dejar en `—`)

| Celda de la card | Campo del wire | Productor | Se vuelve `—` cuando |
|---|---|---|---|
| **IN** (principal) | `simulated_amount_in_usd` (o `amount_in_wei` valorado) | `forwardSimulate` → sim | no hay simulación (rechazo) **o** `gross == null` ⇒ `computeSimulatedNet` devuelve `null` |
| **GROSS** | `expected_profit_usd` | kernel de sizing (rama `Sized`) | la fila es un rechazo: **`cartridge_boot.rs` lo pone a `None`** ★ |
| **NET** | `net_expected_profit_usd` | kernel (rama `Sized`) o `0.0` fijo en el path triangular | rechazo sin figuras (`Rejected(reason, None)`) |
| **BPS / ROI %** | `roi_pct` | sólo en el paso del spine (`orchestrator` rama PASS) | **`apply_gate_rejection_fields` lo fuerza a `None`** ★ |
| **RISK** | `risk_score` | ramas del spine | no se fija en el path de cartuchos; en rechazos queda `0.0` |
| **SIM** | `simulated_net_profit_usd` | `computeSimulatedNet` | `gross == null` ⇒ no hay simulación |
| **TARGET · SIM TAB** | `simulated_target.*` | `solveDualFloors` | `required = Infinity` ⇒ JSON `null`; el veredicto sigue saliendo (FAIL con su aritmética) |
| **Ladder · Gross out** | `expected_profit_usd` / `simulated_gross_usd` | kernel / sim | sin notional ⇒ celda muda (decisión de `buildLedger`, R8) |
| **Ladder · Total cost** | `gross − net` | derivado | sin bruto ni net ⇒ muda |
| **Ladder · Net yield** | `net_expected_profit_usd` / `simulated_net_profit_usd` | kernel / sim | ambos `null` |
| **Ladder · Hop k/N** | `route_metadata.leg_amounts_in/out` | kernel (`attach_leg_ledger`) | la fila no se dimensionó (rechazo) ⇒ **no existe el campo** |
| **RUTA** | `route_metadata.dex_adapters` | bridge/persistencia | sin topología persistida |
| **HOPS** | `deriveHopCount(route_metadata)` | frontend (derivado) | sin `dex_adapters` |

★ = fábrica de nulos verificada en `main @ c4b61470`.

---

## 3. ÁRBOL DE DECISIÓN: "¿por qué esta celda es `—`?"

```
La card muestra —
│
├─ ¿La fila tiene route_metadata.leg_amounts_in? ─── NO ──► la fila NO se dimensionó
│     │                                                      ⇒ no hay economía que pintar.
│     └─ ¿rejection_reason?  (esto dice QUIÉN la mató)
│           ├─ v3_quote_unavailable ......... el proveedor V3 no cotizó (transporte RPC)   ← 76 % del flujo
│           ├─ v3_multileg_unsupported ...... el kernel N-leg con V3 no existía (pre-#700)
│           ├─ v3_multileg_budget_exhausted . presupuesto por (chain,block) agotado (post-#700)
│           ├─ missing_reserves_pool_a|b .... el caché de reservas V2 no tiene el pool
│           ├─ non_positive_profit .......... SÍ se calculó y no daba (FAIL legítimo)
│           └─ gas_floor_breach ............. SÍ se calculó y el gas se lo come (FAIL legítimo)
│
└─ ¿La fila SÍ tiene economía? ─── SÍ ──► ¿por qué igual hay celdas en —?
      ├─ roi_pct null ............ el rechazo fuerza roi=None (apply_gate_rejection_fields)
      ├─ expected_profit_usd null  el path de cartuchos lo borra (cartridge_boot)
      ├─ simulated_* null ........ gross==null ⇒ la simulación no corre (es downstream)
      └─ required null ........... required=Infinity ⇒ JSON null (R8: "no hay tamaño finito")
```

**Regla de oro (doctrina del operador):** `—` sólo es legítimo cuando el número **no existe en el wire**.
Si el kernel calculó una cifra, la fila DEBE llevarla aunque el veredicto sea FAIL. `FAIL` = "se calculó y no cumple el criterio"; nunca "no hay números".

---

## 4. ESTADO MEDIDO (as-is, 2026-09-27 ~01:50Z) — el baseline contra el que se compara cada fix

| Medición | Valor | Comando |
|---|---|---|
| Filas servidas por la API | 26–28 (ventana 300 s; 47 en 1 h) | `GET :8787/api/opportunities/live` |
| Filas con **cualquier** economía | **1 de 28** | ídem |
| Filas profundas (≥3 hops) | 10 de 28 | ídem |
| Filas profundas **con ledger** | **0** | ídem |
| `amount_in_wei` en filas profundas | **`"0"`** en 10/10 | ídem |
| `expected_profit_usd` / `net` / `roi_pct` en profundas | **null / null / null** | ídem |
| Rechazo dominante del sistema | **`v3_quote_unavailable` = 1516 de ~1999 / min (76 %)** | `paper_archiver.skip_rejected_summary` en api-server |
| Filas profundas post-#697 | `v3_multileg_unsupported` 100 % (1536/1536 en 13,6 min) | PG |
| Ledger en PG (última hora) | **0** (6 h: 1899 · 24 h: 2376) | `SELECT count(*) … route_metadata ? 'leg_amounts_in'` |
| Cartuchos v4 evaluando | 264 desplegados; `pertinent: 269, positive: 0`; motivo dominante `dispatch_needs_route_data` (174/269) | log del searcher |

---

## 5. LAS 6 BARRERAS CONOCIDAS (cada una con su ID y su PR)

| # | Barrera | Dónde | ID / PR |
|---|---|---|---|
| 1 | El kernel N-leg no sabía dimensionar ciclos con pierna V3 | `size_optimizer.rs` | **#700 V3-MULTILEG-SIZING-01** (mergeado; deploy en curso) |
| 2 | Las filas rechazadas no llevan NINGUNA cifra (roi/gross borrados, `Rejected(_, None)`) | `cartridge_boot.rs`, `apply_gate_rejection_fields` | **ALWAYS-COMPUTE-ECONOMICS** (patch listo: `RejectedComputed` + `EconomicsComputation`) |
| 3 | `Infinity` desaparece al serializar (`required_amount_in_usd`) | `computeSimulatedNet.ts` | mismo patch (centinela `"Infinity"` + `required_is_infinite`) |
| 4 | El transporte V3 se cae (76 % del flujo) | `v3_quote_provider.rs`, lista RPC gratuita | **#674** (DIRTY: conflicto sólo en un doc; rama de otra sesión) |
| 5 | La card no dice POR QUÉ una celda va en guion | `OpportunityTradeCard.tsx` | **ALWAYS-COMPUTE-01** (panel de diagnóstico + `@basis` por cifra) |
| 6 | El ledger de papel está vacío y el executor dormido | `index.ts:1960` (`ARBX_PAPER_EXECUTOR_MODE` ausente) | decisión del operador + `arbx:hot:simulated` XLEN=0 |

---

## 6. COMANDOS DE VERIFICACIÓN POR CAPA (copiar/pegar, sin inventar nada)

```bash
# CAPA 1+2 · ¿el kernel dimensionó y selló el ledger?
ssh arbx 'docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -t -A -F"|" -c "
SELECT jsonb_array_length(route_metadata->'"'"'dex_adapters'"'"') AS hops,
       count(*), count(*) FILTER (WHERE route_metadata ? '"'"'leg_amounts_in'"'"')
FROM opportunities WHERE detected_at > now() - interval '"'"'10 minutes'"'"'
  AND route_metadata ? '"'"'dex_adapters'"'"' GROUP BY 1 ORDER BY 1;"'

# CAPA 2 · ¿por qué murió cada fila profunda?
ssh arbx 'docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -t -A -F"|" -c "
SELECT rejection_reason, count(*) FROM opportunities
WHERE detected_at > now() - interval '"'"'10 minutes'"'"'
  AND jsonb_array_length(route_metadata->'"'"'dex_adapters'"'"') >= 3 GROUP BY 1 ORDER BY 2 DESC;"'

# CAPA 3 · ¿qué ve la página exactamente? (misma ruta que el SSR)
ssh arbx 'curl -sS http://127.0.0.1:8787/api/opportunities/live?order=profit_usd' \
  | python -c "import json,sys; d=json.load(sys.stdin); it=d['items']; print('items',len(it));
print('con ledger', sum(1 for r in it if 'leg_amounts_in' in (r.get('route_metadata') or {})));
print('con economía', sum(1 for r in it if r.get('expected_profit_usd') is not None or r.get('net_expected_profit_usd') is not None))"

# CAPA 4 · ¿qué pinta la card en el DOM? (marcador SSR: React parte el texto con <!-- -->)
ssh arbx 'curl -sS http://127.0.0.1/opportunities' | grep -oE 'Target.{0,120}?Sim tab' | wc -l
```

---

## 7. PROTOCOLO DE ACTUALIZACIÓN (obligatorio, mismo PR)

1. **Todo PR que toque un productor o consumidor de un valor de card actualiza este archivo**: la fila de §2 que cambie, y §4 con la medición nueva.
2. **Ninguna afirmación sin comando.** Si una fila de §2 no se puede respaldar con un `grep`/consulta, se marca `NO VERIFICADO` — no se redondea.
3. **El baseline de §4 se vuelve a medir** antes y después de cada deploy, y se registra en el issue fijado **DEPLOY LEDGER** (#705).
4. **Un cambio que rompa la promesa "FAIL con números"** (una celda en `—` con cifra calculada) se revierte o se arregla antes de mergear: es la doctrina del operador, no una preferencia.
5. **Prohibido** dar por resuelto un fix de card sin **render local con datos reales** (sandbox) Y la verificación en el dominio vivo (§6).
