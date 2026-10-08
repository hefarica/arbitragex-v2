# FUND-OUTCOME-RESAMPLE-01 — el veredicto (b) aguanta 6× más volumen

**Tarea:** t107 · **run_id:** `arbx-entrega-20261007` · **permisos:** SOLO LECTURA (nada reiniciado, reparado, mergeado ni desplegado).
**`main` medido:** `9ccd1d04beb433073e19b2184cdfa0f98bd14e5a` (= SHA_BASE del briefing) · **revisión servida DENTRO del contenedor:** `9ccd1d04…`.
**Alcance:** único path escrito `docs/sre/`.

---

## 0. Veredicto en una línea

**CAUSA (b) CONFIRMADA con margen.** Con **2.195 eventos** (6,0× la muestra previa) en **30,7 min continuos**, el mix es **`verify_mismatch` 65,06 % · `balance_unreadable` 18,68 % · `slot_unresolved` 16,26 %** y **`write_rejected` AUSENTE** ⇒ la escritura **no** se rechaza (es efectiva) y la verificación del centinela **no reproduce** ⇒ **el fix tiene que apuntar a `balanceOf`/centinela, no al write.**

## 1. O1 — El mínimo, declarado ANTES de mirar (y cumplido con margen)

Declaración, puesta como **primera línea de la captura, antes de cualquier consulta**:

```
MINIMO EXIGIDO, DECLARADO ANTES DE MIRAR (O1):
  >= 2000 eventos acumulados desde el ultimo reinicio de sim-ctl
  Y >= 30 minutos continuos desde esa marca
  Si no se cumple al primer corte -> se espera. Si el reinicio cae
  a mitad de muestra -> se parte o se descarta.
```

**Primer corte (03:00:31Z): 695 eventos / 9,1 min ⇒ NO cumplía** (lo digo porque el primer corte se hizo y se declaró insuficiente, no porque se haya elegido después). Se **esperó** con un muestreador que exigía ambas condiciones a la vez. **Corte válido (03:22:08Z): 2.195 eventos / 1.843 s = 30,7 min** ⇒ **cumple las dos**.

Tasa implícita de la ventana: **2.195 / 1.843 = 1,19 eventos/s ≈ 71,5/min**.

## 2. O2 — El REINICIO como parte del instrumento (y su artefacto, medido dos veces)

| dato | valor |
|---|---|
| `sim-ctl` `State.StartedAt` | **`2026-10-08T02:51:25.24075118Z`** |
| revisión horneada | **`9ccd1d04beb433073e19b2184cdfa0f98bd14e5a`** |
| `searcher-rs` `StartedAt` | `2026-10-08T02:51:25.243187219Z` |
| identidad al INICIO de la captura | `STARTEDAT_A=…02:51:25.24075118Z` |
| identidad al FIN de la captura | `STARTEDAT_B=…02:51:25.24075118Z` |
| veredicto | **`RESTART_MID_SAMPLE=NO`** (instrumento estable: la muestra NO se parte ni se descarta) |

**El artefacto del cruce, medido en los DOS regímenes** (es la prueba de que el reinicio no es ruido):

| momento | ventana de `increase([30m])` | `slot_unresolved` | `balance_unreadable` | `verify_mismatch` |
|---|---|---|---|---|
| **primer corte 03:00:31Z** (la ventana **cruza** el reinicio de 02:51:25) | [02:30:31, 03:00:31] | **322,05** vs crudo **110** (×2,9) | **142,52** vs **145** | **435,71** vs **440** |
| **corte válido 03:22:08Z** (la ventana **no** cruza) | [02:52:08, 03:22:08] | **348,91** vs crudo **357** | **398,32** vs **410** | **1.395,63** vs **1.428** |

**Lectura:** cuando la ventana de `increase([30m])` **cruza** el reinicio, el valor mezcla **pre-deploy con post-deploy** y sobreestima (`slot_unresolved` 322 contra 110 reales: el 66 % de ese número venía del contenedor anterior). Cuando no lo cruza, `increase` y el crudo coinciden dentro de la fracción de minuto que quedó fuera de la ventana. **Por eso la ventana válida es el CRUDO acotado al `StartedAt`, y así se reporta.**

## 3. O3 — Controles de canal PRIMERO, con exit code literal

