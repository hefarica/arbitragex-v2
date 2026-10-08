# VERIFY-E2E-02 — ¿La cadena llega al final? Verificación independiente (línea base PRE-DEPLOY + candidato t88)

**Orden:** t91 (kind: verification) · **Intento:** `88fe7f01-1345-4943-a9db-9c34db7d3ddc`
**Verificador:** Reviewer — **no soy autor de t88 ni de t89** · **`SHA_BASE`** = `21d2039cc80b8c47c3eea6b223ce7e663da604fd`
**Permisos ejercidos:** SOLO LECTURA sobre el VPS y sobre el repo ajeno. Sin merge, sin deploy, sin push, sin editar producción.
**Canal:** `& "C:\Program Files\Git\usr\bin\ssh.exe" -o BatchMode=yes -o ConnectTimeout=25 -i ~/.ssh/arbx_hetzner root@195.201.235.70 "echo <b64> | base64 -d | bash"` (el `ssh.exe` de Windows da 255; el de Git-for-Windows llega como root; base64 evita el CRLF).
**Único path escrito:** `docs/review/VERIFY-E2E-02.md` (untracked, sin commit y sin push).

---

## 0. VEREDICTO

**La cadena SE MUEVE y ESCRIBE — pero NO LLEGA AL FINAL.** Con el canal probado y la ventana declarada antes de mirar:

| Pregunta del objetivo | Respuesta medida |
|---|---|
| ¿El productor publica? | **SÍ** — `arbx:opps:validated`: `entries-added` **12.451.182 → 12.459.516** en **120 s** (**+8.334**, ≈**69,5/s**) |
| ¿sim-ctl consume y escribe? | **SÍ** — en la MISMA ventana `simulations` creció **+8.334 filas** (**1:1** con las entradas) |
| ¿Llega al final (veredicto computado)? | **NO** — `simulations` = **212.515 filas**, `passed` = **`false` en el 100 %**, y `simulated_profit_usd` **computado en 0 filas** (NULL en las 212.515) |
| ¿Y después? | `executions` = **0 filas / 0 inserciones**, `paper_trade_runs` = **0 filas / 0 inserciones** |
| Motivo | **único y localizado**: `strategy_cyclic_route_not_simulatable_in_s4:{dex_arb,flashloan_arb,triangular}` — **la familia cíclica cubre el 100 % de las filas** |

**El candidato de t88 NO está desplegado** (`PR #846` OPEN, `mergedAt=null`; `main` = `21d2039c`, que no lo contiene) ⇒ **todo lo medido aquí es LÍNEA BASE PRE-DEPLOY**, y la comparación "después" **NO ES MEDIBLE en esta tarea: derivada a t93**.

**Un hallazgo MEDIA sobre el candidato** (no sobre su intención): renombrar el `fail_reason` a `cyclic_route_missing_route_metadata:*` **saca a esa familia del clasificador de capability-gap** de `persistence.rs` — archivo que el propio PR declara no tocar. Impacto **HOY = 0** (medido: 0 oportunidades en estados vivos y 0 filas con la familia cíclica como `rejection_reason`), pero **latente y con el mecanismo probado**: la primera oportunidad viable que llegue viva al simulador sería marcada `rejected`. Detalle y `requiredFix` en §5.

---

## 1. C0 — CONTROL DE CANAL (corre primero)

```
$ docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"
1
psql_exit=0            (SSH_EXIT=0)
```

El canal **devuelve fila**. Por lo tanto **los ceros de este informe son ceros medidos, no ausencia de dato.** Si el canal hubiera fallado, el veredicto habría sido `NO MEDIDO`.

---

## 2. C1 — LÍNEA BASE PRE-DEPLOY, FECHADA (2026-10-07T22:06:19Z)

```
$ psql ... -tAc "SELECT passed, count(*), count(simulated_profit_usd) FROM simulations GROUP BY passed"
f|192470|0
```

