# RCA — `build_error: amount invalid: zero amount_in` (18% de sims fallidas, ~1.400/día)

**Fecha:** 2026-09-17 · **Modo:** 100% read-only (cero cargo/build/git/VPS) · **Alcance:** diagnóstico con evidencia file:line; sin fixes.

---

## 1. Sitio emisor EXACTO

| Elemento | Ubicación |
|---|---|
| Check que dispara | `backend/sim-ctl/src/tx_builder.rs:81-85` — `U256::from_dec_str(&opp.amount_in_wei)` parsea **"0"** válido (no es error de parseo) y `amount_in.is_zero()` → `Err(BuildError::InvalidAmount("zero amount_in"))` |
| Texto del enum | `backend/sim-ctl/src/tx_builder.rs:34-35` — `#[error("amount invalid: {0}")]` |
| Prefijo `build_error:` | `backend/sim-ctl/src/sim_engine.rs:83` — `Err(e) => return Self::failed(id, trace_id, &format!("build_error: {e}"))` |
| Label final en PG | `"build_error: amount invalid: zero amount_in"` — composición exacta de las tres piezas anteriores. **Único sitio del repo** que emite este string (grep `zero amount_in` + `amount invalid` verificado). |

Condición necesaria y suficiente: la `Opportunity` que llega a `SimEngine::simulate` trae `amount_in_wei == "0"` literal (parse decimal exitoso + valor cero). El emisor NO es SizeOptimizer ni sim_multistep: `sim_orchestrator.rs:115` ("RoundTripContext has zero amount_in") es OTRO label del path searcher, no éste.

## 2. Cadena productor → consumidor

```
[mempool] tx → route_decoder (decode_to_route_intents)
    route_decoder.rs:113-121: UR ALREADY_PAID amountIn==0 se queda en 0 por diseño (R8);
    ETH-in legacy → tx.value. → intent.amount_in puede ser 0.
        ↓ (test que lo fija: route_decoder.rs:924 ur_zero_amount_in_stays_zero)
[engines] StrategyCandidate con opportunity.amount_in_wei:
    • dex_engine.rs:367      → intent.amount_in VERBATIM en accepted (0 posible)
    • triangular_engine.rs:570-572 → amount None ⇒ unwrap_or_else("0") en rechazados
    • cartridge_boot.rs:1187 → intent.amount_in.to_string() VERBATIM
        ↓
[SizeOptimizer] si Sized ⇒ kernel sobrescribe opportunity.amount_in_wei (size_optimizer.rs:768/786/1001/1019/1259/1277 — siempre >0, kernels rechazan profit<=0).
Si Rejected ⇒ el candidato conserva el amount del engine (posible "0") y el HARDENING
orchestrator.rs:996-999 MANTIENE expected_profit_usd Some (el gate net es de ejecución, no de detección).
        ↓
[opportunity_emitter.emit_rejected] opportunity_emitter.rs:517-524 — clona con rejection_reason seteado
    y PUBLICA al MISMO stream `arbx:opps:detected` (publisher.rs:41, RULE 00: "siempre recibe la opportunity").
        ↓
[selector-api] prefilter/decide (pre-SEL-GATE-01) IGNORABA el veredicto del productor
    (status/rejection_reason) ⇒ re-decidía como accept filas ya rechazadas.
        ↓ XADD arbx:opps:validated
[sim-ctl Consumer] consumer.rs:48 lee `arbx:opps:validated`; path legacy ANVIL
    → SimEngine::simulate (sim_engine.rs:47) → build_probe (tx_builder.rs:81-85) → FAIL.
```

Nota de arquitectura: el label solo existe en el path **anvil legacy** de sim-ctl; el path B2c/revm (route-aware, `sim_runner::run_real_simulation`) no pasa por `build_probe`. Su presencia masiva en PG implica que el consumidor productivo corre el backend anvil (GAP: no verifiqué `SIM_BACKEND` en el VPS).

## 3. Hipótesis rankeadas

### H1 (MÁS PROBABLE — causa primaria, ya identificada por el audit hermano): filas RECHAZADAS por el productor (con `amount_in_wei="0"`) re-decididas como accept por selector-api

