# S4-UNSIMULATABLE-01 — el 88,2 % es un **corte**, no un embudo: 100 % hueco de capacidad, y la etiqueta **ya no se emite**

**Resultado en una línea:** el discriminante declarado antes de contar da **617.570 de 617.570 = 100 % ANTES DE EVALUAR** — **cero rastro en las siete columnas, una por una** — y el **control lo valida** (todas las familias que sí evaluaron dan **100 % de rastro**). Es **HUECO DE CAPACIDAD, no veredicto de mercado**, y el código de `main` lo confirma por byte: `persistence.rs:130` clasifica **exactamente estos strings** como *capability gap* y su doc dice *«Capability gaps must NOT reject the opportunity»*. **Y el hallazgo que cambia la tarea: `strategy_cyclic_route_not_simulatable_in_s4` NO TIENE PRODUCTOR EN `main`** — es una etiqueta **retirada**, que sobrevive sólo para clasificar filas históricas. La familia **dejó de emitirse el `2026-10-08 00:08:50`** y su corte de la última hora es **0**. **No es un techo que se esté golpeando: es una época cerrada.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `e59912d9-e8b1-4c54-892e-6275a545691f`
**Solo lectura** · **CERO cambios al motor** · **CERO escrituras** · **In scope:** `docs/data/`

---

## 0. ★ EL CRITERIO, DECLARADO ANTES DE CONTAR

Sellado en `t158-criterio.md` **antes** de la primera consulta, con su hash y su predicción:

| | |
|---|---|
| **archivo** | `docs/data/S4-UNSIMULATABLE-01.criterio.md` (publicado junto al artefacto) |
| **blob git** | **`65bf600f79d2ba45533fd53b9e64b644a9203f74`** |
| sha256 | `15B14E8BC5FEBB6751FF96C6F2557D5EB2CFA77B68CA389401876C21BA907A3D` |
| **mtime UTC** | **`2026-10-08T13:45:33.3795852Z`** — **anterior** al primer `SELECT now()` de la medición (`2026-10-08 13:46:07.73315+00`) |
| criterio | una fila fue **EVALUADA** si y sólo si tiene **al menos uno** de seis rastros NO NULOS: `raw_trace`, `gas_estimate_wei`, `gas_price_wei`, `slippage_pct`, `revert_risk_pct`, `simulated_profit_usd`. **Murió ANTES DE EVALUAR** si los seis son NULOS y `fail_reason` está puesto |
| por qué este | es **binario por fila**, sin umbral que ajustar ⇒ no se puede afinar para que el número quede lindo |
| **predicción dicha antes** | *«Dado el nombre literal —«not_simulatable_IN_S4»— predigo que la MAYORÍA murió ANTES de evaluar… Si sale al revés, se reporta al revés.»* |

**Resultado: la predicción se sostiene, y más fuerte de lo predicho — no es «la mayoría», es el 100 %.**

---

## 1. Ventana — y una corrección de contrato ajena que se evita

`SELECT min(simulated_at), max(simulated_at), count(*) FROM simulations` — **exit 0**:

| | |
|---|---|
| **ventana de la tabla** | min `2026-10-07 21:19:58.524132+00` | max `2026-10-08 13:46:03.312988+00` | **700.464** |
| **ventana de la FAMILIA** | min `2026-10-07 21:19:58.524132+00` | **max `2026-10-08 00:08:50.140074+00`** | **617.570** |

**La columna es `simulated_at`.** El defecto de contrato de t156 (`detected_at`) habría dado `exit=1`; acá se usa la real y el `exit code` va a la vista en cada comando.

---

## 2. ★★ LAS VARIANTES, EXACTAS Y SEPARADAS — y el **residuo es CERO**

`SELECT fail_reason, count(*) … WHERE fail_reason LIKE 'strategy_cyclic_route_not_simulatable_in_s4%' GROUP BY fail_reason ORDER BY 2 DESC` — **exit 0**:

