# FASE 1 — VEREDICTO: criterio de muerte sec7 con datos reales de la DB

Fecha de medición: **2026-10-23 13:37–13:43 UTC** · Autor: verificador (t12) · Repo: `hefarica/arbitragex-v2`
Base: `origin/main` = `6bde300e8b14434894e23885bd5b7fd927dc4c06` (incluye los 6 merges de la fase y `FASE1_GATE.md` #1027).
Insumo: `docs/rea/FASE1_GATE.md` (t11), `docs/rea/FASE1_BASE.md` (t1), `docs/rea/REA_MASTER.md` (§0/§3/§8), fichas REA-1/2/4/trace.

> **Este documento no toca código, ni config, ni DB.** Toda cifra sale de `psql` de **solo lectura** sobre la PG viva del VPS
> (`195.201.235.70`, `arbitragex-v2-postgres-1`, DB `arbitragex`) por WSL→ssh. Cada número lleva en la misma línea el id de la
> query (Q1…Q7) cuyo SQL literal y salida cruda están en **§7**. Sin id de query, un número no se reporta.

---

## 0. Veredicto en una frase

**El criterio de muerte de sec7 no se activa hoy porque su antecedente es falso: no hay filas `executed` (0), así que "hay executed pero cero rentables" es INDECIDIBLE, no un diagnóstico.** Lo que sí está medido es que la fase murió **tres etapas antes** de esa pregunta, y por tres causas que se pueden separar y cuantificar: (1) **gate de modo** — paper mode ⇒ 0 difusiones ⇒ 0 ejecuciones por diseño; (2) **pisos de config inalcanzables con la clase de ruta medida** — el piso `min_profit_usd=$50` sobre `capital_usd=$1000` exige **500 bps** de dislocación y la dislocación máxima medida en WETH/USDC (la única clase con datos) fue **59.97 bps** ⇒ **8.3× de brecha**, independiente de paper mode; (3) **capacidad** — **0 de 516,059** simulaciones `passed` en toda la tabla, con el 76.2 % de los fallos en `candidate_incomplete`: la tubería nunca produjo una fila apta para ejecutar. En consecuencia: **no es correcto declarar "problema de selección de mercado" todavía, ni abrir FASE 2 (PROTECTIONS/DETERMINISTIC/PER-LEG/CONNECTORS): ninguna de esas unidades mueve las tres restricciones activas.** Si el operador quiere conservar pisos y capital, el barrido de nichos es la decisión correcta — pero como **cambio de nicho con requisitos de capacidad concretos** (§5), no como conclusión del criterio sec7.

---

## 1. Ventanas y método

| Ventana | Definición | Filas | Fuente |
|---|---|---|---|
| **W1 post-deploy** | `detected_at >= '2026-10-22T13:59:50Z'` (deploy `af331ce7` completó 13:59:50Z, `FASE1_GATE.md` §2) → medición 2026-10-23T13:37Z = **23.6 h** | **2315** | Q1 |
| **W0 pre (2 h)** | `>= '2026-10-22T11:59:50Z' AND < '2026-10-22T13:59:50Z'` | **62** | Q1/Q6 |
| Retención | `min(detected_at)` = `2026-10-11 04:27:10Z`; `opportunities` total hoy = **515,989** (la base de 4.8 M de `FASE1_BASE.md` ya fue podada por retención) | — | Q1 |

Transporte reproducible (el `ssh` de Windows falla exit 255; `sql_query` del harness no ejecuta SQL — ambas limitaciones ya declaradas en `FASE1_GATE.md` §3 y §5):

```bash
# WSL (bash), clave con chmod 600 en ~/.ssh
ssh -i ~/.ssh/arbx_hetzner -o BatchMode=yes root@195.201.235.70 \
  "docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -q -f -" < query.sql
```

---

## 2. (1) Filas `executed` vs filas rentables (neto > 0 tras fees+gas)

| Métrica (W1, 2315 filas) | Valor | Query |
|---|---|---|
| `status='executed'` | **0** | Q1 |
| `status='simulated'` | 0 | Q1 |
| `status='rejected'` | **2315 (100 %)** | Q1 |
| `net_expected_profit_usd > 0` | **1** (0.043 %) | Q7 |
| `net_expected_profit_usd = 0` | 0 | Q7 |
| `net_expected_profit_usd < 0` | 41 (1.77 %) | Q7 |
| `net_expected_profit_usd IS NULL` | **2273 (98.2 %)** — *no computado, NO cero* | Q7 |
| `expected_profit_usd > 0` (bruto) | 62 (2.68 %) | Q1 |
| Filas con `dex_b` no nulo | **122 (5.27 %)** vs **0/62** en W0 (control PR1) | Q6 |
| `executions` (tabla) | **0 filas** | Q1 |
| `paper_trade_runs` (tabla) | **0 filas** | Q1 |
| `simulations` enlazadas a W1 | 2266 filas, **`passed`=0** | Q2 |
| `simulations` all-time `passed` | **0 / 516,059** | Q2 |

Razones de rechazo en W1 (Q2): `TokenNotAllowed` **2273**, `NegativeNetProfit` **42**.

### 2.1 La única fila con neto positivo — y por qué su etiqueta miente

`id=eb057d32-2f1b-417d-a68c-e8db38e13f1e` (Q3): `patterns` / `dex_arb`, WETH↔COMP, `uniswap-v2 → uniswap-v3`,
`amount_in` 0.020083833 WETH, **bruto $0.94823370**, **neto $0.345481**, ROI 0.2582 %, `route_metadata.fee_tier_bps=[30,3000]`,
`economics = NULL`, `rejection_reason = NegativeNetProfit`.

El neto es positivo y aun así la fila se etiqueta `NegativeNetProfit`: la etiqueta está **conflacionada en código** —
`backend/prioritization-spine/src/config_aware.rs:1234-1235` mapea `RiskRejectionReason::NetRoiTooLow => RejectReason::NegativeNetProfit`.
Con el config vivo (`min_roi_pct = 0.3000 %`, Q7) el ROI de 0.2582 % queda por debajo del piso de ROI y se publica con la etiqueta del
piso de neto. **Consecuencia honesta:** hay **1** fila neta-positiva en 23.6 h — prueba de que la cadena sizing→net puede dar positivo —
pero está **145× por debajo** de `min_profit_usd = $50` y la etiqueta que la entierra no describe su causa (FOLLOWUP de nomenclatura).

### 2.2 Las 54 filas brutas-positivas que mueren en la allowlist (no en la economía)

Q6: 54 filas con `expected_profit_usd > 0` y `rejection_reason = TokenNotAllowed:<token>`; **suma bruta $13,657.61**, máximo **$1,272.10**,
**12 tokens distintos** (top: `0xb2617246…` 29 filas / $11,554.45; `0x8ed97a63…` 1 / $610.33; `0xf94e7d07…` 2 / $396.90).
`net_expected_profit_usd` es **NULL en las 54** y `economics` es `{"computation_status":"error","error_reason":"token_not_allowed", …}`
(§3.2 del apéndice): **el neto no está computado, luego el bruto NO es un beneficio verificado** (R8: `None ≠ Some(0)`).

---

## 3. (2) Dislocación medida WETH/USDC vs fees reales

Población medible (Q5): solo **2 de los 8 pools WETH/USDC** tienen snapshots de reservas en W1 — **Uniswap V2 `0xb4e16d01…`** y
**SushiSwap `0x397ff154…`**, ambos `fee_tier=30` (0.30 %), **5,300 snapshots cada uno** en 23.6 h (≈1 cada 16 s).
Los 6 pools V3 WETH/USDC (`fee_tier` 100/500/3000/10000) tienen **cero snapshots, nunca** (Q6 `LASTSNAP`: `max(timestamp)=NULL`);
en total **564 pools activos nunca fueron muestreados** y solo **1,436 de 2,000** activos (71.8 %) tienen snapshot en la última hora (Q6).

Dislocación por minuto (último snapshot de cada pool en cada minuto, |p₁/p₂−1|×10⁴, **1,426 pares-minuto**, Q5/Q7):

| estadístico | bps |
|---|---|
| media | 5.91 |
| mediana | **2.85** |
| p25 / p75 / p95 | 2.01 / 7.09 / 19.20 |
| mín / **máx** | 0.02 / **59.97** |
| minutos > 31 bps | **24 (1.68 %)** |
| minutos > 40 bps | 5 |
| minutos > 59.91 bps (costo de la ida y vuelta V2+V2, `1−(1−0.003)²`) | **1 (0.07 %)** |

Top de la ventana (Q7): `2026-10-22 16:56Z 59.97` · `16:57 53.68` · `16:58 43.05` · `16:59 43.05` · `17:00 42.09` · `17:01 36.65` · `17:02 35.32` · `17:03 35.32`.

### 3.1 Contraste con el control del operador (23.5–27.0 bps vs 31 bps de fees)

- **Coincidencia direccional, no numérica.** El control del operador (dislocación 23.5–27.0 bps contra 31 bps de fees, negativo en los
  tres bloques) describía una ruta **V2 (30 bps) + V3 0.01 % (1 bp) = 31 bps**: esa base calza con los `fee_tier_bps=[30,100]` que la DB
  muestra en 9 de las 10 filas brutas-positivas mayores (Q6 `FEE_META`). La lectura de `100` como *fee crudo v3* (0.01 %) y no como 100 bps
  (1 %) está respaldada por `pools.fee_tier`, cuyos valores observados en WETH/USDC son `{30, 100, 500, 3000, 10000}` — exactamente la escala
  de tiers de Uniswap v3 (0.01 %/0.05 %/0.30 %/1 %), con el 30 de las V2 en bps. Sobre esa base, mi medición da **24/1426 minutos (1.68 %)
  por encima de 31 bps** — la dislocación **sí cruza** el nivel de fees de esa ruta, pero solo ~1.7 % del tiempo y por poco.
- **Lo que NO pude reproducir**: el número 23.5–27.0 bps exacto y su ventana. La pata V3 de esa ruta **no tiene reservas muestreadas**
  (cero snapshots ever), así que el cociente V2↔V3 no es computable desde la DB. Declarado: **no reproducido**, no refutado.
- **Lo que sí es más estricto que el control**: para la única pareja con datos (V2↔V2) el costo es **59.91 bps**, y la dislocación supera ese
  nivel en **1 minuto de 1,426 (0.07 %)**, con un edge de **0.06 bps** ($0.006 sobre $1000). Es decir: en la clase V2↔V2 el arb de WETH/USDC
  es estructuralmente negativo, con una ventana de un minuto en 23.6 h.

### 3.2 La aritmética que decide: pisos de config vs dislocación disponible

Config viva, chain 1, `enabled=t` (Q7 `CFG`): `capital_usd=$1000`, `min_profit_usd=$50`, `min_roi_pct=0.3000 %`, `lp_fee_default_pct=0.0030`,
`max_slippage_pct=0.0050` (unidad **no establecida desde la DB**: se reporta verbatim), 8 estrategias, **21 tokens con precio**.

| Cálculo | Resultado |
|---|---|
| Piso neto exigido en bps sobre el capital | `50/1000` = **5.00 % = 500 bps** |
| Piso de ROI exigido | **30 bps** ($3 sobre $1000) |
| Dislocación **máxima** medida (mejor minuto de 23.6 h) | **59.97 bps** |
| Brecha contra el piso de neto | **8.34×** |
| Mejor caso con base de fees 31 bps (si la pata V3 fuera 0.01 %) | `59.97 − 31 = 28.97 bps` = **$2.90** sobre $1000 |
| ¿Ese mejor caso pasa el piso de ROI (30 bps)? | **NO** — 28.97 < 30 bps (falla por 1.03 bps) |
| ¿Pasa el piso de neto ($50)? | **NO** — $2.90 vs $50 (**17×**) |
| La única fila neta-positiva ($0.3455) | 145× por debajo de $50 |

**Conclusión de §3.2:** con `capital_usd=$1000` y `min_profit_usd=$50`, **ningún** minuto medido de la única clase de ruta con datos puede
pasar los pisos — ni con fees de 0 bps. El problema, en esta ventana, **no es que la dislocación sea negativa después de fees: es que el
tamaño del capital y el piso de beneficio están dimensionados para un mercado que no existe en WETH/USDC 2-pata.**

---

## 4. (3) Criterio de muerte sec7 aplicado

**Texto del criterio (plan del operador, sec7, citado en el encargo de t12):** *si hay `executed` pero CERO rentables, el problema es
selección de mercado, no capacidad ⇒ barrido de nichos; si hay rentables ⇒ FASE 2.*

| Antecedente | Medición | Estado del criterio |
|---|---|---|
| ¿Hay `executed`? | **0** (W1 y all-time), `executions`=0 filas, `paper_trade_runs`=0 filas | **Antecedente FALSO** |
| ¿Cuántos "rentables"? | 1 de 2315 neto>0 (0.043 %), 2273 **no computados** | **No hay población rentable** |

⇒ **El criterio sec7 es INDECIDIBLE con estos datos** (no "selección de mercado", no "FASE 2"). Declarar cualquiera de las dos ramas
sería leer una etiqueta como otra cosa. Lo que la evidencia sí decide, en orden de tamaño:

1. **Gate de modo (bloqueo absoluto, por diseño).** El único escritor de `status='executed'` es el relayer tras inclusión on-chain
   (`backend/relays-client/src/persistence.rs:47-54`, `submit_engine.rs:854-883`) y producción corre en paper
   (`paper_mode=true` — verificado hoy en `origin/main:configs/app.toml`; `ARBX_LIVE_EXEC_ENABLED=False`, Redis `arbx:papermode:1`, t11 §3.3).
   Sin difusión no hay inclusión ⇒ `executed=0` **no es un síntoma, es el diseño**. Revertir esto es decisión del operador sobre una ruta protegida (G5).
2. **Pisos inalcanzables con la clase de ruta medida (§3.2):** 500 bps exigidos vs 59.97 bps disponibles. Es un desajuste
   **capital/piso/nicho**, arreglable por config del operador (bajar piso o subir capital) o por cambio de nicho — **no** por más unidades REA del mismo motor.
3. **Capacidad de simulación:** **0 / 516,059** `passed` (Q2). Familias all-time: `candidate_incomplete` **393,485 (76.2 %)**,
   `reverted` 64,105 (12.4 %), `rpc_error` 31,032 (6.0 %), `sim_signer_funding_slot_unresolved` 18,068 (3.5 %), `build_error` 4,758 (0.9 %),
   `sim_timeout` 3,133, `funding_balanceof_timeout` 1,324, `route_path_not_representable` 122. En W1: 2,266 sims, `passed`=0, y el reparto
   cambia (`rpc_error` 957, `build_error` 541, `sim_signer_funding_slot_unresolved` 322, `reverted` 313) — **infra/RPC y armado del payload,
   no reverted de mercado**. Aunque el operador flipeara `paper_mode`, **no hay una sola fila que haya pasado simulación**.

### 4.1 Recomendación (decisión, no hedging)

**Orden correcto, con el costo de cada paso:**

| # | Acción | Ataca | Por qué primero |
|---|---|---|---|
| A1 | Cerrar `candidate_incomplete` (76.2 %) y `rpc_error`/`build_error` hasta que **una** ruta conocida-buena pase simulación (control positivo reproducible) | Restricción 3 | Sin un solo `passed`, cualquier conclusión de mercado se apoya en filas que la tubería nunca validó |
| A2 | Re-basar `min_profit_usd`/`capital_usd` a algo alcanzable por la clase medida (p.ej. 60 bps × capital) **o** aceptar que el piso actual es un filtro de nicho | Restricción 2 | Es config del operador (no código, no rutas protegidas); el desajuste es 8.34× |
| A3 | Decidir el gate de modo: seguir en paper (y entonces **redefinir** la puerta de fase, que hoy es inalcanzable por diseño) o autorizar la ruta live (protegida, G5) | Restricción 1 | Único camino a `executed ≥ 1`; decisión explícita del operador |
| B | **Solo si el operador conserva pisos y capital:** barrido de nichos, con los requisitos de capacidad de §5 | Nicho | Es la rama "selección de mercado" *después* de A1/A2, no antes |
| C | **NO como primer movimiento:** FASE 2 (PROTECTIONS / DETERMINISTIC / PER-LEG / CONNECTORS) | — | No mueve ninguna de las 3 restricciones medidas; `CONNECTORS` (REA-11) recién se vuelve prerrequisito si se elige el nicho funding/CEX-DEX |

**Nichos, con su requisito real (si B):** funding/carry → conectores CEX normalizados **REA-11** + inventario (REA-15); CEX-DEX → REA-15
(whack-a-mole) + REA-11; liquidaciones → REA-17 (health factor/close factor/bonus de `aave-v3-core`); ciclos 5–7 patas → **REA-03** (V3 math)
+ **REA-04** (enumeración/ciclo negativo); Polymarket → cadena y contratos nuevos (fuera de las fuentes permitidas hoy);
long-tail/memecoins → **no requiere motor nuevo**: ya produce el 100 % del bruto grande medido ($13.7 k en W1), pero exige **decisión de
allowlist + screening de token** (`arbx-token-safety-screen`) y un modelo de valuación del token (hoy `economics=error`, neto no computado).

---

## 5. (4) Capacidades cerradas vs pendientes (sec0 del plan / `REA_MASTER.md` §3 y §8)

### 5.1 Unidades del plan

| Unidad | Estado | Evidencia |
|---|---|---|
| REA-01 TWO-LEG-01 | **CERRADA** (merge `b5809f08`, deploy verificado) | `dex_b` no nulo **122/2315** vs **0/62** en W0 (Q6); log del searcher `dex_b=Some("uniswap-v3")` (t11 §4) |
| REA-02 OPTIMAL-SIZE-01 | **CERRADA** (merge `0a0b1388`) | forma cerrada + V-A/B/C byte-exacto; verificación independiente de t7 (upstream `f8b96f3` leído por HTTP) |
| REA-05 ATOMIC-CAUSE-01 | **CERRADA** (merge `9ecefc75`) | t10 |
| REA-06 POBLACION-FILTER-01 | **CERRADA con alcance nulo hoy** (merge `d2863d95`) | excluye 0 filas de producción all-time (t8, fail-honest declarado) |
| REA-07 REVERT-TRACE-01 | **CERRADA con hueco** (merge `d71c9fa5`) | P99 del hook por instrucción **no medido** (t9) |
| REA-03 V3-MATH | **PENDIENTE** (P0 del plan, no entró en la fase) | corrobora §3: los 6 pools V3 WETH/USDC no tienen reservas muestreadas; sin `TickMath/SqrtPriceMath/SwapMath` la pata V3 no se cotiza ni se ledgeriza |
| REA-04 NEG-CYCLE | **PENDIENTE** (P0 del plan) | sin enumeración Bellman-Ford de ciclos, la clase multi-pata no se explora sistemáticamente |
| REA-08 PROTECTIONS · REA-09 DETERMINISTIC · REA-10 PER-LEG-LEDGER · REA-11 CONNECTORS | **PENDIENTES** (P1) | `REA_MASTER.md` §8 |
| REA-12 BUNDLE · REA-13 BEST-SOLUTION · REA-14 SIGNED-ORDER · REA-15 NO-TRANSFER · REA-16 MEV-CLASS · REA-17 LIQUIDATION · REA-18 V3-SIM | **PENDIENTES** (P2) | `REA_MASTER.md` §8 |

**Fase 1 entregó 5 de las 7 unidades P0 del plan.** Las dos que faltan (REA-03, REA-04) son exactamente las que habilitarían las rutas donde
la DB muestra el bruto grande (V2↔V3, multi-pata).

### 5.2 Qué se sustrajo de cada fuente y qué quedó fuera

| Fuente (sha) | Se portó / se extrajo | Quedó fuera (declarado) |
|---|---|---|
| `jacksonConrad/better-simple-arbitrage@f8b96f3` (sin licencia → solo números) | forma cerrada `dx*`, orden fee-sobre-input, floor/ceil, guard de dirección; **vectores V-A (4), V-B (2), V-C (2)** | MongoDB/Token DAO (decimals), granularidad float 1e-4 de reservas, 1e-10 del tamaño, blacklist de addresses, provider Flashbots, `console.table`, su sistema de errores (FICHA REA-2 §1.3-1.4) |
| `flashbots/simple-arbitrage@03cf1058` (ISC declarada, sin LICENSE) | **ladder 9 + meet-halfway como VECTOR** (`208333333333333333` / `84409320961485006`) y el pre-filtro `1.003²` | el shotgun de tamaños sin rating y el CSV de pools (FICHA REA-1 §8) |
| `flashbots/simple-blind-arbitrage@73c6c270` (MIT) | patrón de error nombrado por causa, repago guardado por balance real, guard de caller | callback al **Vault de Balancer** como arquitectura del destino, payout nativo con `tx.origin`, `execute/` JS (FICHA REA-4 §7) |
| `aave/aave-v3-core@782f519` (BUSL-1.1, change-date vencido) | premium de flash loan, orden anti-reentrancia, patrón PULL de repago | verbatim (copyleft de red: `solo_vectores_numericos`) |
| `paradigmxyz/artemis@4bb1580` (MIT) | abstracción estrategia/ejecutor; ladder fijo de 14 tamaños como precedente | **DESCARTADO como fuente del segmento de traza**: el segmento no existe en el árbol (FICHA REA-trace §1.1); healthcheck HTTP, wallet/relay setup |
| `flashbots/mev-inspect-rs@b6e2f0a` (sin licencia) | **Tier 3: semántica de referencia, cero código** | todo el código |
| `Uniswap/v3-core` (TickMath/SqrtPriceMath/SwapMath) | **no se extrajo** | es REA-03, P0 pendiente |

Gates respetados en todo el rango: `git diff --name-only f27bdb33..6bde300e -- configs/app.toml
backend/relays-client/src/live_exec_policy.rs backend/relays-client/src/bundle_builder.rs` → **vacío**; `paper_mode = true` sigue en
`configs/app.toml` de `origin/main`. Sin mainnet, sin escritura en DB, sin cambios de config.

---

## 6. Huecos declarados (no computado ≠ cero)

1. **`net_expected_profit_usd` es NULL en 2273/2315 filas (98.2 %).** No son "no rentables": son **no computadas** (rechazo por allowlist antes del costeo). Contar 0 rentables sobre el total sería leer una etiqueta como otra cosa.
2. **Las 54 filas brutas-positivas ($13,657.61) no son beneficio verificado**: `economics.computation_status="error"`, neto no computado, y el token long-tail no tiene precio en `token_prices_usd` (21 entradas).
3. **Dislocación V2↔V3 y V3↔V3 no medida**: los 6 pools V3 WETH/USDC no tienen snapshots de reservas (cero ever); 564 pools activos nunca muestreados.
4. **El 23.5–27.0 bps del control del operador no fue reproducido** (sí su signo y su base de 31 bps). La pareja V2↔V2 que sí puedo medir es más estricta (59.91 bps de costo).
5. **`max_slippage_pct=0.0050`**: unidad (fracción vs %) no establecida desde la DB; reportado verbatim, no interpretado.
6. **P99 del hook de traza (REA-07)** sigue sin medir (heredado de t9); **REA-06 excluye 0 filas hoy** (heredado de t8).
7. **La puerta de fase (`executed ≥ 1`) es inalcanzable con `paper_mode=true`**; este documento **no** la re-interpreta ni propone marcarla en paper: la decisión es del operador y la ruta live es protegida (G5).
8. **La fila neta-positiva única** está etiquetada `NegativeNetProfit` por conflación de etiquetas (`config_aware.rs:1234-1235`): FOLLOWUP de nomenclatura, sin efecto en la puerta.

---

## 7. Apéndice — SQL literal y salidas crudas (Q1…Q7)

Todas las queries se ejecutaron con `psql -q -f -` (tuples_only, `|`) vía el transporte de §1, el **2026-10-23 entre 13:37 y 13:43 UTC**.
`now()` de la primera query: **2026-10-23 13:37:34.233069+00**.

### Q1 — ventana, status, tablas satélite

```sql
SELECT 'now', now();
SELECT 'opps_total', count(*) FROM opportunities;
SELECT 'status', status, count(*) FROM opportunities GROUP BY status ORDER BY 3 DESC;
SELECT 'rango', min(detected_at), max(detected_at) FROM opportunities;
SELECT 'post_summary',
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z') AS post_total,
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z' AND expected_profit_usd > 0) AS post_gross_pos,
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z' AND net_expected_profit_usd > 0) AS post_net_pos,
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z' AND net_expected_profit_usd IS NULL) AS post_net_null,
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z' AND status = 'executed') AS post_exec,
  count(*) FILTER (WHERE detected_at >= '2026-10-22T13:59:50Z' AND status = 'simulated') AS post_sim
FROM opportunities;
SELECT 'executed_all', count(*) FROM opportunities WHERE status='executed';
SELECT 'ptr', count(*) FROM paper_trade_runs;
SELECT 'exec_rows', count(*) FROM executions;
```
```text
now|2026-10-23 13:37:34.233069+00
opps_total|515989
status|rejected|515989
rango|2026-10-11 04:27:10.210808+00|2026-10-23 12:51:40.466318+00
post_summary|2315|62|1|2273|0|0
executed_all|0
ptr|0
exec_rows|0
```

### Q2 — rechazos, simulaciones y familias de fallo

```sql
SELECT 'REJ', coalesce(split_part(rejection_reason,':',1),'(null)'), count(*)
FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z' GROUP BY 2 ORDER BY 3 DESC LIMIT 15;
SELECT 'SIM_ALL', count(*), count(*) FILTER (WHERE passed) FROM simulations;
SELECT 'SIM_FAIL', coalesce(split_part(fail_reason,':',1),'(null)'), count(*) FROM simulations GROUP BY 2 ORDER BY 3 DESC LIMIT 12;
SELECT 'SIM_POST', count(*), count(*) FILTER (WHERE s.passed)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id WHERE o.detected_at >= '2026-10-22T13:59:50Z';
```
```text
REJ|TokenNotAllowed|2273
REJ|NegativeNetProfit|42
SIM_ALL|516059|0
SIM_FAIL|candidate_incomplete|393485
SIM_FAIL|reverted|64105
SIM_FAIL|rpc_error|31032
SIM_FAIL|sim_signer_funding_slot_unresolved|18068
SIM_FAIL|build_error|4758
SIM_FAIL|sim_timeout|3133
SIM_FAIL|funding_balanceof_timeout|1324
SIM_FAIL|route_path_not_representable|122
SIM_FAIL|funding_setstorage_rpc|20
SIM_FAIL|anvil_setStorageAt|5
SIM_FAIL|cyclic_route_missing_route_metadata|5
SIM_FAIL|fork_acquire_failed|2
SIM_POST|2266|0
SIM_POST_FAIL|rpc_error|957
SIM_POST_FAIL|build_error|541
SIM_POST_FAIL|sim_signer_funding_slot_unresolved|322
SIM_POST_FAIL|reverted|313
SIM_POST_FAIL|route_path_not_representable|122
SIM_POST_FAIL|sim_timeout|10
SIM_POST_FAIL|funding_balanceof_timeout|1
```

### Q3 — la fila neta-positiva (detalle completo)

```sql
SELECT 'ROW', id, detected_at, detector_id, status, strategy_kind, pair_symbol, dex_a, dex_b,
       amount_in_wei, expected_profit_usd, net_expected_profit_usd, roi_pct, risk_score,
       rejection_reason, coalesce(economics::text,'(null)'), coalesce(route_metadata::text,'(null)')
FROM opportunities WHERE id = 'eb057d32-2f1b-417d-a68c-e8db38e13f1e';
```
```text
ROW|eb057d32-2f1b-417d-a68c-e8db38e13f1e|2026-10-22 14:52:41.121067+00|patterns|rejected|dex_arb|c02aaa…/c00e94…|uniswap-v2|uniswap-v3|20083833301747329|0.94823370|0.345481|0.2582|0.0000|NegativeNetProfit|(null)|{"decimals": {"map": {}}, "dex_adapters": ["uniswap-v2", "uniswap-v3"], "fee_tier_bps": [30, 3000], "pool_addresses": ["0xcffdded873554f362ac02f8fb1f02e5ada10516f", "0xea4ba4ce14fdd287f380b55419b1c5b6c3f22ab6"], "token_addresses": ["0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0xc00e94cb662c3520282e6f5717214004a7f26888", "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"]}
```

### Q5 — pools WETH/USDC con snapshot en W1 y dislocación por minuto (V2↔V2)

```sql
SELECT 'SNAP', pr.pool_id, p.address, p.fee_tier, count(*), min(pr.timestamp), max(pr.timestamp)
FROM pool_reserves pr JOIN pools p ON p.id = pr.pool_id
WHERE pr.pool_id IN (SELECT unnest(ARRAY['2c19b298-…','3a6193f9-…','cd255a88-…','7fcbddad-…',
                                        '55642af7-…','48537c5d-…','5616cefc-…','41e921cf-…']::uuid[]))
  AND pr.timestamp >= '2026-10-22T13:59:50Z' GROUP BY 2,3,4 ORDER BY 5 DESC;

WITH px AS (  -- último snapshot por pool por minuto; precio USDC-por-WETH con el decimal de cada token
  SELECT p.id, date_trunc('minute', s.timestamp) AS m,
         (CASE WHEN p.token0_id = '843a6c39-5a49-4595-96a3-3406b0496195'::uuid
               THEN s.reserve1 * 1000000000000 / NULLIF(s.reserve0,0)
               ELSE s.reserve0 * 1000000000000 / NULLIF(s.reserve1,0) END) AS price
  FROM (SELECT DISTINCT ON (pool_id, date_trunc('minute', timestamp)) pool_id, timestamp, reserve0, reserve1
        FROM pool_reserves WHERE pool_id IN ('2c19b298-…','3a6193f9-…') AND timestamp >= '2026-10-22T13:59:50Z'
        ORDER BY pool_id, date_trunc('minute', timestamp), timestamp DESC) s
  JOIN pools p ON p.id = s.pool_id WHERE s.reserve0 > 0 AND s.reserve1 > 0
)
SELECT 'DISC', a.fee_tier || '->' || b.fee_tier, count(*),
  round(percentile_cont(0.5)  WITHIN GROUP (ORDER BY abs(a.price/b.price - 1) * 10000)::numeric, 2) AS med_bps,
  round(percentile_cont(0.25) WITHIN GROUP (ORDER BY abs(a.price/b.price - 1) * 10000)::numeric, 2) AS p25_bps,
  round(percentile_cont(0.75) WITHIN GROUP (ORDER BY abs(a.price/b.price - 1) * 10000)::numeric, 2) AS p75_bps,
  round(percentile_cont(0.95) WITHIN GROUP (ORDER BY abs(a.price/b.price - 1) * 10000)::numeric, 2) AS p95_bps,
  round(min(abs(a.price/b.price - 1) * 10000)::numeric, 2) AS min_bps,
  round(max(abs(a.price/b.price - 1) * 10000)::numeric, 2) AS max_bps
FROM px a JOIN px b ON a.m = b.m AND a.id < b.id GROUP BY 2;
```
```text
SNAP|2c19b298-7ca0-477b-9243-0b4ee5adcc86|0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc|30|5300|2026-10-22 13:59:53.567426+00|2026-10-23 13:42:34.532869+00
SNAP|3a6193f9-612d-4304-9336-9496089ec884|0x397ff1542f962076d0bfe58ea045ffa2d347aca0|30|5300|2026-10-22 13:59:53.575762+00|2026-10-23 13:42:34.541605+00
DISC|30->30|1424|2.85|2.01|7.09|19.20|0.02|59.97
```
*(Q5 midió 1,424 pares-minuto a las 13:41Z y Q7 midió 1,426 a las 13:43Z: la diferencia son los 2 minutos que se acumularon entre ambas corridas sobre datos vivos — no un cambio de método.)*

### Q6 — control PR1, serie horaria, cobertura del poller, allowlist y metadata de fees

```sql
SELECT 'DEXB_POST', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL) FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';
SELECT 'DEXB_PRE', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL)
FROM opportunities WHERE detected_at >= '2026-10-22T11:59:50Z' AND detected_at < '2026-10-22T13:59:50Z';
SELECT 'HOUR', to_char(date_trunc('hour', detected_at),'MM-DD HH24'), count(*),
       count(*) FILTER (WHERE expected_profit_usd > 0), count(*) FILTER (WHERE net_expected_profit_usd > 0),
       count(*) FILTER (WHERE net_expected_profit_usd IS NULL), count(*) FILTER (WHERE dex_b IS NOT NULL)
FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z' GROUP BY 2 ORDER BY 2;
SELECT 'COVERAGE',
  (SELECT count(DISTINCT pool_id) FROM pool_reserves WHERE timestamp >= now() - interval '1 hour') AS pools_1h,
  (SELECT count(*) FROM pools WHERE is_active) AS pools_active, (SELECT count(*) FROM pools) AS pools_all;
SELECT 'ALLOWLIST_KILL', count(*), round(sum(expected_profit_usd),2), round(max(expected_profit_usd),2),
       count(DISTINCT split_part(rejection_reason,':',2)) FROM opportunities
WHERE detected_at >= '2026-10-22T13:59:50Z' AND expected_profit_usd > 0 AND rejection_reason LIKE 'TokenNotAllowed%';
SELECT 'FEE_META', pair_symbol, dex_a, dex_b, expected_profit_usd, net_expected_profit_usd,
       route_metadata->'fee_tier_bps', amount_in_wei
FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z' AND expected_profit_usd > 0
ORDER BY expected_profit_usd DESC LIMIT 10;
```
```text
DEXB_POST|2315|122
DEXB_PRE|62|0
HOUR|10-22 14|16|3|1|15|5      HOUR|10-23 00|129|1|0|128|2
HOUR|10-22 15|51|4|0|45|15     HOUR|10-23 01|151|0|0|151|1
HOUR|10-22 16|24|0|0|23|6      HOUR|10-23 02|175|1|0|172|4
HOUR|10-22 17|78|0|0|78|1      HOUR|10-23 03|172|3|0|170|3
HOUR|10-22 18|111|6|0|107|10   HOUR|10-23 04|243|1|0|243|1
HOUR|10-22 19|110|29|0|102|37  HOUR|10-23 05|165|1|0|165|1
HOUR|10-22 20|98|3|0|95|6      HOUR|10-23 06|201|0|0|201|2
HOUR|10-22 21|53|1|0|53|2      HOUR|10-23 07|105|1|0|105|1
HOUR|10-22 23|25|0|0|25|0      HOUR|10-23 08|106|1|0|105|5
                               HOUR|10-23 09|93|3|0|89|7
                               HOUR|10-23 10|88|1|0|84|6
                               HOUR|10-23 11|52|3|0|51|4
                               HOUR|10-23 12|69|0|0|66|3
COVERAGE|1436|2000|5881
ALLOWLIST_KILL|54|13657.61|1272.10|12
FEE_META|b26172…/c02aaa…|uniswap-v2|uniswap-v3|1272.10144129||[30, 100]|28289151293009610000000
FEE_META|b26172…/c02aaa…|uniswap-v2|uniswap-v3|1261.99846540||[30, 100]|28051341482143913000000
FEE_META|8ed97a…/c02aaa…|uniswap-v2|uniswap-v3|610.32745179||[30, 10000]|91918729989317370000000
```
*(columnas de `HOUR`: filas | bruto>0 | neto>0 | neto NULL | `dex_b` no nulo. Las 10 primeras filas de `FEE_META` muestran `[30, 100]` en 9 de 10; el neto va vacío en todas.)*

### Q7 — distribución de neto, config viva, umbrales de dislocación y cobertura de pools

```sql
SELECT 'NET_DIST', count(*) FILTER (WHERE net_expected_profit_usd > 0), count(*) FILTER (WHERE net_expected_profit_usd = 0),
       count(*) FILTER (WHERE net_expected_profit_usd < 0), count(*) FILTER (WHERE net_expected_profit_usd IS NULL), count(*)
FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';
SELECT 'CFG', chain_id, capital_usd, min_profit_usd, min_roi_pct, max_slippage_pct, lp_fee_default_pct, enabled,
       enabled_strategies, array_length(enabled_dex_ids,1), (SELECT count(*) FROM jsonb_object_keys(token_prices_usd))
FROM trading_config ORDER BY chain_id;
-- DISC_THRESH: mismo CTE px de Q5, contando bps > 31 / > 40 / > 59.91
SELECT 'NEVER_POLLED', count(*) FROM pools p WHERE p.is_active
  AND NOT EXISTS (SELECT 1 FROM pool_reserves r WHERE r.pool_id = p.id);
```
```text
NET_DIST|1|0|41|2273|2315
CFG|1|1000.00|50.0000|0.3000|0.0050|0.0030|t|{dex_arb_v2v2,dex_arb,dex_arb_v2v3,dex_arb_v3v3,dex_arb_v3v2,liquidation,flashloan_arb,triangular}||21
CFG|10|0.00|5.0000|0.1000|0.5000|0.0030|f|{}||0     (idem 56,137,8453,42161: enabled=f)
DISC_THRESH|1426|24|5|1|1.68|0.07|5.91
DISC_TOP|2026-10-22 16:56:00+00|59.97   DISC_TOP|2026-10-22 17:00:00+00|42.09
DISC_TOP|2026-10-22 16:57:00+00|53.68   DISC_TOP|2026-10-22 17:01:00+00|36.65
DISC_TOP|2026-10-22 16:58:00+00|43.05   DISC_TOP|2026-10-22 17:02:00+00|35.32
DISC_TOP|2026-10-22 16:59:00+00|43.05   DISC_TOP|2026-10-22 17:03:00+00|35.32
NEVER_POLLED|564
```

### Verificación de gates (no-SQL)

```bash
git -C repos/arbitragex-v2 diff --name-only f27bdb33de026c0f6fa5edcbc1b94438b20af663..origin/main \
  -- configs/app.toml backend/relays-client/src/live_exec_policy.rs backend/relays-client/src/bundle_builder.rs
# -> (salida vacía)
git -C repos/arbitragex-v2 show origin/main:configs/app.toml | grep paper_mode
# -> paper_mode = true                       # SAFETY DEFAULT. Only the operator flips this to false...
```
