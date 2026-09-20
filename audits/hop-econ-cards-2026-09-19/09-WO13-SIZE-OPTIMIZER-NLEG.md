# WO-13 — SizeOptimizer N-leg: cards muestran solo h2/h3 (causa raíz + fix)

**Fecha**: 2026-09-20 · **Branch**: `fix/size-optimizer-nleg-cycle` (base `fe403e0a`) ·
**Archivos**: `backend/searcher-rs/src/size_optimizer.rs`, `backend/searcher-rs/src/workers/triangular_worker.rs`

## Síntoma

Screenshot del operador: chips de hops en cards = `h2:1 · h3:5 · h4:30 · h5:57 · h6:7 · h7:0`
mientras la detección encuentra rutas en múltiples hops (h4..h7). Las cards solo muestran h2/h3.

## Causa raíz (verificada, no hipótesis)

El dispatch del Step 7 (`size_optimizer.rs`) solo tenía kernels honestos para rutas 2-leg
(V2/V3) y 3-leg triangular. Candidatos de 4..7 legs caían al kernel two-leg, que leía las
reservas de los primeros 2 pools como si la ruta fuera de 2 hops → 100% rechazados con
`missing_reserves_pool_b` / `spot_product_le_one` (ninguna ruta 4+ legs pasaba a las cards).

La matemática del kernel de ciclo YA era genérica en N: `spot_product`, `cycle_profit` y
`golden_section_search` iteran `hop_reserves: Vec<(U256,U256)>` de longitud arbitraria.
SOLO el wiring la bloqueaba (guard `!= 3`, `take(3)`, `Vec::with_capacity(3)`, dispatch sin
brazo N-leg, y `evaluate_cycle` con guard `!= 3`).

El fix es wiring, no matemática nueva — la regla de admisibilidad canónica del workbook
(`StrategyHopAllowed(s,h) = h ∈ [max(2,Min_Legs_s), min(7,Max_Legs_s)]`, hoja `08_HOPS_2_7`)
queda ahora materializada en sizing para TODO el rango 2..7.

## Cambios (5 sitios + enum)

1. **Dispatch Step 7**: brazo nuevo `legs != 2 → size_triangular_with_reason` (kernel ciclo
   N-leg); rutas V3 multi-leg (>2) rechazan honestas con `V3MultilegUnsupported` (los brazos
   QuoterV2 son estrictamente 2-leg; truncar fabricaría la economía).
2. **`size_triangular_with_reason`** generalizada a N-leg: guard `< 2 → MissingRouteLegs`,
   `> 7 → UnsupportedLegCount` (fail-closed; discovery ya clampa con `min(7)`); itera TODOS
   los legs; ledger genérico `ins = [x] + outs[..n-1]`, `outs = leg_outputs` (antes
   hardcodeado a 3 → los leg_amounts de rutas 4+ se truncaban aunque pasaran).
3. **Guards defensivos fail-closed** en `size_two_leg_with_reason` y
   `size_two_leg_v3_with_reason`: `legs > 2 → Rejected(UnsupportedLegCount)` — el mismo
   bug no puede volver a entrar por el kernel 2-leg.
4. **`triangular_worker.rs::evaluate_cycle`**: guard `hop_reserves.len() != 3` → `< 2`
   (admite 2..7; <2 no es un ciclo).
5. **Enum `OptimizeRejectReason`**: variantes nuevas `UnsupportedLegCount`
   ("unsupported_leg_count") y `V3MultilegUnsupported` ("v3_multileg_unsupported").

## Validación (TDD + vector independiente)

- **Vector independiente 4-hop** (Python, computado fuera de Rust y pineado en el test):
  hops=[(100,120),(100,110),(100,90),(100,200)]e18, x=1e18 →
  `leg_outs=[1184589641276473558, 1283975251278183205, 1137548963352774498, 2242835817401646860]`,
  `profit=1242835817401646860`, `spot_product=2.3476160475844563`. El pin PASÓ **pre-fix**
  (confirma kernel genérico; el bug era wiring).
- **6 tests RED→GREEN** por causa exacta: dispatch multileg V2V2 4-leg sizes vía kernel
  ciclo; V3 multi-leg rechaza con `v3_multileg_unsupported`; ledger cubre las 4 legs sin
  truncar (cadena `ins[i+1]==outs[i]`, `ins[0]==optimal_amount_in`); kernel 2-leg rechaza
  4-leg defensivamente; `evaluate_cycle` sizea ciclo 4-hop completo.
- **Suite lib completa searcher-rs**: **1335 passed · 0 failed · 5 ignored** (EXIT=0).
- **Tests de integración** (`cargo test -p searcher-rs --tests -j 2`): las suites que
  ejecutaron pasaron (**1335 + 1319 passed, 0 failed**), pero el binario `calldata_test`
  no pudo **ejecutarse** — `os error 4551` (Windows AppControl bloquea el .exe de test en
  el target dir compartido; bloqueo ambiental pre-existente documentado en memoria, no
  causado por este cambio). Corridas previas abortaron por **os error 1455** (pagefile
  agotado al mmap libtest del sysroot — C: al 99%). CI de GitHub (Rust tests, rust-unit,
  Rust integration) es el gate autoritativo para estos suites.

## Impacto esperado

Las rutas 4..7-leg que hoy mueren 100% en sizing (`missing_reserves_pool_b` /
`spot_product_le_one`) pasarán al kernel de ciclo y aparecerán en las cards como chips
h4..h7, con `leg_amounts_in/out` completos por hop. Las V3 multi-leg rechazan con razón
explícita y trazable (R8) en vez de razones falsas.

## Post-deploy (WO-S7)

- Recontar chips h2..h7 en `/opportunities` (debe aparecer h4+).
- Recount de `rejection_reason` en PG: `unsupported_leg_count`/`v3_multileg_unsupported`
  reemplazan a los falsos `missing_reserves_pool_b` en rutas 4+ legs.
- Latencia p50 por hop (h4..h7) a extraer de `pipeline_latency_ms` + RTT #611.