**Lectura:** un **único grupo**, `passed=false`, con **192.470 filas**; `count(simulated_profit_usd)` = **0** ⇒ **la columna está NULL en las 192.470**. *(Un NULL se reporta como **NO COMPUTADO**, jamás como 0: aquí el 0 es el número de filas CON valor computado, y son 192.470 las que NO lo tienen.)*

```
$ psql ... -tAc "SELECT count(*) AS filas, count(simulated_profit_usd) AS computados, count(*) FILTER (WHERE simulated_profit_usd IS NULL) AS nulos FROM simulations"
192470|0|192470
```

```
$ psql ... -tAc "SELECT fail_reason, count(*) FROM simulations GROUP BY fail_reason ORDER BY 2 DESC"
strategy_cyclic_route_not_simulatable_in_s4:dex_arb|180828
strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb|6922
strategy_cyclic_route_not_simulatable_in_s4:triangular|4720
```

**180.828 + 6.922 + 4.720 = 192.470 = el 100 % de la tabla.** No hay una segunda familia de `fail_reason`: **todo lo que sim-ctl escribe hoy muere en el mismo punto** y con el mismo nombre.

**Fecha y columnas (para poder fechar la línea base):** `SELECT string_agg(column_name…) FROM information_schema.columns WHERE table_name='simulations'` →
`id,opportunity_id,simulator,gas_estimate_wei,gas_price_wei,slippage_pct,revert_risk_pct,simulated_profit_usd,passed,fail_reason,raw_trace,trace_id,simulated_at` ⇒ existe **`simulated_at`**, que permite fechar cada fila.

*Nota de comparación con el briefing: el capitán midió 50.496 filas; a las 22:06 había 192.470 y a las 22:11, **212.515**. La tabla crece a ~70 filas/s ⇒ **cualquier número de esta línea base queda fechado**, y compararlo con el del briefing sin fecha sería un error.*

---

## 3. C2 — VENTANA DECLARADA **120 s** (declarada ANTES de mirar; medida dentro de un solo script)

```
VENTANA_DECLARADA=120s
T0_UTC=2026-10-07T22:08:52Z   entries_added=12451182  last_generated_id=1791410932080-0  simulations=203350
T1_UTC=2026-10-07T22:10:52Z   entries_added=12459516  last_generated_id=1791411052108-0  simulations=211684
DELTA_entries_added=8334      DELTA_simulations=8334
```

**Lectura:** en 120 s entraron **+8.334** entradas al stream `arbx:opps:validated` (≈**69,5/s**) y `simulations` creció **exactamente lo mismo** ⇒ **el consumo es 1:1 con la publicación** (no hay backlog ni descarte). `last-generated-id` avanza (1791410932080-0 → 1791411052108-0). **El pipeline está VIVO de punta a punta hasta la escritura** — lo que no llega es el **veredicto**.

**Qué contienen esas filas nuevas** (mismo canal, ventana desde T0):

```
$ psql ... -tAc "SELECT passed, fail_reason, count(*), count(simulated_profit_usd) FROM simulations WHERE simulated_at >= TIMESTAMPTZ '2026-10-07T22:08:52Z' GROUP BY 1,2 ORDER BY 3 DESC"
f|strategy_cyclic_route_not_simulatable_in_s4:dex_arb|8776|0
f|strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb|346|0
f|strategy_cyclic_route_not_simulatable_in_s4:triangular|220|0
```

**9.342 filas nuevas: el 100 % `passed=false`, el 100 % con la familia cíclica, el 100 % con `simulated_profit_usd` NO COMPUTADO.** El pipeline no está "casi llegando": está escribiendo, en tiempo real, la misma negativa por nombre.

**Claves del plan canónico:** `redis-cli --scan --pattern 'arbx:validated_plan:*' | wc -l` → **0**.

---

## 4. C3 — `executions` y `paper_trade_runs`

