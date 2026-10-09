# LEDGER-SPINE-SIM-01 — Un solo ledger para spine y sim: un término que falta es `UNREADABLE`, no `$0`

- **Rol:** Specification · tarea **t190** · `attempt_id ad41b700-1213-428d-b1ae-f109ff521468`
- **Ámbito:** `docs/ledger/` (único path escrito). **CERO cambios al motor, umbrales o configuración.**
- **`origin/main` medido:** `4902a47c16ef21b7683e5bf405ab49cd1c525b77`
- **Naturaleza:** contrato escrito. **No estima ganancia**; arregla la honestidad del número.

---

## 0. Controles de instrumento (todos con exit code)

| Control | Comando / medición | Resultado |
|---|---|---|
| **Canal** | `SELECT 1` con `--set=ON_ERROR_STOP=1 -tAc` | **`1` · exit=0** |
| **Negativo** | `SELECT esto_no_existe` **sin tubería** | **exit=1** + `ERROR: column "esto_no_existe" does not exist` |
| **Positivo del `LIKE`** | `count(*) FROM simulations WHERE fail_reason LIKE 'reverted:%'` | **5322** · exit=0 — **no es cero: el campo tiene productor** |
| **`simulations` tiene filas** | `count(*)`, razones distintas, rango temporal | **719 987 filas**, **22** razones distintas, `2026-10-07 21:19:58` → `2026-10-08 20:14:47` |
| **`:9090` externo** | `curl -o NUL -w '%{http_code}'` contra `195.201.235.70:9090` | **no ejecutado con éxito en esta corrida** — el acceso SSH al VPS expiró por timeout al final del turno; **se declara como no medido, no como `000`** |
| **`forge fmt --all --check`** | no existe (defecto ya cazado) | no ejecutado |

**Desviación registrada, no glosada:** el control positivo del `LIKE` dio **5322** al abrir el turno y **5498** al cerrarlo — **la tabla creció durante la propia medición** (las `simulations` corren en vivo; última fila `2026-10-08 20:14:47`). **Todo conteo de esta acta es una foto con su instante, no un valor estable.** El control **sigue siendo válido como positivo** (≠ 0); el valor de t184 (**4007**) **no se reutiliza como si fuera de hoy**.

---

## 1. ★ EL DEFECTO, medido sobre la MISMA fila (no citado)

**Fila testigo: `949a670e-4d19-4a08-93c7-c336e56d6a9e`** · `triangular` · `236eb8(6-hop cycle)` · rechazo `gas_floor_breach:own_capital`.

**Ledger completo del spine (`opportunities.economics`, JSONB verbatim):**

| Término | Valor | Presente |
|---|---|---|
| `amount_in_usd` | 999.9999999999998 | sí |
| `amount_out_usd` | 1000.6746384959141 | sí |
| `gross_profit_usd` | **0.6746384959143795** | sí |
| `gas_usd` | **0.61523385** | sí |
| `other_costs_usd` | **0.01** | sí |
| `bribe_usd` | **0.0** | sí — *«0.0 is its TRUE computed value today»* (`economics.rs:162`) |
| **`flash_fee_usd`** | **0.0** | **presente con valor 0.0** |
| **`dex_fees_usd`** | **`null`** | **ausente, con razón declarada** |
| **`slippage_usd`** | **`null`** | **ausente, con razón declarada** |
| `total_cost_usd` | **0.62523385** | = 0.61523385 + 0.01 |
| **`net_profit_usd`** | **0.049404645914378764** | **= +$0.0494** |
| `target_net_usd` | 50.0 | sí |
| `target_delta_usd` | **−49.95059535408562** | **= −$49,95** |
| `meets_target` | `false` | sí |

**`not_computed_reasons` (lo que el spine declara sobre lo que falta):**

```json
{ "dex_fees_usd": "included_in_amount_out_post_fee",
  "slippage_usd": "priced_by_amm_curve",
  "amount_out_wei": "cycle_output_not_exposed_by_kernel",
  "simulation_block": "revm_simulation_is_sim_ctl_scope" }
```

> **El `+$0,0494` y la brecha `−$49,95` del encargo SE REPRODUCEN EXACTAMENTE.** El spine cierra sobre **cuatro** componentes y **el término más grande que falta es el LP**.

### 1.1 La misma fila del lado de la SIM — y aquí está el hallazgo duro

> ⚠️ **LECTURA OBLIGATORIA DE ESTA TABLA — es exactamente el defecto que la tarea cierra.**
> `count(<campo>)` es **`count` de un CAMPO**, es decir **cuántas filas lo traen poblado**. **No es el valor del campo.**
> Un **`0`** en esta columna significa **«NINGUNA fila lo trae: el campo no tiene productor»** — **jamás** «el beneficio vale cero» ni «el gas vale cero».
> **`count(simulated_profit_usd)` = `0` ⇒ el neto de la sim es `UNREADABLE`, no `$0`.** Confundir ambas cosas es **el defecto original** (un `0` que significa «no lo cargué» leído como un valor).