- **Evidencia directa:** `backend/selector-api/src/policy/sel-gate01.test.ts:5-11` documenta el incidente: *"87% of sims were status='rejected' / amount_in_wei='0' rows re-decided as accepts"* — funnel 2026-09-17T04:00Z, citando `audits/real-cycles-audit-20260916/00-SYNTHESIS.md` (detección 100% rejected, `v3_quote_unavailable` 60/76).
- **Mecanismo:** `emit_rejected` publica al stream detected (opportunity_emitter.rs:524) y el `decide()` del selector no miraba `status`/`rejection_reason` hasta el guard `producerRejected()` añadido en `backend/selector-api/src/policy/engine.ts:36-39,56-58` (commit `fdb40401`, SEL-GATE-01, 2026-09-17 — está en ESTA branch, no en main).
- **Por qué el amount es "0":** los rechazos provienen de candidatos cuyo amount nunca fue sizeado (kernels solo escriben amount en `Sized`): dex_engine accepted construye con `intent.amount_in` verbatim (dex_engine.rs:367) que puede ser 0 (UR ALREADY_PAID), triangular_engine rejected usa `None ⇒ "0"` (triangular_engine.rs:570-572), y el HARDENING del orchestrator mantiene el profit Some (orchestrator.rs:996-999) de modo que el scoring del selector tenía números para "aceptar".
- **Coherencia con la escala:** detección 100%-rejected ⇒ casi todo el feed detected son filas amount=0 → ~18% de sims fallidas con este label es exactamente el orden esperado.
- **Corrobora el fenotipo:** `audits/cerebro-2026-09-07/BR-00-VERIFY.md:188-190` — "el sample de stream muestra triangular con amount_in_wei:'0'" (166 rows `build_error: zero amount_in` en 2h, pre-BR-00).

### H2 (residual post-fix): emisiones ACCEPTED legítimas con amount 0 que pasan el gate de economía

- `has_computed_economics` (opportunity_emitter.rs:750-752) solo exige `expected_profit_usd.is_some() || net_expected_profit_usd.is_some()` — **no valida amount_in_wei > 0**. El test `no_economics_is_not_computed_economics` (opportunity_emitter.rs:946-953) cubre amount-0+profit-None, pero NO amount-0+profit-Some.
- Productor candidato: cartridge path — `cartridge_boot.rs:1187` copia `intent.amount_in` (0 en UR ALREADY_PAID) y `:1193-1197` setea `expected_profit_usd` desde `profit_usd_hint > 0` ⇒ accepted con amount "0" que SÍ pasaría incluso el SEL-GATE-01 (no tiene rejection_reason).
- Menor volumen que H1 pero es el vector que queda vivo tras desplegar SEL-GATE-01.

### H3 (evaluada y DESCARTADA como productora del label): bordes numéricos del sizing

- `clamp_to_cap_wei` (triangular_worker.rs:381-418) puede devolver `Some(0)` si el cap USD vale < 1 wei del token (`wei_f.floor()` de un valor en (0,1), línea 408-409) — pero TODOS los consumidores lo neutralizan: `evaluate_cycle` rechaza `profit_at_clamped <= 0` (triangular_worker.rs:815-821) y `size_two_leg_with_reason` rechaza `NonPositiveProfit` (size_optimizer.rs:939-941). Ningún `Sized` sale con amount 0 por esta vía.
- Kelly: `kelly_cap_wei` puede ser 0 cuando `0 < wei_f < 1` pese al guard `wei_f <= 0.0` (size_optimizer.rs:577-581 floor posterior), y de hecho escribe `sized.optimal_amount_in = kelly_cap_wei` (líneas 615/634) — pero **nunca escribe `opportunity.amount_in_wei`**, y el orchestrator tampoco sincroniza (orchestrator.rs:945-969 copia profits, no el amount). Produce inconsistencia amount↔profit, no el label de build. Inconsistencia documentada como GAP de coherencia, no como causa.

## 4. Tests existentes que tocan el path

| Test | Qué cubre | Qué NO cubre |
|---|---|---|
| `backend/selector-api/src/policy/sel-gate01.test.ts` (64-76) | La clase del incidente H1: filas rejected + amount "0" no deben llegar a validated | El residual H2 (accepted sin reason) |
| `backend/searcher-rs/src/route_decoder.rs:924` `ur_zero_amount_in_stays_zero` | Fija el productor del 0 (R8, ALREADY_PAID) | — |
| `backend/searcher-rs/src/opportunity_emitter.rs:946-953` | amount-0 + profit-None ⇒ reclassificado | amount-0 + profit-Some pasa el gate (hueco H2) |
| `backend/sim-ctl/src/tx_builder.rs` (mod tests, 280-501) | kind/cyclic/chain/router/pancake | **NO existe test del brazo `InvalidAmount("zero amount_in")`** — el check de tx_builder.rs:83-85 no tiene regresión unitaria |