```
$ psql ... -tAc "SELECT relname,m_tup_ins…"   →  (comando exacto abajo)
simulations|211285|211518
executions|0|0
paper_trade_runs|0|0
$ psql ... -tAc "SELECT (SELECT count(*) FROM executions) AS exec_rows, (SELECT count(*) FROM paper_trade_runs) AS paper_rows"
0|0
```

**Motivo exacto de que sigan en 0:** la cadena tiene **dos puertas** y ninguna abre — (1) `paper_trade_runs` sólo se escribe cuando el checklist falla por `PaperModeActive` o cuando hay un envío simulado de una oportunidad **simulada con éxito**; (2) `executions` exige un veredicto con `passed` decidido. Como **`passed=true` no existe en ninguna de las 212.515 filas**, ninguna oportunidad alcanza esas etapas. No es un canal mudo: es una cadena que **se corta antes**, con el motivo escrito en `fail_reason`.

**Contexto de las oportunidades** (mismo canal):

```
$ psql ... -tAc "SELECT status, count(*) FROM opportunities GROUP BY status ORDER BY 2 DESC"
rejected|7613152
$ psql ... -tAc "SELECT status, count(*) FROM opportunities WHERE status IN ('validated','scored','detected') GROUP BY 1"
(sin filas)
$ psql ... -tAc "SELECT count(*) FROM opportunities WHERE rejection_reason LIKE 'strategy_cyclic_route%'"
0
```

**7.613.152 oportunidades, TODAS en `rejected`; 0 en estados vivos; y 0 con la familia cíclica como `rejection_reason`.** Ese último 0 es un resultado con significado: **la rama de capability-gap está funcionando hoy** (la familia cíclica nunca quedó como motivo de rechazo de la oportunidad) — dato que se vuelve decisivo en §5.

---

## 5. C4 — EL CANDIDATO DE t88, verificado de forma independiente

**Metadatos (leídos del remoto, no del informe del autor):** `gh pr view 846` → `state=OPEN`, **`mergedAt=null`**, `head=c5adf311e5d8…`, `base=main`, **3 archivos, +289/−35**: `backend/sim-ctl/src/sim_engine.rs` (+17/−10), `backend/sim-ctl/src/tx_builder.rs` (+189/−25), `docs/backend/SIM4-CYCLIC-01.md` (+83).

### (i) La ruta cíclica se hace REPRESENTABLE — **CONFIRMADO por el diff y por el test**

Símbolos medidos en el diff (`+`/`−` ocurrencias): `build_probe_with_path` **+8/−0**, `CyclicRouteMissingPath` **+8/−0**, `CyclicRouteNotRepresentable` **+2/−4**, `encode_v2_path` **+3/−0**, `encode_v3_exact_input` **+4/−1**, `cyclic_route_missing_route_metadata` **+3/−0**, `strategy_cyclic_route_not_simulatable_in_s4` **+2/−1**.

El test nuevo **afirma la representabilidad, no la declara**: `cyclic_two_leg_route_builds_executable_probe` construye `build_probe_with_path(&o, signer, &[a, b, a])` y comprueba `from == signer`, `to != 0`, `data.len() > 4`, **selector `0x38ed1739`** (`swapExactTokensForTokens`) y que el array del path codificado lleva **A, B, A en orden** (helper `word()` de 32 bytes). Otros tres tests: `cyclic_two_leg_route_builds_on_v3`, `cyclic_route_without_path_has_its_own_reason`, `cyclic_route_with_inconsistent_path_is_refused`. **⇒ (i) sostenido.**

### (ii) El fail-closed — **INTACTO, y con una corrección al contrato**

El contrato apunta a `persistence.rs:131-135` como "el fail-closed". **Medido: esas líneas NO son el fail-closed, son el clasificador de capability-gap:**

```
persistence.rs (main):
L130: fn is_sim_capability_gap(fail_reason: &str) -> bool {
L131:     fail_reason.starts_with("strategy_not_simulatable")
L135:         || fail_reason.starts_with("strategy_cyclic_route_not_simulatable")
```