| Medición (`count` del **campo**) | Filas que lo traen poblado | Lectura correcta |
|---|---|---|
| `count(*) FROM simulations` | **719 987** | total de filas |
| `count(simulated_profit_usd)` | **0** | **el campo no tiene productor ⇒ `UNREADABLE`, NO `$0`** |
| `count(gas_estimate_wei)` | **0** | **`UNREADABLE`, NO `$0`** |
| `count(slippage_pct)` | **0** | **`UNREADABLE`, NO `$0`** |
| `count(raw_trace)` | **0** de **720 110** | **`UNREADABLE`, NO ausencia de revert** |
| `count(revert_risk_pct)` | **37 869** | control positivo: **este campo SÍ tiene productor** |

**Tres campos sin productor sobre una tabla de 719 987 filas.** Con control positivo al lado (`fail_reason` = 5322, `revert_risk_pct` = 37 869) esos ceros **no son transportes rotos: son campos sin productor**, y por la regla de esta tarea **se leen `UNREADABLE`, no `$0`**.

**Y está declarado en el código, no inferido:**
- `backend/sim-ctl/src/canonical_plan_consumer.rs:195-205` → *«PRICES-FREE by design (R8): net-USD is computed downstream from prices; `simulated_profit_usd` stays None (None = not computed, never fabricated)»* → `simulated_profit_usd: None`
- `backend/sim-ctl/src/consumer.rs:1165-1175` → **idéntico**, `simulated_profit_usd: None`
- `backend/sim-ctl/src/revm_backend.rs:231` → `simulated_profit_usd: None, // USD conversion deferred to Sprint 5`
- `backend/sim-ctl/src/revm_backend.rs:425` → **el test lo pinna**: `assert!(r.simulated_profit_usd.is_none(), "no fabricated profit");`

**Y el término que el encargo atribuye a la sim no vive en `sim-ctl`:**

`git grep -n 'lp_fee\|slippage_usd\|flash_fee' origin/main -- backend/sim-ctl/src` → **0 coincidencias.**

⇒ **`LP $7,38 + slip $12,31 + flash $2,22` NO están persistidos en ninguna columna ni en `raw_trace`.** La sim **sí los mide** (el número del operador sale de su corrida), pero **no los escribe en la base**. **Su neto es `None` por diseño.**

---

## 2. ★★ LA ACEPTACIÓN ES **NO COMPUTADA** — y ésa es la respuesta, no un fracaso

**La meta del contrato es `net_spine == net_sim ± ε` sobre la MISMA fila.**

| Extremo | Estado | Evidencia |
|---|---|---|
| `net_spine` | **existe** | `opportunities.economics->>'net_profit_usd'` = `0.049404645914378764` |
| `net_sim` | **NO EXISTE como término persistido** — **`UNREADABLE`, no `$0`** | `count(simulated_profit_usd)` = **0** de 719 987 filas (campo sin productor) |

**⇒ `ε` no se puede declarar y el conteo de filas que cumplen no se puede dar: el lado derecho de la igualdad no está en la base.** Declarar un `ε` aquí sería inventar el término que falta — exactamente el defecto que la tarea existe para cerrar.

**LA FRASE CLAVE DEL ENCARGO, medida contra la realidad:** el operador dice *«La pestaña de simulación sí [carga TLS/relay/slippage]»*. **La sim los MIDE — y su número contradice al kernel —, pero NO los persiste.** Lo que sí está en el código es lo contrario: *net-USD is computed **downstream** from prices*. Ese downstream **no se localizó en el repo como productor de un campo**.

**Lo que haría falta, mecánicamente, para poder computar `net_spine == net_sim ± ε`:**
1. **Que la sim persista su neto** (`simulated_profit_usd` hoy `None` por R8 en los 4 sitios citados), **o**
2. **que el spine cargue los términos** LP-por-pool + slippage + flash **con su fuente**, y entonces la igualdad se computa **contra el ledger de la sim**, no contra una columna vacía.

**Sin (1) o (2) el contrato no es computable.** Se declara **`NO COMPUTADO`** con esta evidencia exacta, conforme a la regla de la campaña.

---

## 3. ★★ LA REGLA: UN TÉRMINO QUE NO ESTÁ ES `UNREADABLE`, NO `$0`

**La parte más importante, y la que ordena todo lo demás.** Hoy **el spine emite `flash_fee_usd: 0.0`** en la fila testigo — y ese `0.0` **no significa «vale cero»: significa «no se cargó»**. Es la misma clase de defecto que la campaña ya pagó tres veces (el cero con productor ausente · el cero del transporte roto · el `0,000e+00` de una lista vacía).