```
command -v curl     -> /usr/bin/curl     RC_CURL=0
command -v docker   -> /usr/bin/docker   RC_DOCKER=0
Prometheus count(up) -> {"result":[{"value":[1791429728.657,"10"]}]}   rc=0     (10 targets vivos)
Postgres SELECT 1    -> 1                RC_PSQL=0
```

**Los ceros y las ausencias de este informe salen de instrumentos probados vivos**: el `write_rejected` ausente es una ausencia dentro de un canal que contestó 10 targets y devolvió fila en Postgres.

## 4. O1/O4 — El desglose, ventana válida (crudo desde el reinicio), y qué significa el cero

`sum by (outcome) (arbx_sim_funding_total)` sobre la ventana válida:

| outcome | eventos | proporción | t106 (365 ev) | delta |
|---|---|---|---|---|
| **`verify_mismatch`** | **1.428** | **65,06 %** | 61,4 % | **+3,7 pp** |
| **`balance_unreadable`** | **410** | **18,68 %** | 23,3 % | **−4,6 pp** |
| **`slot_unresolved`** | **357** | **16,26 %** | 15,3 % | **+0,9 pp** |
| **`write_rejected`** | **0 — SERIE AUSENTE** | — | 0 (ausente) | — |
| **TOTAL** | **2.195** | 100,00 % | 365 | ×6,0 |

**El mix es el mismo, con el doble de resolución.** Tres puntos porcentuales de variación sobre una muestra 6× mayor es estabilidad, no deriva: **la causa (b) no era un artefacto de ventana chica**.

**O4 — el cero de `write_rejected` es INFORMATIVO, no ausencia.** Medido con el inventario de series: `count by (outcome) (arbx_sim_funding_total)` devuelve **exactamente 3 series** (`slot_unresolved`, `balance_unreadable`, `verify_mismatch`, cada una con valor 1) ⇒ **la serie `write_rejected` no existe en la exposición**. Y sus tres hermanos **sí** cuentan, sobre el mismo contador, el mismo instrumento y la misma ventana (2.195 eventos, `count(up)`=10, `SELECT 1` vivo). **Un cero con productor probado es un dato; un cero sin productor no lo sería.** No los tres en cero ⇒ **NO COMPUTADO no aplica**.

## 5. O5 — La otra superficie (Postgres), SIN mezclarla, y con el corte anclado al reinicio

**El agregado miente** (ya medido en t93): la tabla entera tiene **639.507 filas** y está dominada por las familias PRE. El corte anclado a `02:51:25Z`:

| época | clase de `fail_reason` | n | % de su época |
|---|---|---|---|
| **POST** | `sim_signer_funding_slot_unresolved` | **360** | **49,66 %** |
| **POST** | **`candidate_incomplete:amount_in_wei_zero`** | **344** | **47,45 %** |
| **POST** | `build_error: router not in catalog for chain=1 dex=unknown` | 21 | 2,90 % |
| | **TOTAL_POST** | **725** | 100,00 % |
| PRE | `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` | 576.907 | — |
| PRE | `strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb` | 23.743 | — |
| PRE | `strategy_cyclic_route_not_simulatable_in_s4:triangular` | 16.920 | — |
| PRE | `sim_signer_funding_slot_unresolved` | 20.971 | — |
| PRE | `candidate_incomplete:amount_in_wei_zero` | 87 | — |
| PRE | `build_error: amount invalid: zero amount_in` | 144 | — |
| PRE | `build_error: router not in catalog for chain=1 dex=unknown` | 5 | — |
| PRE | `fork_acquire_failed` | 3 | — |
| PRE | `funding_balanceof_timeout` | 1 | — |
| PRE | `anvil_setStorageAt: … tcp connect error: Connection refused (os error 111)` | 1 | — |

**Las dos superficies contestan cosas distintas y NO se mezclan:**
- **Prometheus** (`arbx_sim_funding_total` por `outcome`) describe **los intentos de fondeo y su modo de fallo**: 65,06 % verificación que no reproduce, 18,68 % balance ilegible, 16,26 % slot irresoluble. **Es la superficie que contesta (a) vs (b).**
- **Postgres** (`simulations.fail_reason`) describe **las filas de simulación**: 49,66 % detenidas por el fondeo del signer y 47,45 % por candidato incompleto (`amount_in_wei_zero`). **Es la superficie que da el cuadro de la tabla.**