| `fail_reason` **literal** | filas | **última hora** | % de la familia |
|---|---|---|---|
| `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` | **576.907** | **0** | 93,42 % |
| `strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb` | **23.743** | **0** | 3,85 % |
| `strategy_cyclic_route_not_simulatable_in_s4:triangular` | **16.920** | **0** | 2,74 % |
| | **Σ = 617.570** | | |

`SELECT count(DISTINCT fail_reason)…` ⇒ **`variantes_distintas = 3` | `total_prefijo = 617.570`**.
**⇒ RESIDUO = 617.570 − 617.570 = 0. No hay variantes ocultas.** No hubo que declarar un residuo porque **no existe**: la partición es exhaustiva y verificada por dos vías (el `GROUP BY` literal y el `count(DISTINCT)`).

### ★ Y LA FAMILIA **PARÓ**: `última hora = 0` en las tres

`SELECT count(*) … AND simulated_at > now() - interval '1 hour'` ⇒ **`0`**, exit 0.
**max de la familia = `2026-10-08 00:08:50` contra un `now()` de `13:46:07` ⇒ la familia no produce una sola fila desde hace 13 h 37 min**, mientras la tabla sigue viva (`max` de la tabla: `13:46:03`).

### ★ La hipótesis t130 — declarada HIPÓTESIS, con los literales para decidir por byte

La variante es el string **`strategy_cyclic_route_not_simulatable_in_s4:triangular`** (16.920 filas). t130 apunta a **`triangular_arb`**. **NO son el mismo string.** Por tanto **no se afirma que t130 mueva estas filas.** Quedan entregados los literales exactos para que el acople se decida **con el byte, no con el parecido**. Y hay un tercer literal que conviene mirar antes: `persistence.rs:271` nombra **`strategy_cyclic_route_not_simulatable_in_s4:mev_01_016_triangular_arbitrage`** — un **stem compuesto** que **no aparece en PG**, lo que confirma que la lista del clasificador es de *strings esperados*, no de *strings observados*.

---

## 3. ★★★ LA PREGUNTA QUE DECIDE: **HUECO DE CAPACIDAD, 100 %**

### 3.1 El control que hace válido el discriminante

`con_rastro` por familia (**la misma consulta para todas**) — **éste es el control sin el cual un 0 no distingue «no evaluó» de «el instrumento no ve nada»**:

| familia | n | **con rastro** | % |
|---|---|---|---|
| `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` | 576.907 | **0** | **0 %** |
| `candidate_incomplete:amount_in_wei_zero` | 53.610 | **0** | **0 %** |
| **`sim_signer_funding_slot_unresolved`** | 29.078 | **29.078** | **100 %** |
| `strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb` | 23.743 | **0** | **0 %** |
| `strategy_cyclic_route_not_simulatable_in_s4:triangular` | 16.920 | **0** | **0 %** |
| `build_error: amount invalid: zero amount_in` | 144 | **144** | **100 %** |
| `build_error: router not in catalog…` | 84 | **84** | **100 %** |
| `funding_balanceof_timeout` | 5 | **5** | **100 %** |
| `fork_acquire_failed` | 4 | **4** | **100 %** |
| `anvil_setStorageAt: … Connection refused` | 2 | **2** | **100 %** |

**★ El instrumento discrimina, y perfecto: TODAS las familias que sí corrieron el simulador dan 100 % de rastro; la familia del S4 da 0 %.** El 0 es un **cero real**, no un fallo de instrumento. *(Y es la primera vez en la campaña que un 0 sale limpio sin necesidad de una segunda condición: el control lo prueba en la misma tabla, con la misma consulta, contra nueve familias.)*

### 3.2 Los dos conteos

`SELECT 'total=' || count(*) || ' | EVALUADAS=' || count(…) FILTER (WHERE <alguno de los seis>) || ' | ANTES_DE_EVALUAR=' || count(…) FILTER (WHERE <los seis nulos>) FROM simulations WHERE <prefijo>` — **exit 0**:

```
total=617570 | EVALUADAS_con_rastro=0 | ANTES_DE_EVALUAR_sin_rastro=617570
```

Y **columna por columna**, para que no quede en un `OR`:

```
raw_trace=0 | gas_estimate_wei=0 | gas_price_wei=0 | slippage_pct=0
| revert_risk_pct=0 | simulated_profit_usd=0 | passed=0 | total=617570
```

**⇒ 617.570 de 617.570 = 100 % murieron ANTES de evaluar. Cero en las siete columnas, una por una.**
**⇒ La respuesta a la pregunta del contrato es: HUECO DE CAPACIDAD. No es un veredicto de mercado.**

### 3.3 ★ Y el código lo dice con el mismo byte — la confirmación independiente

`backend/sim-ctl/src/persistence.rs:125-135` (blob **`377ef1b7a4c603772cb5205104e2e60faf29da0e`**):

> `/// Classify a sim fail_reason as a *capability gap* (the sim engine cannot`
> `/// run this strategy/chain/fork) rather than a genuine opportunity-quality`
> `/// failure (revert, gas exceeded, etc.). **Capability gaps must NOT reject the`**
> `/// **opportunity** — see insert_simulation.`
> `fn is_sim_capability_gap(fail_reason: &str) -> bool {`
> `    fail_reason.starts_with("strategy_not_simulatable")`
> `        || fail_reason.starts_with("strategy_cyclic_route_not_simulatable")`

**La medición empírica y el código coinciden**: mi discriminante dice «no evaluó» y la función del motor se llama `is_sim_capability_gap` para **exactamente ese prefijo**, con el doc diciendo que un *capability gap* **no es** un veredicto sobre la oportunidad y **no debe rechazarla**. **Dos vías independientes, misma conclusión.**

---

## 4. ★★ EL TECHO DENTRO DEL TECHO: es un **CORTE**, no un embudo

`filas = 617.570 | opps_distintas = 593.827` — y el `EXISTS` por `opportunity_id` (**exit 0**):

```
opps_familia=593827 | llegaron_a_S4_alguna_vez=0
```

**⇒ Ninguna de las 593.827 oportunidades distintas de esta familia llegó a S4 jamás, en ningún intento.** El contrato pedía exactamente esta distinción y su propia formulación la resuelve: *«Si `not_simulatable_in_s4` es terminal para el 100 %, el 88,2 % no es un embudo: es un corte.»* **Es terminal para el 100 %. Es un corte.**

**★ Y el corte es más grande que la familia:** `candidate_incomplete:amount_in_wei_zero` (53.610) también tiene **0 % de rastro**. Sumando: **617.570 + 53.610 = 671.180 de 700.464 = 95,8 %** de todas las filas de `simulations` son filas **que nunca simularon nada**.

---

## 5. ★★★ EL PRODUCTOR: **NO EXISTE EN `main`** — es una etiqueta retirada

`git grep -n 'not_simulatable_in_s4' origin/main` (sobre **`main` `77b42b3d`**, nunca el checkout compartido):

**El único `format!` que emite la familia es `sim_engine.rs:73`** (blob **`a86e45a8fa373aead429c7df9cf6926ce2fac6ab`**):

```rust
69:  Err(BuildError::UnsupportedStrategy(kind)) => {
70:      return Self::not_implemented(
71:          id, trace_id,
73:          &format!("strategy_not_simulatable_in_s4:{}", kind.as_str()),
74:      );
75:  }
```

**Emite `strategy_not_simulatable_in_s4:<kind>` — SIN `cyclic_route`.** Y el comentario inmediatamente siguiente (`sim_engine.rs:76-90`) dice, textualmente:

> `// SIM4-CYCLIC-01 (2026-10-07): a closed route is NO LONGER refused by`
> `// SHAPE. A closed route is representable — the V2 router takes the`
> `// whole path array in one call — so this arm now fires only when the`
> `// traversal path is ABSENT, which is a DATA-AVAILABILITY gap, not a`
> `// topological one.`
> `//`
> `// It gets its OWN reason family on purpose: the old`
> `// strategy_cyclic_route_not_simulatable_in_s4 label asserted an`
> `// impossibility that is FALSE, and reusing it would keep publishing a`
> `// false verdict on 50k rows.`