### 3.1 Enumeración de CADA término del ledger

| Término | Fuente en el spine | ¿Qué pasa si falta? (HOY) | ¿Qué DEBE pasar? |
|---|---|---|---|
| **gas** | `econ.gas_usd` → `economics.rs:160`; config `gas_estimate_units=250000`, `gas_price_strategy=dynamic_basefee_plus_tip` | **presente** (`0.61523385`); si falta → `not_computed["gas_usd"]` con razón | correcto hoy |
| **LP fee (por pool)** | **NO EXISTE como línea.** `economics.rs:193-198` la declara `dex_fees_usd → "included_in_amount_out_post_fee"` | **`null` con razón** | ⚠️ **la razón es una AFIRMACIÓN, no una medición**: la sim mide LP $7,38 sobre la misma familia ⇒ **la afirmación del kernel está REFUTADA por la medición de la sim** |
| **impacto de curva / slippage** | **NO EXISTE como línea.** `economics.rs:199-202` la declara `slippage_usd → "priced_by_amm_curve"` (`SLIPPAGE_PRICED_BY_CURVE`, `economics.rs:104`) | **`null` con razón** | ⚠️ **misma objeción**: la sim mide slip $12,31; **`priced_by_amm_curve` no cubre ese monto** |
| **flash** | `econ.flash_fee_usd` → `economics.rs:161`; config `flashloan_fee_pct=0.0009` | **`0.0`** | ❌ **debe ser `UNREADABLE` si no se cargó**; `0.0` solo si es **valor computado** |
| **relay / builder tip** | `econ.builder_tip_usd` → `economics.rs:162` | **`0.0`**, con comentario *«0.0 is its TRUE computed value today»* | ✅ **correcto**: es un cero **computado**, y está declarado |
| **other_costs** | `econ.other_cost_usd` → `economics.rs:163`; config `ops_overhead_usd_per_attempt=0.01` | **presente** (`0.01`) | correcto hoy |
| **bribe** | `econ.builder_tip_usd` (mismo valor) | **`0.0`** | requiere declarar si es computado o ausente |

**Distinción que este contrato fija — y que hoy no se hace:**
- **`0.0` COMPUTADO** (p. ej. `bribe_usd`, con su comentario) → **es un valor, se emite `0.0`**.
- **`0.0` POR AUSENCIA** (p. ej. un flash que no se cargó) → **se emite `UNREADABLE`**, jamás `0.0`.
- **`null` CON RAZÓN** (p. ej. `dex_fees_usd`) → **está bien emitido así**; el problema **no es el formato, es que la razón sea falsa**.

### 3.2 ★ NO SE USA UN PROXY DE 30 bps PARA EL LP

**El LP fee va POR POOL, con el fee tier real.** El repo **ya tiene** el dato: `pools.fee_tier` (catálogo consultable) y `pool_a.fee_bps` (usado en `dex_engine.rs:439-440`, **con `unwrap_or(30)` como fallback** — fallback que **es exactamente un proxy constante** y que este contrato prohíbe).

**Fees vigentes por pool-tipo, declarados:** V2 `0,30 %` · V3 `0,05 %` / `0,30 %` (los fee tiers de la ruta). **La suma se emite por pool, no como constante.**

*(Nota de honestidad: los `fee_tier` concretos de los pools de la fila testigo **no se leyeron en esta corrida** — el acceso al VPS expiró al final del turno. Se declaran como **pendientes de lectura**, no como ausentes.)*

---

## 4. ★ LOS TRES SITIOS — path + línea + blob

| # | Sitio | Path | Línea | Blob (`origin/main`) |
|---|---|---|---|---|
| **1** | **El spine calcula el neto** | `backend/searcher-rs/src/economics.rs` | **`157`** (los 4 componentes) · **`204-207`** (`total_cost_usd`) · **`154`** (`net`) · **`270`** (`net_profit_usd`) | **`173e9227d308867bd561fba9c44a06e7d395a85c`** |
| **1b** | La razón declarada de lo que falta | `backend/searcher-rs/src/economics.rs` | **`193-202`** (`dex_fees_usd`, `slippage_usd`) · constantes `103-104` | *(mismo blob)* |
| **2** | **La card lo consume** | `backend/api-server/src/routes/opportunities-live.ts` | **`711-726`** — `ECONOMICS_USD_FIELDS`, la lista blanca de los 14 campos USD que el boundary acepta | **`750c313deae43d237ce2d4d9de9a6a4b95654747`** |
| **3** | **La sim calcula el suyo** | `backend/sim-ctl/src/canonical_plan_consumer.rs:205` · `consumer.rs:1175` · `revm_backend.rs:231` — **los tres lo dejan `None`** | ver §1.1 | *(blobs por archivo)* |
| **3b** | El test que **pinna** que no se fabrique | `backend/sim-ctl/src/revm_backend.rs` | **`425`** | *(mismo archivo)* |