**Observación nueva, medida y NO investigada acá** (fuera del alcance de esta orden): **`candidate_incomplete:amount_in_wei_zero` = 344 filas = 47,45 % del POST**, cuando en el corte POST de t93 (deploy anterior) esa clase era **0**. Es una clase que **apareció después de este deploy** y que ningún otro artefacto de esta campaña había medido. Se declara para que la célula decida si es una unidad aparte; **no se interpreta**.

## 6. O6 — `passed=true` en la ventana nueva: NO APLICABLE con su razón

```
PASSED_TRUE_POST  = 0   (rc=0)      -- ventana desde el reinicio
PASSED_TRUE_TABLA = 0   (rc=0)      -- tabla entera
```

**NO APLICABLE**: no hay ni una fila con `passed=true` en la ventana nueva, así que **no hay nada que trazar** (la integridad de la métrica de t93 E9 seguiría siendo la regla cuando aparezca la primera). Y se declara lo que el contrato exige: **un `passed=true` aislado no se interpretaría como confirmación ni refutación** — el caso no se da.

## 7. O7 — Alcance

**Solo lectura.** Nada reiniciado, nada reparado, nada mergeado, nada desplegado, **nada escrito en el VPS**. El único path escrito es este documento. Las consultas usadas fueron `docker inspect`/`psql -tAc SELECT`/`GET` a la API de Prometheus.

## 8. Sobre la medición previa (t106): de dónde salen sus números

Los valores de comparación (365 eventos, 224/85/56, ventana de 4 min) se citan **del registro de la tarea t106** (el brief y su reporte). **Declaro que su artefacto NO está manifestado en el remoto**: revisé `origin/main:docs/backend/` (10 archivos: `SIM-FUND-02.md`, `SIM-FUND-02b.md`, `SIM4-CYCLIC-01.md`, …) y `origin/main:docs/release/` (5 archivos) y **ninguno contiene el desglose por `outcome`**. Si esa medición debe ser citable como fuente primaria y no como reporte, **su autor tiene que manifestarla** (es la doctrina D9 que la célula ya aplicó en t92/t98). Acá se la usa solo como punto de comparación, y se dice de dónde viene.

## 9. Qué habilita este resultado (y qué NO)

- **Habilita** diseñar el arreglo sobre `balanceOf`/centinela: el write **no** se rechaza (`write_rejected` ausente en 2.195 eventos) y el fallo está del lado de la **verificación del centinela** (65,06 %) y de la **lectura del balance** (18,68 %). La causa (b) queda **confirmada con 6× el volumen**.
- **NO habilita** concluir sobre `candidate_incomplete:amount_in_wei_zero` (47,45 % del POST en la tabla, 0 en el corte del deploy anterior): es una clase nueva, medida acá por primera vez, y **no está interpretada**.
- **NO habilita** tratar la ventana de 30,7 min como régimen permanente: la duración del contenedor es la que manda (§2), y **cada deploy la reinicia**.

## 10. Reproducción

```bash
# O1: el minimo se declara ANTES; primer corte y corte valido
ssh arbx "docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1"
ssh arbx "curl -s --data-urlencode 'query=sum by (outcome) (arbx_sim_funding_total)' http://127.0.0.1:9090/api/v1/query"
ssh arbx "curl -s --data-urlencode 'query=count by (outcome) (arbx_sim_funding_total)' http://127.0.0.1:9090/api/v1/query"
# O2: el cruce del reinicio
ssh arbx "curl -s --data-urlencode 'query=sum by (outcome) (increase(arbx_sim_funding_total[30m]))' http://127.0.0.1:9090/api/v1/query"
# O3: controles
ssh arbx "command -v curl; command -v docker; docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'"
# O5/O6: la otra superficie, con el corte anclado al reinicio
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \"SELECT CASE WHEN simulated_at >= '2026-10-08T02:51:25.24075118Z' THEN 'POST' ELSE 'PRE' END, coalesce(fail_reason,'<NULL>'), count(*) FROM simulations GROUP BY 1,2 ORDER BY 1,3 DESC\""
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \"SELECT count(*) FROM simulations WHERE passed = true AND simulated_at >= '2026-10-08T02:51:25.24075118Z'\""
```