## 5. Telemetría / queries PG para confirmar cada hipótesis

**H1** (en VPS, read-only):
```sql
-- ¿Las sims con este label corren sobre filas que el productor ya rechazó?
SELECT o.status, o.rejection_reason, count(*)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id
WHERE s.fail_reason = 'build_error: amount invalid: zero amount_in'
  AND s.simulated_at > now() - interval '24 hours'
GROUP BY 1,2 ORDER BY 3 DESC;
-- H1 confirmada si domina status='rejected' / rejection_reason NOT NULL.
```
Secundaria: `SELECT count(*) FROM simulations WHERE fail_reason LIKE 'build_error: amount invalid%' AND simulated_at > '<deploy SEL-GATE-01>';` — si el rate cae a ~0 tras el deploy de `fdb40401`, H1 queda cerrada.

**H2**:
```sql
SELECT o.strategy_kind, o.cartridge_id, count(*)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id
WHERE s.fail_reason = 'build_error: amount invalid: zero amount_in'
  AND o.rejection_reason IS NULL AND o.amount_in_wei = '0'
  AND s.simulated_at > now() - interval '24 hours'
GROUP BY 1,2 ORDER BY 3 DESC;
-- H2 confirmada si quedan rows POST-deploy-SEL-GATE-01 dominadas por cartridge_id NOT NULL.
```
Telemetría de código: contador `event="opportunity_emitter.accept_without_economics"` NO cubre H2 (el gate pasa); habría que agregar amount==0 al warn o al gate — propuesta, no implementada (read-only).

**H3 (descartada)**: `SELECT count(*) FROM opportunities WHERE amount_in_wei='0' AND status='validated' AND expected_profit_usd > 0 AND rejection_reason IS NULL AND cartridge_id IS NULL;` — esperado ~0.

## 6. GAPs (fail-honest)

- **GAP-1:** Sin acceso VPS/PG en esta sesión (read-only local): las queries de §5 están formuladas pero NO ejecutadas; la distribución real por `strategy_kind`/`status` del label hoy es inverificable desde aquí.
- **GAP-2:** Estado de deploy de SEL-GATE-01 (`fdb40401`) desconocido — vive en la branch actual `fix/v3-slot0-coverage-20260917` (HEAD~2), no verifiqué main ni el VPS. La cifra "~1.400/día" del operador es consistente con pre-fix; post-fix solo H2 debería sobrevivir.
- **GAP-3:** Backend productivo de sim-ctl (anvil legacy vs B2c/revm, `SIM_BACKEND`) no verificado — la existencia del label en PG implica que el path anvil corre, pero no descarto coexistencia de ambos.
- **GAP-4:** El sample "triangular con amount_in_wei:'0'" (BR-00-VERIFY) no pudo re-verificarse contra el stream (entradas ya trimeadas, MAXLEN 10.000). La reconstrucción por código indica que un triangular TRUE-cycle moriría antes como `CyclicRouteNotRepresentable` (tx_builder.rs:76); las rows triangulares con zero-amount serían rechazos del triangular_engine (amount None ⇒ "0") re-publicados — consistente con H1, no con un triangular accepted.
- **GAP-5:** `~18%` y `~1.400/día` tomados del enunciado del operador; no recalculados.

## 7. Síntesis

El label nace en `tx_builder.rs:84` y muere en `sim_engine.rs:83`; el "0" lo fabrican los PRODUCTORES upstream (dex_engine/triangular_engine/cartridge_boot copiando `intent.amount_in` o el default R8 "0" de rechazados), lo transporta el stream único `arbx:opps:detected` (rejected y accepted comparten stream por diseño RULE 00), y lo deja llegar a la sim la ausencia de guard de veredicto-del-productor en selector-api (H1, 87% del burn según el audit hermano; fix SEL-GATE-01 ya escrito en esta branch). El residual post-fix es H2: `has_computed_economics` no exige amount > 0, y el cartridge path puede emitir accepted amount-0 con profit. Recomendación de verificación (no ejecutada): correr las queries de §5 tras el deploy y, si H2 se materializa, endurecer el gate del emitter + agregar la regresión unitaria faltante del brazo `InvalidAmount` en tx_builder.