**Y `persistence.rs:288-290` (test) lo cierra:**

> `/// Both families stay recognised: the OLD one for historical rows, the NEW`
> `/// one for everything the fix now produces.`

**Los productores ACTUALES, según `persistence.rs:294-304`, son las familias nuevas:**
`cyclic_route_missing_route_metadata:<kind>` · `route_path_not_representable:<kind>`.

### ⇒ EL HALLAZGO, EN UNA FRASE

**`strategy_cyclic_route_not_simulatable_in_s4` NO TIENE PRODUCTOR EN `main`. Es una etiqueta RETIRADA — `SIM4-CYCLIC-01 (2026-10-07)` — que sobrevive con dos únicos usos: (a) el clasificador `is_sim_capability_gap` la reconoce **para filas históricas**; (b) tests que la fijan como string esperado. **La familia dejó de emitirse a las `2026-10-08 00:08:50`, que es lo que mi ventana mide.**

**★ Y por eso el 88,2 % no es un techo: es un fósil con fecha de cierre.** Las 617.570 filas se produjeron en **una ventana de 2 h 49 min** (`21:19:58` → `00:08:50`) por un camino que **ya no existe**.

---

## 6. ★ DOS TRAMPAS QUE EL CONTRATO PEDIÓ EVITAR, Y UNA QUE YA ESTABA ESCRITA EN `main`

### 6.1 Las poblaciones, declaradas (no se confunden)

| cifra | **qué es** |
|---|---|
| **`7`** | conteo dentro de la **MUESTRA DE 25** del benchmark (`SAMPLE_REASON_DISTRIBUTION`) |
| **`29.010`** | **UNIVERSO PG** de esa misma razón, medido por mí en t156 (total; 609/h; 214/15 min) |
| **`617.570`** | **UNIVERSO PG**, familia del S4, ventana `2026-10-07 21:19:58` → `2026-10-08 00:08:50` |

**Ninguna corrige a la otra. Son poblaciones distintas, y mezclarlas reproduce el error de escala de esta sesión.**

### 6.2 La hipótesis t130, ya declarada HIPÓTESIS en §2

`triangular` ≠ `triangular_arb`. Los literales van entregados; el acople se decide con el byte.

### 6.3 ★ Y una que el contrato no anticipaba: **esto ya estaba escrito en `main`**, fuera de mi alcance de escritura

| `archivo:línea` en `main` | lo que dice |
|---|---|
| **`docs/backend/SIM-FUND-02.md:70`** | *«en la tabla entera `strategy_cyclic_route_not_simulatable_in_s4:*` sigue mostrando **617.570 filas (99,9 %)** mientras su corte posterior al deploy es **0**. **El agregado miente.** Toda cifra de este informe es de corte.»* |
| `docs/backend/SIM4-CYCLIC-01.md:35` | *«**Ya no se emite** `strategy_cyclic_route_not_simulatable_in_s4`. Reutilizarlo habría seguido publicando un veredicto falso sobre 50.496 filas.»* |
| `docs/backend/SIM4-CYCLIC-01.md:104` | *«`strategy_cyclic_route_not_simulatable_in_s4` **ya no se emite (desde t88)** y esta rama lo confirma.»* |

**⇒ Ni la cifra ni el diagnóstico son nuevos: `docs/backend/SIM-FUND-02.md` tiene el mismo `617.570` y la misma conclusión.** Lo que esta tarea **agrega**, y que no estaba: (a) el **discriminante con control** — 100 %/0 % con nueve familias de contraste, que convierte «no se emite» en **«murieron antes de evaluar, y esto lo prueba un control»**; (b) **`llegaron_a_S4_alguna_vez = 0` sobre 593.827 oportunidades distintas** — la prueba de que es un **corte** y no un embudo; (c) el **productor localizado con línea y blob**, y la lista de las familias que **sí** se emiten hoy. **Y el dato de escala —que fue el aporte de t156— sigue siendo de t156.**