El fail-closed real que decide `passed` está en **`sim_engine.rs`**:

```
L24:  pub max_slippage_for_pass_pct: f64,
L153: let passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct);
L164: fail_reason: if passed { … }
```

**Es un fail-closed de verdad:** `is_some_and` exige que **haya** medición (`slippage_pct = Some`) y que esté **dentro del umbral**; sin medición ⇒ `false`. **El PR lo deja intacto**: en `sim_engine.rs` el diff tiene **UN solo hunk** (`@@ -58,19 +58,26 @@`, el brazo del error + comentarios) y **no toca L153 ni L164**. **⇒ (ii) sostenido.**

### (iii) ¿Se relajó algún umbral para forzar `passed=true`? — **NO. Comprobación explícita**

**Comparación literal main vs head del PR** (mismo archivo, dos refs):

```
main: pub max_slippage_for_pass_pct: f64,  ||  let passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct);
head: pub max_slippage_for_pass_pct: f64,  ||  let passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct);
IDENTICO: True
```

Y en el diff completo, `max_slippage_for_pass_pct` aparece **sólo dentro del texto del documento** (2 líneas de doc), **nunca como línea de código cambiada**. **⇒ (iii) sostenido: ningún umbral de rentabilidad fue relajado**, y `passed=true` sigue exigiendo medición + umbral.

### HALLAZGO F-01 (MEDIA) — el renombrado saca a la familia del clasificador de capability-gap

**Mecanismo, medido, no inferido.** El PR renombra el `fail_reason` emitido:

```
-  "strategy_cyclic_route_not_simulatable_in_s4:{}"      (viejo)
+  "cyclic_route_missing_route_metadata:{}"              (nuevo)
```

`persistence.rs` (**archivo NO tocado por el PR**) clasifica así:

```
L73-77:  let sim_capability_gap = r.fail_reason.as_deref().map(is_sim_capability_gap).unwrap_or(false);
L79-86:  if !r.passed && sim_capability_gap { /* commit, NO flippea a 'rejected'; return */ }
L88-109: next_status = if r.passed {"simulated"} else {"rejected"};
         UPDATE opportunities SET status=$2, rejection_reason=COALESCE($3, rejection_reason)
          WHERE id=$1 AND status IN ('validated','scored','detected')
```

**Prueba por patrones** (los 10 prefijos/contiene del clasificador, L131-L178, **sin catch-all**: termina en `|| fail_reason == "output_undecodable"`):

```
cadena strategy_cyclic_route_not_simulatable_in_s4:dex_arb => match=strategy_cyclic_route_not_simulatable => capability_gap=SI
cadena cyclic_route_missing_route_metadata:dex_arb        => match=                                             => capability_gap=NO
```

⇒ **Con el candidato desplegado, la nueva familia deja de ser "capability gap"** y por lo tanto **deja de estar protegida**: la oportunidad se flipea a `rejected` con `rejection_reason='cyclic_route_missing_route_metadata:<kind>'`. Es exactamente el modo de fallo que el propio código prohíbe en su comentario de SIMWIRE-02: *"Absence of capability must NEVER become an opportunity-quality rejection — otherwise a flipped SIM_BACKEND=revm structurally drains the validated stream into permanent `rejected` rows."*

**Impacto HOY: 0 — y lo digo con la medición (§4):** el `UPDATE` sólo aplica a `status IN ('validated','scored','detected')`, y hay **0 oportunidades en esos estados**; además **0 filas** llevan la familia cíclica como `rejection_reason`, lo que confirma que la protección **hoy funciona** con el nombre viejo. Por eso la severidad es **MEDIA y no ALTA**: el defecto es real y su mecanismo está probado, pero su radio de acción actual es nulo y **se activa justo cuando el arreglo empiece a producir oportunidades viables** (es decir, en el paso siguiente de esta misma cadena, t93).