**Comentario del propio spine que define el estándar (`economics.rs:116-119`):** *«Closure: `total_cost_usd` sums exactly the four present components and `net_profit_usd` is the kernel's own net; the two agree to float associativity (≤ ~1e-12 relative, the ARBX-0007 ulp lesson) — far inside the display tolerance the frontend ledger applies ($0.005).»*

> **Con los tres sitios localizados, hay contrato y no una intención.** Y el sitio 2 prueba que **la card tiene un boundary de 14 campos** — el mismo boundary donde un `0.0` de omisión entra al UI sin ser distinguido de un cero computado.

---

## 5. Efecto sobre lo que está en pantalla

**No se puede declarar cuántas de las 50 filas vivas cambian de número**, porque **el número de destino (`net_sim`) no está persistido** (§2).
- Filas donde spine y sim coexisten (join por `opportunity_id`): **569 628**.
- **De esas, el lado sim aporta `simulated_profit_usd` sin productor en el 100 % de las filas** (campo `UNREADABLE`; `count` = **0** de 719 987 — **no es un neto de `$0`**).
- ⇒ **Con el ledger unificado, TODAS las filas con spine computado cambiarían de número, y la dirección sería a la BAJA** (se añadirían términos hoy ausentes: LP + slippage + flash). **El conteo exacto es `NO COMPUTADO` porque el término de destino no existe.**

**No se declara mejora de PnL.** Esta tarea **no estima ganancia**.

---

## 6. ★ LA DISTINCIÓN QUE NO SE PIERDE

**Unificar el ledger NO crea EV.** Hace que **la card diga lo que la sim dice**.

Si el `net(s*)` de los 2-leg (t189) sigue siendo negativo, **esto no cambia el resultado económico: cambia que el operador lo vea sin tener que abrir la pestaña de simulación.**

**Los dos números al lado:**

| Extremo | Número | Estado |
|---|---|---|
| Mejor neto canónico vivo (spine) | **+$0,0494** | medido en el repo |
| Lo que la sim mide sobre la misma familia | **−$24,02** | **del encargo; no re-derivado ni persistido** |
| Target | **$50,0** | intacto |
| Brecha | **−$49,95** | `FAIL`, `meets_target=false` |
| **`net(s*)` de los 2-leg** | **t189** — **no se estima aquí** | |

**Misma fila, dos mundos. Y el que se ve es el que no carga los costos.** Unificar el ledger **no mueve el −$24,02**: mueve **que la card lo muestre**.

---

## 7. NO SE TOCA EL TARGET NI EL APETITO

Medido (`trading_config`, chain_id=1):

| Campo | Valor | Estado |
|---|---|---|
| `capital_usd` | **1000.00** | **intacto** |
| `min_profit_usd` | **50.0000** | **intacto** |
| `simulation_target_profit_usd` | **50.0000** | **intacto** |

- **Bajar `$50` a `$0,05` certificaría el agujero y no cambiaría el EV**: `kelly_fraction_bps` = **0** y `posterior_prob` ~0,00015 (dex) / ~0,00365 (ciclo) ⇒ **no hay edge estadístico que dimensionar**.
- **`net_realized_usd`:** la columna **no existe** en `opportunities` (`information_schema` → **0** filas). ⇒ *«sigue `NULL` hasta que exista receipt»* se cumple **vacuamente**: no hay campo que poblar.

---

## 8. Cierre

- **`net_spine == net_sim ± ε` es `NO COMPUTADO`**: `net_sim` **no está persistido** (campo sin productor: `count` = **0** de 719 987 ⇒ **`UNREADABLE`, no `$0`**; `None` por diseño R8 en los 4 sitios del código). **No se declara un `ε` sobre un término inexistente.**
- **La regla queda fijada:** un término que falta es **`UNREADABLE`**, nunca `$0`. Con la distinción **cero computado ≠ cero por omisión**.
- **LP por pool, nunca proxy de 30 bps.** El `unwrap_or(30)` de `dex_engine.rs:440` es el proxy que se prohíbe.
- **Los tres sitios están localizados con path + línea + blob.**
- **`+$0,0494` y `−$49,95` reproducidos exactamente** sobre la fila testigo.
- **Unificar el ledger NO crea EV.** No se declara mejora de PnL.
- **Target y apetito intactos** (`1000.00` / `50.0` / `50.0`).

*La card no miente por malicia: miente porque suma cuatro términos y muestra el total como si fueran todos. El arreglo no es sumar más — es dejar de escribir `$0` donde no se cargó nada.*
