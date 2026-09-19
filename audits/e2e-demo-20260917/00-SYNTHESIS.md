# 00-SYNTHESIS — DEMO E2E: veredicto unificado honesto (R8)

**Programa:** e2e-demo-20260917 · **WO:** WO-E8 (síntesis final) · **Fecha:** 2026-09-17
**Compositor:** ecc:code-architect (Gang Omniscience) · **Método/diseño:** `WO-E8-DESIGN.md`
**Presupuesto HTTP de cortesía de E8:** 0/2 gastados (el estado de inputs se resolvió por
filesystem y `hermes_status` local; los HTTP citados abajo son de los pares, no míos).

---

## §0. INVENTARIO DE INPUTS — **PARCIAL (2/7) — re-ejecución requerida**

| WO | Reporte | Estado |
|---|---|---|
| WO-E4 | `WO-E4-VERIFY.md` (12:25, ecc:performance-optimizer) | **EN DISCO** — incorporado §1.4 |
| WO-E5 | `WO-E5-VERIFY.md` (12:27, ecc:security-reviewer) | **EN DISCO** — incorporado §1.5 |
| WO-E1, E2, E3, E6, E7 | — | **INPUT MISSING** al cierre de esta pasada de E8 |

Historia fail-honest: al INICIO de E8 el directorio `audits/e2e-demo-20260917/` NO
existía (0/7; dos ventanas de poll + `hermes_status` con `active_agents=0`). Los pares
aterrizaron DURANTE la composición (E4 12:25, E5 12:27) y la ventana de espera se agotó
con 5 reportes aún ausentes. Conforme a INV-E8 (`WO-E8-DESIGN.md` §4): las secciones que
requieren E1/E2/E3/E6/E7 se publican `NOT COMPUTED — INPUT MISSING`; el veredicto
NO puede ser PASS/FAIL/GAPS completos y queda **NO-ADJUDICABLE (INPUTS PARCIALES)**.
La re-ejecución de E8 (§7) completa las secciones marcadas sin reescribir el método.

## §1. QUEDA DEMOSTRADO (componible con los inputs presentes)

### 1.4 — de WO-E4 (`WO-E4-VERIFY.md`)

- **E4-V1 (PRIMARY_SOURCE):** transporte RT del demo sano — engine.io sesión estable
  (`sid=MBZryrhGESBSlBKJAABV`), 100% long-poll vía CF tunnel (0 upgrades WS), entrega
  continua 33-47 ev/s; coste extra ≈ 1 RTT de tunnel vs WS nativo. `WO-E4-VERIFY.md:44-47`.
- **E4-V2 (PRIMARY_SOURCE):** pierna server-side detección→emit socket.io: p50 164-239 ms,
  p95 345-642 ms, p99 373-721 ms, ~18.4K eventos con `skipped=0` (R8 limpio).
  `WO-E4-VERIFY.md:37-42`.
- **E4-V3 (CANONICAL_REPO):** el "polling-400" del orquestador NO se reprodujo; el ruido
  console real de la ventana es `GET /api/quote/anchor?chain_id=1 → 503` en loop. El 400
  se clasifica INFERRED como rechazo de upgrade WS en el tunnel, sin pérdida de frames.
  `WO-E4-VERIFY.md:53-60`.
- **E4-V4 (fail-honest):** latencia evento→card NO COMPUTADA — /opportunities mostró
  "0 viable / 0 total" toda la ventana, 0 mutaciones DOM; el agente rehusó inventar cifra.
  `WO-E4-VERIFY.md:50-51`.

### 1.5 — de WO-E5 (`WO-E5-VERIFY.md`)

- **E5-V1 (PRIMARY_SOURCE):** RUTA DEL DATO VIVO LIMPIA bajo RULE 00 — 4.726 líneas
  leídas íntegras across emitter/counters/API/store/client (LIMPIO ×5); caza adversarial:
  `Math.random` solo en decoración de canvas y jitter de backoff; `mock/sample/fixture`
  solo en tests; sentinels 0 hits; freshness exige payload ACEPTADO (`lastMessageAt=null`
  jamás `live`). `WO-E5-VERIFY.md:15-31`.