**Agravante de forma:** el diff **no menciona `is_sim_capability_gap` ni una sola vez** (0 ocurrencias) y `persistence.rs` no aparece en ningún hunk de código ⇒ **no hay test que ancle el contrato entre el emisor y el clasificador**. El PR sí declara el otro hueco (que falta el call site en `consumer.rs`), pero **no declara éste**.

**requiredFix** (una de las dos, en el MISMO change-set que renombra):
1. añadir la familia nueva a `is_sim_capability_gap` (`persistence.rs`): `|| fail_reason.starts_with("cyclic_route_missing_route_metadata")`; **o**
2. mantener un prefijo ya reconocido por el clasificador al emitir la razón nueva.
Y en cualquiera de los dos casos: **un test del clasificador** con la cadena nueva (hoy: 0 menciones), para que un renombrado futuro no vuelva a romper el contrato en silencio.

---

## 6. C5 — EJE TEMPORAL, declarado sin ambigüedad

- **El candidato NO está desplegado.** `gh pr view 846` → `state=OPEN`, **`mergedAt=null`**, head `c5adf311…`; `main` = **`21d2039c…`**, que **no lo contiene**. No hubo merge, ni deploy, ni push (prohibidos y no ejecutados).
- Por lo tanto **todas las mediciones de contadores de este informe son LÍNEA BASE PRE-DEPLOY** (§2-§4), **no** el resultado del arreglo.
- La comparación **"después"** (¿aparece una fila con `simulated_profit_usd` computado y `passed` decidido?) **NO ES MEDIBLE en esta tarea** y se declara **NO MEDIDA**, derivada a **t93**.
- **Prohibido y no hecho:** presentar esta línea base como si fuera el efecto de t88.

**Lo que sí queda fijado para que t93 sea comparable:** el mismo comando, la misma ventana declarada y estos anclajes fechados — `simulations` **212.515** filas / `passed=false` **100 %** / computados **0** (2026-10-07T22:11Z); `fail_reason` familia cíclica **100 %**; `executions` **0**; `paper_trade_runs` **0**; `arbx:validated_plan:*` **0**; `entries-added` **12.459.516** @ 22:10:52Z.

---

## 7. C6 — Cada número con su comando (mismos comandos del contrato, con los placeholders resueltos)

| Comando (contenedores reales) | Salida literal |
|---|---|
| `docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"` | `1` |
| `… -tAc "SELECT passed, count(*), count(simulated_profit_usd) FROM simulations GROUP BY passed"` | `f\|192470\|0` (22:06) → `f\|212515\|0` (22:11) |
| `… -tAc "SELECT fail_reason, count(*) FROM simulations GROUP BY fail_reason ORDER BY 2 DESC"` | `strategy_cyclic_route_not_simulatable_in_s4:dex_arb\|180828` · `:flashloan_arb\|6922` · `:triangular\|4720` |
| `… -tAc "SELECT relname, n_tup_ins, n_live_tup FROM pg_stat_user_tables WHERE relname IN ('simulations','executions','paper_trade_runs')"` | `simulations\|211285\|211518` · `executions\|0\|0` · `paper_trade_runs\|0\|0` |
| `docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:validated` | `entries-added 12440302→12459516` · `last-generated-id 1791410774836-0→1791411052108-0` · `length 10000` · `groups 1` |
| `docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern 'arbx:validated_plan:*' \| wc -l` | `0` |

*(El `0` de `validated_plan` es un cero MEDIDO: el canal del control C0 devolvió fila, y el mismo `redis-cli` devolvió el `XINFO` completo.)*

---

## 8. C7 — Solo lectura sobre el trabajo ajeno