---

## 7. NO COMPUTADO, Y LÍMITES DEL INSTRUMENTO

| # | NO COMPUTADO | razón |
|---|---|---|
| 1 | **Cuántas filas produce hoy cada familia nueva** (`cyclic_route_missing_route_metadata:*`, `route_path_not_representable:*`) | No están en PG **porque la muestra no las contiene** dentro de la ventana medida; medirlas exige un corte sobre el runtime **actual**, no sobre una tabla cuyo corte de la familia es `00:08:50`. **NO COMPUTADO, no cero.** |
| 2 | **Si el deploy de `SIM4-CYCLIC-01` sigue en pie** | La familia paró a las `00:08:50`, consistente con un deploy; **no verifiqué el SHA del runtime en ese instante** |
| 3 | **La marca temporal exacta del cambio de etiqueta en producción** | Tengo la línea de código y su fecha de commit (`SIM4-CYCLIC-01`, 2026-10-07) y el **max de la familia** (`2026-10-08 00:08:50`); **no** tengo el deploy que los une. **NO COMPUTADO** — no se afirma que uno cause al otro |
| 4 | **La cobertura del catálogo de stems** (`persistence.rs:271` nombra un stem que no aparece en PG) | Medí que ese literal **no existe en la tabla**; **no** medí por qué el clasificador lo lista |

**Límites declarados:**
- **`no_simulation_row` es NO MEDIDA**, no fallo — y en esta tarea **no se usó**: todo el §3 se mide sobre `simulations`, que sí tiene fila.
- **Un contador acumulativo vuelve a 0 al reiniciar** ⇒ cualquier delta leído sobre `arbx_sim_funding_total` después de un deploy **no es comparable** con el acumulado anterior. Aplica al seguimiento de t156.
- **`passed = true` sigue en 0 sobre 699.889** (medido en t156; ahora 700.464). **El benchmark NO pasa porque un techo se arregle** — y en este caso **el techo ni siquiera está activo**: retirar una etiqueta no aprueba nada.

---

## 8. PERMISOS E INTEGRIDAD

**Solo lectura.** Todos los comandos fueron `SELECT` (con `--set=ON_ERROR_STOP=1` **antes** de `-tAc`), `git grep`/`git show` sobre `main` y `git rev-parse` de blobs. **NO se disparó el benchmark.** CERO escrituras en PG/Redis, CERO cambios al motor, CERO cambios de umbrales. Único archivo publicado: `docs/data/S4-UNSIMULATABLE-01.md` (+ el criterio sellado).

```bash
git hash-object docs/data/S4-UNSIMULATABLE-01.md
git rev-parse HEAD:docs/data/S4-UNSIMULATABLE-01.md
```

---

*El 88,2 % no era un techo: era un fósil con fecha de cierre. Las 617.570 filas se produjeron en 2 h 49 min por un camino que `main` ya no recorre —`SIM4-CYCLIC-01`, 2026-10-07— y su corte de la última hora es **0**. Mi discriminante, declarado antes de contar, dice que **el 100 % murió antes de evaluar**, con **cero rastro en las siete columnas**; y el control lo valida, porque **las nueve familias que sí simularon dan 100 % de rastro**. `persistence.rs:130` nombra esa condición con el mismo byte: `is_sim_capability_gap`, y su doc dice que **un capability gap no debe rechazar la oportunidad**. Es **corte, no embudo**: ninguna de las 593.827 oportunidades distintas llegó a S4 jamás. Y el dato incómodo que cierra: **`docs/backend/SIM-FUND-02.md` ya tenía el mismo 617.570 y la misma conclusión.** Lo que faltaba —y es lo que aporta esta tarea— era probarlo con control, demostrar que es un corte, y decir con línea y blob que **la etiqueta no tiene productor en `main`**.*