- **E5-V2 (verificado con tests + wire vivo):** precedentes SIN regresión —
  CARDS-MIRROR-01 (#445): `status_from_rejection_reason()` pura, test 17/17 PASS, card
  viva con `rejection_reason:"spot_product_le_one"` visible; TRIANGULAR-PRICE-SCALE-01
  (6d034e54): precio real de `extract_pricing`, 4/4 tests PASS, card viva con
  `expected_profit_usd:null` (sin número a escala 3000×). `WO-E5-VERIFY.md:33-53`.
- **E5-V3 (PRIMARY_SOURCE, 2 HTTP):** wire vivo `window_total:111.162` detecciones/1h
  con pools reales y leg_symbols resueltos; heartbeat `passed_all_gates:0`,
  `redis_stream_total:10000`. `WO-E5-VERIFY.md:55-62`.
- **E5-V4 (afirmación R8 central):** "100% real SOBREVIVE en términos R8: real ≠
  rentable; 2.630 rechazadas honestas + 0 accepts + passed=0 en 2.03M sims es
  PRECISAMENTE un sistema real; un sistema fabricado mostraría accepts."
  `WO-E5-VERIFY.md:4-11`.

### 1.x — E1, E2, E3, E6, E7: **NOT COMPUTED — INPUT MISSING** (§0).

## §2. Registro ADYACENTE componible (OTROS programas — no sustituye al E-series)

| # | Hecho | Evidencia canónica |
|---|---|---|
| A1 | FEE-TIER-AWARE-QUOTING landed en main (PR #581: a25da887, fbdd1899, 9b381475; merge 6b0f00c4) | `git log` |
| A2 | Verificación matemática del fee-tier: PASS-CON-SALVEDADES (S1-S6); 1274/0/3 re-ejecutada por el verificador | `first-understand-20260917/08-FEE-TIER-MATH-VERIFY.md:8,154` |
| A3 | CS review fee-tier: 9 rutas esenciales PR-1, sin bloqueos, 1274/0/3 independiente | `first-understand-20260917/09-FEE-TIER-CS-REVIEW.md` §6 |
| A4 | CATALOG-BACKFILL-01 built+tested (12 tests, lib 1290/0/3) SIN commit/deploy | `first-understand-20260917/10-CATALOG-BACKFILL.md:134-180` |
| A5 | TRIANGULAR-PRICE-SCALE-01 commiteado (6d034e54) SIN merge a main | `first-understand-20260917/11-TRIANGULAR-PRICE-SCALE.md:7-14` |
| A6 | Gates live 2026-09-17 temprano: G2 FAIL (passed=0 historia), G3 FAIL (executions=0), G1 PARCIAL | `live-activation-package-20260917/GATES-G1-G8-ESTADO.md:7-18` |

**Contradicción entre pares adjudicable con lo presente (única):** board HFT-1000 dice
fee-tier "En vuelo... corriendo" (`hft-1000-20260917/GOAL-WORKORDERS.md:63-65`, mtime
06:27) vs `git log` PR #581 mergeado → el board es staled por el merge; lo abierto es el
EFECTO (accepts>0 post-deploy), no el código. Sin E1..E7 no hay más cruces adjudicables.

## §3. GAPS abiertos (componibles)

| Gap | Descripción | Dueño tentativo | ¿Bloquea "100% real"? |
|---|---|---|---|
| G-META-1 | 5/7 reportes E-series ausentes (E1,E2,E3,E6,E7) — síntesis incompleta; veredicto total no adjudicable | orquestador (re-despacho) + re-ejecución E8 | SÍ bloquea el VEREDICTO, no la honestidad |
| G-E4-GAP2 (fuerte) | server broadcastea ~2.3K opps/min (room `opportunities`) mientras /opportunities muestra "0 viable / 0 total" y 0 mutaciones — ¿hook page-local suscrito a la room correcta? El transporte NO es | WO-E2/WO-E7 (derivado por E4) | SÍ hasta explicar la desconexión display↔stream |
| G-E5-GAP1 (fuerte) | Deploy-lag WO-G2-PARITY: wire VIVO devuelve `block_number:"25998598"` (string); fix local NO desplegado → feed live potencialmente degradado en prod | operador (deploy decision) | SÍ para la afirmación de demo E2E en el dominio público, hasta deploy |
| G-E5-GAP2 (medio) | Heartbeat prod: `pg_period_inserted:-1` centinela numérico (R8 estricto preferiría null) + `redis_stream_total=10000` = tope MAXLEN (subcuenta tasa, artefacto R9) | funnel WOs | NO (lectura del funnel, no veracidad de card) |
| G-E4-GAP1/GAP3 (medio) | Sin upgrade WS por CF tunnel (RTT extra); el 400 original sin captura exacta → clasificación INFERRED | config ingress (fuera de demo) / recaptura | NO |
| G-E5-GAP3 (vigilancia) | `sim-ctl/consumer.rs:605-606` ceros estructurales "honest 0.0" — correctos hoy, peligrosos si un scorer los lee | vigilancia futura | NO hoy |
| G-DEP-1 | Deploy post-#581 sin verificar; CATALOG-BACKFILL-01 sin commit/deploy; 6d034e54 sin merge | operador + builders | SÍ hasta verificar pipeline vivo post-fix |
| G-ACC-1 | accepts>0 sin artefacto reproducible post-fee-tier → sin labels (WO-F2) ni G2/G3 | searcher VPS + WO-F2 | SÍ para "extremo a extremo CON accepts"; NO para honestidad |

## §4. Respuestas cruzadas (las 7 preguntas de ataque)

Solo 2 preguntas son visibles con los inputs presentes:

- **E4 → E7** (`WO-E4-VERIFY.md:70-72`): ¿tu medición de latencia controló por transporte
  real (100% polling vía tunnel, +1 RTT) y por fuente de timestamp (`detected_at` WS vs
  snapshot REST 30s)? **PENDIENTE — requiere reporte E7** (la síntesis NO responde por
  el originador sin su evidencia).
- **E5 → E6** (`WO-E5-VERIFY.md:85-105`): ¿la "razón exacta" es la del productor o la
  traducida por la API? E5 aporta SU mitad: API pasa `rejection_reason` VERBATIM
  (`opportunities-live.ts:589`), emitter guarda verbatim (`opportunity_emitter.rs:518`);
  y nombra 3 costuras de traducción con pérdida (contador `contains()` :466-479, label
  Prometheus CamelCase→snake, vocabularios cruzados sim-wire vs emitter). Conclusión de
  E5: la razón exacta SOLO es canónica en `opportunities.rejection_reason`. **La
  adjudicación final espera el reporte E6**; si E6 afirma "razón exacta" desde
  agregaciones (counters/labels/families), la síntesis deberá marcarlo como hallazgo
  refutado por E5 §5.
- **E1, E2, E3, E7 → resto:** **NOT COMPUTED — INPUT MISSING** (§0).

## §5. VEREDICTO DEMO E2E: **NO-ADJUDICABLE (INPUTS PARCIALES 2/7)**

Regla mecánica (`WO-E8-DESIGN.md` §3): con inputs ausentes el veredicto compuesto no
puede emitirse sin fabricar. Lo que SÍ se afirma con lo presente:

1. **El criterio R8 del operador es correcto y ya rinde verdicts parciales:** E5
   demuestra en SU mitad (ruta del dato vivo, 4.726 L + wire + tests) que "100% real"
   SOBREVIVE — 0 fabricación, rechazos con razón verbatim, null ≠ 0 maquillado
   (`WO-E5-VERIFY.md:4-11`). La ausencia de accepts NO es contradicción de la
   honestidad (E5-V4).
2. **El transporte del demo NO es el problema** (E4-V1/V2/V3): sesión estable,
   entrega continua, p95 server < 650 ms.
3. **Dos gaps fuertes abiertos impiden hoy un PASS completo:** G-E4-GAP2 (2.3K/min
   broadcasteados vs "0 viable / 0 total" en pantalla) y G-E5-GAP1 (deploy-lag del fix
   block_number → feed live potencialmente degradado en prod). Ambos con dueño y
   derivación.
4. **Rechazadas honestas con 0 accepts = parte de la honestidad** — criterio
   operador, confirmado operativamente por E5-V3/V4 (passed_all_gates:0, 111.162
   detecciones/1h reales).

Estado intermedio honesto: **GAPS-parcial con tendencia a PASS-condicionado** en las
mitades E4/E5, **SIN PRONUNCIARSE** sobre las mitades E1/E2/E3/E6/E7.

## §6. Prerrequisitos HFT-1000 que quedan en pie

- **"Sin accepts no hay labels" sigue EN PIE:** E5-V3 observa `passed_all_gates:0` en
  prod a las ~17:1xZ (post E4/E5); A6 lo confirma en la medición de la mañana. El
  código fee-tier landed (A1) PERO su efecto (accepts>0) sigue sin artefacto → G-ACC-1
  bloquea WO-F2 (etiquetado a escala) y G2/G3.
- **CATALOG-BACKFILL-01 sin deploy** (A4): su efecto (93 claves legado reemplazadas,
  labels precisos) sigue predicción.
- **TRIANGULAR-PRICE-SCALE-01 sin merge** (A5) — aunque E5-V2 confirma que en el wire
  vivo ya NO aparece el defecto (la card muestra `expected_profit_usd:null`); el fix
  6d034e54 está verificado 4/4 en local y SIN regresión.
- **WO-R5/F1/F1b/F3/F4/F6:** intactos, sin cambios componibles esta sesión.

## §7. Cómo completar este archivo (re-ejecución de E8)

1. Asegurar reportes E1, E2, E3, E6, E7 en este directorio (re-despacho si cayeron —
   respawn-2 por mitades; nota: `hermes_status` degraded por disco 92.4%, posible
   causa de caída; 2 ya aterrizaron durante la espera).
2. Re-ejecutar E8: llenar §1.1-§1.3/§1.6/§1.7, §4 (preguntas restantes + adjudicación
   E5→E6 contra el reporte real), y recalcular §5 con la tabla de `WO-E8-DESIGN.md` §3
   sobre la matriz completa. Actualizar §2/§3/§6 solo si hay citas nuevas.
3. Este archivo ya cita cada claim presente con file:line — mantener INV-E8.

## §8. Glosario OMEGA

- **TLS (Temporal Liquidity Superposition):** flash loan / capital prestado.
- **Holonomic Loop Resolution:** arbitraje triangular.
- **Topological Yield:** beneficio neto / valor extraído.
- **Decoherencia de Estado:** slippage.
- **Variedad de Liquidez:** pool / DEX.

---
*Fail-honest R8: 2/7 inputs incorporados con cita, 5 declarados INPUT MISSING, veredicto
NO-ADJUDICABLE en vez de fabricado. 0 HTTP propios / 0 git / 0 mutación VPS / 0 código
de producción editado.*