- **No modifiqué** ningún archivo de producción, ni el artefacto verificado, ni el tablero: el único path escrito es **este documento** (`docs/review/VERIFY-E2E-02.md`, **untracked**) y **no hubo push**.
- **No mergeé, no desplegué, no re-disparé CI**, no toqué runs, no reverti nada. El VPS se leyó con `psql`, `redis-cli` y `XINFO` **en modo consulta**.
- **Declaración de no-autoría:** no soy autor de t88 (PR #846) ni de t89 (PR #844). Los verifiqué por lectura del diff y del test, no por su informe.

---

## 9. Handoff

| Consumidor | Qué recibe |
|---|---|
| **t93** | La línea base fechada y comparable (§2-§4, §6) y **la pregunta a responder**: ¿aparece una fila con `simulated_profit_usd` computado y `passed` decidido? Anclajes: 212.515 filas / 0 computados / familia cíclica 100 % (22:11Z). |
| **Autor de t88** | **(i), (ii) y (iii) sostenidos** — el cambio hace la ruta representable, el fail-closed de `sim_engine.rs:153` queda intacto y **ningún umbral fue relajado**. Y **F-01 (MEDIA)**: el renombrado rompe el contrato con `is_sim_capability_gap`; añadir la familia al clasificador (o conservar un prefijo reconocido) **en el mismo change-set**, con test del clasificador. |
| **Capitán / gobernanza** | El cuello **no es el productor ni el consumidor**: es que **todo lo que llega muere en la misma negativa por nombre**. El candidato elimina la parte **falsa** de esa negativa (queda `cyclic_route_missing_route_metadata`), y el propio autor declara el hueco que falta: **el call site en `consumer.rs` que alimente el path** — fuera de su in-scope. Sin eso, el `passed` seguirá sin poder computarse. |
| **Quien lea el briefing** | Los números del briefing (50.496 filas) son **anteriores**: a las 22:11Z la tabla tenía **212.515**. Con ~70 filas/s, **un número sin hora no es comparable**. |

---

## 10. ANEXO (post-firma) — el candidato SE MOVIÓ y un riesgo nombrado por el capitán

> Este anexo es **suplemento append-only**: no cambia el veredicto de §0 ni los criterios C0-C7. Corrige la **identidad del artefacto verificado** y responde a un **riesgo nombrado por el capitán** (no un defecto que él haya medido) con diff y tests, no con su enunciado.

### 10.1 IDENTIDAD: el head que verifiqué y el head vigente NO son el mismo

| | head | archivos | delta | estado |
|---|---|---|---|---|
| **Mi C4 (§5)** | **`c5adf311e5d8ddaad29de8b944bdb9107e922270`** | 3 | +289/−35 | verificado por mí |
| **Vigente hoy** | **`0fbad9758bd85b8632008a8d86f6100cddd175a0`** | **6** | **+414/−38** | `gh pr view 846` → `state=OPEN mergedAt=null` |

**Lo declaro sin ambigüedad: mi C4 verificó el head viejo; el veredicto de §5 NO cubría el head vigente.** Los 3 archivos nuevos son `anvil_backend.rs` (+12), `consumer.rs` (+33/−1), `simulator_backend.rs` (+17), y `sim_engine.rs` pasó de +17/−10 a +34/−12 (total 6 archivos con el doc). **Re-verifiqué (i), (ii) y (iii) sobre `0fbad975`** con el canal local (`git fetch origin refs/pull/846/head` → `FETCH_HEAD = 0fbad975…`):

- **(i) representable — SIGUE VIGENTE:** `build_probe_with_path` (L81), `encode_v2_path` (L273) y `encode_v3_exact_input` (L296) están en el head nuevo, y los tests cíclicos siguen afirmando la forma ejecutable.
- **(ii) fail-closed — SIGUE INTACTO:** la línea decisiva es **idéntica** en `main` y en `0fbad975`: `let passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct);` → `IDENTICO: True`.
- **(iii) umbrales — SIGUEN SIN RELAJAR:** `max_slippage_for_pass_pct` idéntico en ambos refs; el renombre no lo toca.

### 10.2 EL RIESGO NOMBRADO: `build_probe` quedó `#[cfg(test)]`-ONLY — **CONFIRMADO como hecho; y (a)(b) NO se materializan**

**El hecho es cierto y lo medí en el código, no en el aviso:** `tx_builder.rs` del head nuevo, **L63-70**:

```
/// Single-hop probe with NO traversal path. SIM4-CYCLIC-02: `sim_engine` now
/// always goes through `build_probe_with_path`, so this has no production
/// caller left — its only readers are the tests below. `#[cfg(test)]` declares
/// that fact, instead of `#[allow(dead_code)]` …
#[cfg(test)]
pub fn build_probe(opp: &Opportunity, signer_from: Address) -> Result<ProbeTx, BuildError> {
    build_probe_with_path(opp, signer_from, &[])
```

⇒ **la construcción de una sola pata perdió su llamador de producción** y el único punto de entrada productivo es `build_probe_with_path(opp, signer, path)`. **Hasta aquí, el riesgo del capitán es un hecho.**

**(a) EQUIVALENCIA del caso NO cíclico con path vacío/ausente — CONFIRMADA, y por IDENTIDAD, no por parecido.** Los tres encoders de una pata son **byte-idénticos** entre `main` (el camino productivo viejo) y el head nuevo, medidos extrayendo el cuerpo de cada función y comparándolos (`Compare-Object` → **0 diferencias** en los tres):

| Encoder | main vs `0fbad975` |
|---|---|
| `encode_v2` | **0 diferencias** |
| `encode_v3_exact_input_single` | **0 diferencias** |
| `encode_router02_exact_input_single` | **0 diferencias** |

Y la rama de path vacío/ausente llama **a esas mismas funciones con los mismos argumentos** que llamaba el `build_probe` viejo (`L140` `encode_v2(token_in, token_out, amount_in, signer_from, deadline)`; `L145` `encode_v3_exact_input_single(…)`; la rama `PancakeV3` nunca usó el path). Los campos del `ProbeTx` son los mismos: `from = signer_from`, `to = Address::from(router_entry.address)`, `value = U256::zero()`, `gas_cap = DEFAULT_GAS_CAP` (**5.000.000**, constante idéntica en ambos refs), `deadline = now_secs() + 120`. **⇒ Para una oportunidad NO cíclica con ruta ausente o vacía, el `ProbeTx` es EQUIVALENTE al viejo por construcción: misma calldata (misma función, mismos argumentos), mismo destino, mismo `from`, mismo `value`, mismo `gas_cap`.**

**(b) La rama de path vacío NO cae en `cyclic_route_missing_route_metadata` — CONFIRMADA.** El error está **dentro** del test de ciclicidad (`tx_builder.rs` L112-121):

```
let path_tokens: Option<Vec<Address>> = if token_in == token_out {
    if path.len() < 3 || path[0] != token_in || path[path.len() - 1] != token_out {
        return Err(BuildError::CyclicRouteMissingPath(opp.strategy_kind.clone()));
    }
    Some(path.to_vec())
} else {
    None                      // ← NO cíclica: nunca error, va a la construcción de una pata
};
```

y `sim_engine.rs` sólo convierte `Err(BuildError::CyclicRouteMissingPath(kind))` en `cyclic_route_missing_route_metadata:<kind>` (L91-95). ⇒ **ese motivo exige `token_in == token_out`: es para la ruta CERRADA sin ruta, no para toda ruta ausente. Una oportunidad no cíclica con ruta vacía construye normalmente.**

**(c) ¿El test del caso de path vacío ASSERTA la forma? — PARCIALMENTE, con precisión.** El caso "no cíclica + path vacío" **sí está cubierto**, y por una vía que vale la pena nombrar: el `build_probe` `#[cfg(test)]` **delega con `&[]`**, o sea que **es exactamente el punto de entrada productivo** (`build_probe_with_path(…, &[])`), y 5 tests pre-existentes lo llaman con fixtures **no cíclicas**:

| Test | Qué asserta |
|---|---|
| `v2_dex_arb_builds` (L408) | **selector `0x38ed1739`**, `value == 0`, `from == signer` |
| `v3_dex_arb_builds` (L456) | **selector `0x414bf389`** |
| `cartridge_stems_and_relabels_build_probes` (L584) | **selector `0x38ed1739`** para 6 kinds |
| `pancake_v3_probe_uses_router02_style_encode` (L654) | **selector calculado desde la firma** + `data.len() > 4+32` |
| `unknown_open_route_kind_builds` (L605) | **sólo `assert!(…is_ok())`** — sin forma |

⇒ **4 de los 5 assertan forma** (y el de Pancake es el más fuerte: computa el selector desde la firma). **Límite declarado:** **ningún test asserta `to` (el destino/router) ni `gas_cap`**, y el V2 no asserta el contenido exacto del array de path. Por eso la equivalencia del caso de una pata **se sostiene sobre la identidad de los encoders que medí aquí, no sobre un test** — y eso es una **observación BAJA**, no un defecto: si alguien cambia `encode_v2` en el futuro, los tests de una pata no lo detectarán salvo por el selector.

**Veredicto sobre el riesgo nombrado:** **el peor resultado posible NO ocurrió.** El caso de path vacío **no** regresó: cae en la construcción de una sola pata con encoders byte-idénticos, y **no** es interceptado por el motivo cíclico. Riesgo **CONFIRMADO como hecho estructural** (el llamador productivo de una pata desapareció) y **REFUTADO como regresión funcional** (los tres puntos (a)(b)(c) verificados en el diff y los tests).

### 10.3 F-01 (capability-gap) en el head NUEVO: **SIGUE ABIERTO**

El head vigente toca 6 archivos y **`persistence.rs` no está entre ellos** ⇒ el clasificador `is_sim_capability_gap` **sigue reconociendo sólo el nombre viejo**, mientras `sim_engine.rs` del head nuevo **sigue emitiendo** `cyclic_route_missing_route_metadata:<kind>` (L95). Con N1 cableado la frecuencia baja (una ruta cerrada CON path ya construye), pero la familia **seguirá emitiéndose** cada vez que la lectura de `route_metadata` devuelva `Ok(None)` o falle — y en esos casos la oportunidad **no** queda protegida por la rama de capability-gap. **F-01 se mantiene con severidad MEDIA y su `requiredFix` intacto.**

### 10.4 Observación nueva (BAJA) — el path se IGNORA en silencio para rutas NO cíclicas

Con N1 cableado, `consumer.rs` pasa `route_metadata.token_addresses` al backend (L440-453) y el builder decide: **si `token_in != token_out` ⇒ `path_tokens = None`** y construye **una sola pata** `token_in → token_out` por el router de `dex_a`, **descartando los hops intermedios** que ahora sí tiene en la mano. Es el comportamiento heredado (el viejo `build_probe` también era single-hop), pero **después de N1 el call site sí trae el path y el builder lo ignora sin decirlo**. **Impacto: NO MEDIDO** — no encontré un caso vivo de ruta no cíclica multi-hop en el feed (las entradas muestreadas del stream son cerradas y el 100 % de `fail_reason` es la familia cíclica). `requiredFix`: declararlo explícitamente en el doc/código, o encodear el path también para rutas no cíclicas multi-hop (fuera del alcance S4 declarado).

### 10.5 Lo que este anexo NO cambia

`C0`-`C7` (§1-§8) siguen como se midieron: **el candidato no está desplegado** (`main` = `21d2039c`), la línea base PRE-DEPLOY es la de §2-§4, y la comparación "después" sigue **NO MEDIDA** y derivada a t93. Verificación de solo lectura: sin merge, sin deploy, sin push; el único path escrito sigue siendo este documento.

---

*Verificación de solo lectura. Canal probado PRIMERO (C0 → fila), ventana declarada antes de mirar (120 s), y cada cifra con su comando y su salida literal en la misma línea. El candidato verificado **no está desplegado**: nada de lo medido es su efecto. Único path escrito: `docs/review/VERIFY-E2E-02.md` (untracked).*
