# BR-00 — VERIFY adversarial del P0 (D-SIM-01)

> **WO:** BR-00-VERIFY · **kind:** verify · **agente:** cs-validator (Gang Omniscience)
> **Fecha:** 2026-09-07 · medición 14:24Z→14:48Z (VPS en UTC, verificado `date -u`).
> **Método:** read-only TOTAL. `ssh arbx`: `psql SELECT` sobre simulations/opportunities,
> `redis-cli` read (XLEN/XRANGE), `docker inspect`/`ps` read-only. HTTP público: **0/5 usados**.
> Local: lectura de código del crate sim-ctl completo + `cargo test -p sim-ctl`.
> **SHA desplegado en VPS:** `e65040f1` (merge PR #555) — `git rev-parse HEAD` en
> `/opt/arbitragex-v2`, sin cambios desde 12:46:44Z. Working tree local = `27aca289`
> (ancestro directo de e65040f1; `git diff e65040f1 27aca289 -- backend/sim-ctl/` = vacío:
> lo desplegado ES lo que leí).

---

## 0. VEREDICTO: **BLOCKED** — BR-00-APPLY no existe; el P0 sigue roto

**El charter pedía citar `BR-00-APPLY.md` línea por línea, refutada o confirmada. No hay
líneas que citar: el archivo no existe.** Evidencia de la ausencia (verificada 3 veces,
última 14:48:06Z):

1. `audits/cerebro-2026-09-07/` contiene solo `BR-01-DESIGN.md`, `BR-01-FORENSE-EMBUDO.md`,
   `BR-05+06-VERIFY.md`, `GOAL-WORKORDERS.md`. Cero archivos BR-00-* (find sobre
   `audits/` y `.claude/worktrees/` = vacío).
2. `git status backend/sim-ctl/ backend/searcher-rs/ backend/selector-api/` = 0 diffs
   (los únicos cambios backend del árbol son control-board/api-server de otro WO).
3. Cero marcadores `BR-00`/`WO-BR` en `backend/sim-ctl/src/` (grep).
4. Board (09:07 local / 13:07Z): BR-00 sigue **PENDIENTE — prioridad sobre todo**.
5. VPS: SHA sin mover (e65040f1 desde 12:46:44Z) — tampoco aterrizó por deploy.

**Consecuencia (charter): FAIL = BLOCK — el gang NO avanza a BR-02 con el P0 roto.**
El apply del P0 o cayó antes de escribir nada, o nunca despachó. Procedimiento
RESPAWN-2 (orden operador 2026-09-06): el orquestador debe re-despachar BR-00-APPLY
(2 reemplazos con tarea subrepartida si el builder cayó), y ESTE verify se re-ejecuta
inmediatamente después, mismo serial rust.

**Lo que este reporte SÍ entrega** (todo verificable, RULE 00): (a) el censo independiente
que valida el diagnóstico del P0, (b) el baseline RULE 00 de los handlers ACTUALES contra
el que se juzgará el diff del builder, (c) el protocolo de métrica honesta con el peligro
de mix-shift MEDIDO (no teórico), (d) un hallazgo nuevo (anomalía dex_arb) con gap de
observabilidad que el fix DEBE cerrar, (e) cargo test verde del crate, (f) §34.3 intacto.

---

## 1. Censo INDEPENDIENTE (charter §1) — cruces con BR-01-FORENSE-EMBUDO.md

Ventanas mías (declaradas, INV-BR01-3): **2h** = [12:24Z, 14:24Z] (era post-#555 casi
pura), **24h** = [14:24Z−24h, 14:24Z]. Ventana de BR-01: 24h con snapshot 12:51:06Z +
post-deploy 13:0xZ. Mis queries son re-ejecutables tal cual (§6).

### 1.1 Kinds emitidas × kinds simulables (la matriz del P0)

| Magnitud | Mi valor (2h, 12:24→14:24Z) | BR-01 (su ventana) | Cruce |
|---|---|---|---|
| Kinds DISTINTAS emitidas (opportunities) | **40** | 40 | ✅ idéntico |
| Total opportunities | 245,480 (≈123K/h) | 60,302/24h + 9,283/14min | consistente: mi ventana es 1.5h más post-#555 y el rate rampó (ver §1.3) |
| Kinds SIMULABLES (gate tx_builder) | **1** (`dex_arb`) | 1 (`dex_arb`) | ✅ confirmado en código `tx_builder.rs:45-46` |
| Cobertura por kind | 1/40 = **2.5%** | — | el desajuste D-SIM-01 es real |
| Cobertura por volumen | dex_arb = 208,039/245,480 = **84.7%** del flujo | 70.6% (14min post-deploy) | el mix se concentró AÚN más en la única kind simulable |

### 1.2 Sims por razón (mi ventana 2h — era post-#555 limpia)

Total sims 2h = **4,014** · passed = **0** (sin fila (PASS)).

| Razón (prefijo) | simulator | Filas | % de 4,014 |
|---|---|---|---|
| `strategy_not_simulatable_in_s4` | not_implemented | 2,668 | **66.47%** |
| `reverted` (TRANSFER_FROM_FAILED/STF/INSUFFICIENT_OUTPUT) | anvil | 812 | 20.2% |
| `build_error` (zero amount_in 166 · PancakeSwap V3 catalog 88) | anvil | 254 | 6.3% |
| `sim_timeout` | anvil | 214 | 5.3% |
| `rpc_error` (free-plan 408/500 del fork RPC) | anvil | 62 | 1.5% |
| `fork_acquire_failed` | anvil | 4 | 0.1% |

`XLEN arbx:opps:simulated` = **0** (14:24Z) — el terminus sigue muerto, 0 passed jamás.

### 1.3 Cruce de conteos vs BR-01 (tolerancia charter: >5% = hallazgo)

| Métrica | BR-01 (24h, snapshot 12:51Z) | Yo (24h, snapshot 14:24Z) | Divergencia | Lectura |
|---|---|---|---|---|
| Sims totales 24h | 37,723 | 36,350 | **−3.6%** | < 5%: deriva de ventana (1.55h antigua rueda fuera; BR-01 §8.2 declaró ±1-2%, mi delta extra = varianza del consumo) — NO hallazgo |
| `strategy_not_simulatable_in_s4` | 36,422 (96.55%) | 33,874 (**93.21%**) | ratio −3.34pp | **explicado por mix de eras**, no por error: mi 24h contiene 1.75h más del flujo post-#555 (dex_arb-dominante, la única kind simulable) |
| passed | 0 | 0 | 0 | invariante |

**CONFIRMO el censo de BR-01 §3.3 y su entrega a BR-00 (§9):** el catálogo S4 soporta
EXACTAMENTE una kind (`dex_arb`, `tx_builder.rs:45-46`) de las 40 emitidas. Cero
divergencias no explicadas — el diagnóstico del P0 queda firme bajo medición independiente.

### 1.4 HALLAZGO CRÍTICO para la métrica de éxito: el mix-shift SOLO ya movió el número

En mi ventana 2h (era post-#555 limpia, SIN ningún fix desplegado — SHA no se movió):

> **96.55% (24h mixta, BR-01) → 66.47% (2h post-#555) de `strategy_not_simulatable_in_s4`
> SIN QUE NADIE TOCARA EL SIMULADOR.**

El #555 cambió el mix de entrada (dex_arb 84.7% del flujo) y la métrica del P0 mejoró
30 puntos sola. BR-01 §9 lo advirtió ("tu porcentaje puede mejorar SOLO por cambio de
mix: mide también el conteo absoluto por kind") — hoy está MEDIDO. Consecuencia dura para
el gate de éxito "98.2% → <50%": **ese umbral está a ~16pp de alcanzarse por drift de mix
puro**, y cualquier apply podría reclamarlo sin haber arreglado nada.

**Protocolo OBLIGATORIO de métrica honesta para el re-verify post-apply (exigir al builder):**
1. Ventana post-fix comparable y era-separada (INV-BR01-3): SOLO desde el deploy del fix;
   nunca promediada con era pre-fix.
2. `Σ sims(kind simulable)` en conteo ABSOLUTO por kind, además del share — el share solo
   no prueba cobertura.
3. Medir sobre `simulations.fail_reason` (no sobre opportunities).
4. Verificar que BR-04 (anti-spam) NO haya corrido en la ventana (board: sigue PENDIENTE;
   si correra antes, las supresiones de flood recortarían el denominador y fabricarían
   mejora). Re-ejecutar mis queries §6 y comparar el mix por kind contra mi baseline §1.1.
5. `XLEN arbx:opps:simulated` > 0 con passed>0 es la única prueba de vida del terminus.

---

## 2. Auditoría de handlers ACTUALES (charter §2, RULE 00) — baseline pre-apply

El charter pide auditar "cada handler nuevo o filtro" del builder. Sin builder, establezco
el **baseline del estado actual** — el diff futuro se juzga contra esto. Cobertura: TODO el
crate `backend/sim-ctl/src/` (tx_builder, sim_engine, main, consumer, sim_runner,
revm_backend, anvil_backend, simulator_backend, capabilities, route_lookup, persistence).

### 2.1 El camino de aprobación EXIGE simulación real — no existe auto-aprobación

- **Path anvil (el VIVO en prod, `SIM_BACKEND=anvil` verificado por `docker inspect`
  14:24Z):** `passed=true` solo en `sim_engine.rs:100`, que exige (a) fork anvil presente
  (`:32-38`, si no → `anvil_fork_not_configured` honesto), (b) probe construido por
  `tx_builder.rs:41-87` (V2 `swapExactTokensForTokens` o V3 `exactInputSingle` reales),
  (c) `eth_call` + `estimate_gas` contra el fork (`:74-77`), (d) decode del output y
  slippage ≤ umbral. Revert/timeout/RPC-error → `passed=false` con reason tipado (`:86-94`).
- **Path B2c REVM (inactivo en prod):** `sim_runner.rs:197-215` corre
  `execute_multistep_revm` con `paper_mode=true` + `require_positive_net_profit=true`;
  el spawn falla → `b2c_spawn_blocking_join` (never pass).
- **Terminus:** `XADD arbx:opps:simulated` SOLO si `sim.passed && inserted_fresh`
  (`consumer.rs:459`). El terminus relays-client (§34.3) ni se toca desde aquí.
- **Drain-guard (main.rs:855-874):** con `SIM_BACKEND=revm` y env B2c incompleto, el
  consumer NO se spawnea — rechaza drenar el stream a un backend de calldata vacío.

### 2.2 Greps RULE 00 sobre sim-ctl (re-ejecutar sobre el diff del builder)

| Grep | Resultado hoy |
|---|---|
| `passed: true` hardcode | **0** (los únicos `passed` son propagaciones de outcome/sim) |
| `mock\|stub\|fake` en src (no-test, no-comentario) | **0** — los hits son: comentario de doctrina en `capabilities.rs:3`, un TEST que exige `!result.passed` para el stub (`revm_backend.rs:319`), y el comentario "not a stub" de `sim_runner.rs:5` |
| Marcadores `BR-00`/`WO-BR` | **0** (el builder no tocó nada) |
| Escritores de `simulations` | **1 solo**: `sim-ctl/src/persistence.rs:30` |
| Emisores de `strategy_not_simulatable_in_s4` | **1 solo**: `sim_engine.rs:44` |

**Un handler-mock (CRITICAL por charter) sería:** un match nuevo de kind que retorne
`passed=true` sin `eth_call`/REVM, o un `counted_gap` reclasificado a pass. Hoy NO existe
ninguno. Cuando el builder aterrice: grep su diff por `SimulationResult {` con `passed:`
que no pase por `simulate()`/`execute_multistep_revm`, y por `passed = true` no derivado.

### 2.3 HALLAZGO NUEVO (MEDIUM): kind `dex_arb` recibe `strategy_not_simulatable_in_s4`

**Anomalía viva, 100 rows/6h (95 en mi 2h = 2.4% de las sims), en curso a las 14:34:02Z**
(ultima capturada: opp detectada 14:30:49, simulada 14:33:56 — lag ~3min del consumer,
consistente con backlog 8K). Ejemplo verificado:

```
a6f33f8c-1479-4dba-8aa7-f6ea85930ba6 | strategy_kind='dex_arb' (len 7) | dex_a=UniswapV2
| dex_b=SushiSwap | rejection=non_positive_profit | detected 14:30:49
  → simulations: not_implemented / strategy_not_simulatable_in_s4 @ 14:33:56
```

Por qué es anómala: la ÚNICA ruta a ese reason exige `opp.strategy_kind !=
StrategyKind::dex_arb()` (`tx_builder.rs:45` → `sim_engine.rs:44`), y `StrategyKind` es
newtype String con PartialEq DERIVADO (`shared-rs/src/contracts.rs:15-16`) — 'dex_arb'
pasa el gate. Cadena completa auditada sin transform de kind: emitter INSERT verbatim
(`persistence.rs:166` bind `as_str()`), XADD serializa el MISMO struct (`publisher.rs:155`),
Zod enum estricto sin catch/transform (`shared-ts/src/contracts/strategy-kinds.ts:283`),
selector re-serializa tal cual (`selector-api/src/consumer.ts:217`). Desplegado == leído
(`git diff e65040f1 27aca289` vacío). Ocurre en las TRES generaciones de binario (pre-y
post-#555 y container reiniciado 13:20Z).

**Mecanismo: UNKNOWN honesto.** La entrada del stream correspondiente ya fue trimeada
(XRANGE sobre [14:30Z±100s] = 0 entradas) — no pude capturar el JSON exacto que vio el
consumer. Hipótesis INFERIDA (sin confirmar): divergencia stream↔PG para el mismo id
(doble emisión con kind drift); descartadas: transform Zod, UPDATE de kind (0 sitios),
id determinista (sin evidencia en engines), segundo escritor de sims (grep = 1).

**Gap de observabilidad que la hace invisible (el fix BR-00 DEBE cerrarlo):**
`BuildError::UnsupportedStrategy(StrategyKind)` LLEVA la kind (`tx_builder.rs:46`) pero
`sim_engine.rs:44` la descarta (`UnsupportedStrategy(_)`). El reason no puede decir QUÉ
kind fue rechazada. Cualquier handler nuevo debe emitir
`strategy_not_simulatable_in_s4:<kind>` (o mejor: la kind en el reason de TODOS los
rechazos S8) — sin eso, esta clase de drift es invisible por diseño.

### 2.4 Gates de segundo orden que emergen al abrir la compuerta (para el builder)

Al ampliar la cobertura, estas razones SUBIRÁN (mis numeros 2h): `amount_in_wei=0`
(166 `build_error: zero amount_in`; el sample de stream muestra triangular con
`amount_in_wei:"0"`), `PancakeSwap V3` fuera de catálogo (88), `TRANSFER_FROM_FAILED`
(515 — el signer del probe sin balance/approval), timeout (214). Si el builder declara
éxito solo por share de not_simulatable, estas lo sustituirán — son el siguiente cuello
real de S8 y deben quedar declaradas, no escondidas.

---

## 3. §34.1 mode-invariancia y §34.3 terminus (charter §4)

- **§34.3 INTACTO:** `git diff -- backend/relays-client/` = **0 diffs**. Default-deny y
  `MainnetRefused` presentes y sin tocar en `live_exec_policy.rs:32` (mensaje default-deny),
  `:37` (variante), `:85`, `:124`, `:141`, `:154` (los cuatro return MainnetRefused).
  Nada de este WO (ni censo ni código leído) toca el terminus.
- **§34.1 (para el diff futuro):** el gate S8 es mode-invariante POR CONSTRUCCIÓN hoy
  (sim-ctl consume `arbx:opps:validated` igual en PAPER/TESTNET/LIVE; la diferencia de
  modo vive en relays-client, §34.1.3). Condición de verificación para el re-verify: el
  diff del builder NO debe introducir ramas por modo de trading en sim-ctl (grep
  `ARBX_TRADE_MODE|PAPER_SHADOW|LIVE_MAINNET` en el diff = 0).

---

## 4. cargo test del crate tocado (charter §5) — baseline VERDE

`cargo test -p sim-ctl` (árbol 27aca289, 14:4xZ):

| Suite | Resultado |
|---|---|
| unittests lib | 5 passed / 0 failed |
| unittests main (incluye tx_builder: non_dex_arb_rejected, pascalcase, unknown_dex) | 37 passed / 1 ignored / 0 failed |
| simwire02_pel_recovery | 1 passed (36.3s) |
| simwire02_route_aware | 8 passed / 1 ignored (needs live PG+REVM) |
| simwire02c_redelivery_idempotency | 7 passed |
| **Total** | **58 passed · 0 failed · 2 ignored (live-env, honestos)** |

Espera del re-verify post-apply: mismo comando, mismo verde + tests NUEVOS para cada
handler/kind que el builder agregue (el charter exige que cada handler demuestre
simulación real, no un catalog-lookup que aprueba).

---

## 5. Constraints que el builder (re-despachado) DEBE conocer (evidencia de este verify)

1. **Producción corre `SIM_BACKEND=anvil`** (docker inspect 14:24Z) — un fix que solo
   toque el path B2c REVM es NO-OP en prod hoy. El fix vive en el path anvil
   (tx_builder/sim_engine) O requiere flip de SIM_BACKEND=revm + env B2c completo
   (REVM_RPC_URL + ARBITRAGE_EXECUTOR + FLASHLOAN_EXECUTOR_1 + REDIS_URL) — flip es
   decisión de operador, no del builder.
2. La métrica <50% está comprometida por mix-shift (§1.4) — aplicar el protocolo §1.4(1-5).
3. Incluir la kind en el reason de rechazo S8 (§2.3) — cierre el gap que oculta drifts.
4. `decode_amount_out` (`sim_engine.rs:157-176`) solo decodifica `dex_arb`: una kind nueva
   que llegue a anvil sin decoder → `slippage_pct=None` → `passed=false` (fail-closed,
   correcto) — el builder debe agregar decoders por kind o declarar el límite.
5. BR-04 NO debe correr antes de medir el post-fix (§1.4.4).

---

## 6. Gate de verificación re-ejecutable (mis queries, tal cual)

```bash
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F'|' -c \
 \"SELECT COALESCE(split_part(s.fail_reason,':',1),'(PASS)'), s.simulator, s.passed, count(*) \
  FROM simulations s WHERE s.simulated_at >= now() - interval '2 hours' \
  GROUP BY 1,2,3 ORDER BY 4 DESC;\" \
 -c \"SELECT o.strategy_kind, count(*) FROM simulations s JOIN opportunities o ON o.id=s.opportunity_id \
  WHERE s.fail_reason='strategy_not_simulatable_in_s4' AND s.simulated_at >= now() - interval '2 hours' \
  GROUP BY 1 ORDER BY 2 DESC;\" \
 -c \"SELECT count(DISTINCT strategy_kind), count(*) FROM opportunities \
  WHERE detected_at >= now() - interval '2 hours';\""
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:simulated"
# Baseline de este verify (2h 12:24→14:24Z): 4,014 sims · 66.47% not_simulatable ·
# 0 passed · 40 kinds · XLEN 0. El re-verify post-apply compara contra §1.1/§1.2.
```

---

## 7. Fail-honest — lo que NO pude verificar / declaro UNKNOWN

1. **Mecanismo exacto de la anomalía dex_arb (§2.3):** la entrada de stream fue trimeada
   antes de poder capturar el JSON exacto; INFERRED double-emission-kind-drift, sin
   confirmación. Presupuesto de dominio público 0/5 usado — no gasté requests porque el
   P0 no tiene superficie de UI que verificar (el apply no existe).
2. **22 opportunity_ids con >1 sim en 2h** (redelivery at-least-once del PEL — esperado
   por diseño; no desglosé si alguno es de la anomalía).
3. **El delta 236 entre `not_implemented` 36,658 y `strategy_not_simulatable` 36,422 de
   BR-01 §8.5:** no lo re-medí — su explicación (otras razones not_implemented) sigue
   plausible y no afecta el veredicto.
4. **Sesiones pares:** ListAgents mostró 3 sesiones interactivas (12h) — no pude
   determinar si alguna era el builder BR-00-APPLY caído o vivo; a 14:48Z nada aterrizó.

---

## 8. Entrega a la mesa (downstreams)

- **Orquestador (URGENTE):** BR-00-APPLY no existe — re-despachar (RESPAWN-2 si cayó: 2
  reemplazos, tarea subrepartida, serial rust). El BLOCK de este verify se levanta SOLO
  con: apply aterrizado + marcadores `// BR-00 (2026-09-07)` + re-verify verde con
  protocolo §1.4 + cargo test §4 + greps §2.2 sobre el diff.
- **BR-00-APPLY (cuando exista):** leer §5 completo (SIM_BACKEND=anvil es la constraint #1)
  y §2.3 (kind en el reason). Tu censo de entrada ya está cerrado y validado (§1) — no lo
  re-derives, gasta el presupuesto en handlers reales.
- **BR-01:** tu §3.3/§9 confirmados línea por línea bajo medición independiente; tu
  advertencia de mix-shift ahora tiene número (96.55→66.47 sin fix) — INV-BR01-3 se
  demuestra necesaario para el gate del P0.
- **BR-02:** el gang no debe avanzar hasta que el P0 aterrice (charter). Cuando corra,
  tu `missing_reserves` seguirá siendo el asesino S2/S4 con 55.7% had_reserves=f.
- **BR-05 (calibración):** sigue gated a post-sim-viable ≥5% — hoy 0 passed; nada que
  calibrar todavía (coherente con BR-05+06-VERIFY).
- **Board:** BR-00 sigue PENDIENTE; este verify = BLOCKED-by-absence (no PENDIENTE del
  verify: el verify CORRIÓ y su resultado es que no hay nada que verificar).

**Firma de honestidad (RULE 00):** cada cifra de este reporte viene de un SELECT/XLEN/
inspect ejecutado por mí con ventana declarada, o de línea de código citada. Nada fue
inferido donde se pudo medir, y lo no medido está en §7.
