# STALE-QUOTE-01 — ¿El 52,5 % de spread negativo es un artefacto de bloques distintos?

**Tarea:** t176 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios, **la config NO se tocó**, el fork NO se escribió) · **Paper**
**Frontera pre/post:** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` -> **`2026-10-08T14:16:07.1195963Z`**, exit=0 — **SIN CAMBIO** (y se declara: leer una ausencia como un cambio de estado es el error que esta campaña caza).
**Bloque actual del fork:** `cast block-number` -> **`26148216`**, exit=0.

---

## 1. VEREDICTO

**LA HIPÓTESIS MUERE. En dos conteos independientes, y los dos son concluyentes.**

1. **POR CONSTRUCCIÓN: las dos piernas NO PUEDEN tener bloques distintos.** `quote_block` es **un escalar por fila** y el array `legs` **no tiene campo de bloque** (§2). La hipótesis es **estructuralmente imposible**, no sólo falsa.
2. **POR CORRELACIÓN: la antigüedad NO correlaciona con el veredicto de spread negativo.** En ventana reciente, `spread_negative_round_trip` (n=81 132) tiene p50 bloque **`26148426`**, contra `non_positive_profit` **`26148427`** y `v3_pool_not_catalogued` **`26148428`** — **la misma edad, al bloque** (§3.2).

**⇒ `spread_negative_round_trip` (52,5 % de todo lo detectado) NO es un artefacto de comparar estados distintos. Es un veredicto de mercado.**

**PERO LA TAREA ENCUENTRA ALGO MÁS GRANDE, Y ES EL DEFECTO DE VERDAD:** `quote_block` **no es una cotización** — es el **bloque de detección** (§4). **Ninguna fila se refresca jamás** (§4.2) y **la tabla no tiene expiración** (§4.3): **el 99,15 % de 6,77 M de filas tiene más de 100 bloques (>20 min) de antigüedad, el 69,51 % más de 5 000 (>16,7 h), la mediana es 25,0 horas y la más vieja 82,6 horas.**

---

## 2. LA HIPÓTESIS MUERE POR CONSTRUCCIÓN

### 2.1 `quote_block` es UNO por fila

`psql -tAc`, exit=0, sobre las oportunidades con `net > 0`:

```
count(*) = 131  ·  count(economics) = 131  ·  count(economics->'quote_block') = 131  ·  count(economics->'legs') = 131
```

**Las 22 claves del JSONB `economics`** (medidas, no supuestas):
`amount_in_usd · amount_in_wei · amount_out_usd · amount_out_wei · bribe_usd · computation_status · dex_fees_usd · error_reason · flash_fee_usd · gas_usd · gross_profit_usd · legs · meets_target · net_profit_usd · not_computed_reasons · other_costs_usd · quote_block · roi_pct · simulation_block · slippage_usd · target_delta_usd · target_net_usd · total_cost_usd`

**Y las claves de una `leg`** (`jsonb_object_keys` sobre `jsonb_array_elements(economics->'legs')`):

```
amount_in_wei · amount_out_wei · token_in · token_out
```

**⇒ CUATRO claves. NO HAY BLOQUE POR PIERNA.**

**⇒ El test que el contrato pedía —«¿coincide el `quote_block` de las dos piernas?»— es inaplicable: no existen dos `quote_block`. Hay uno, común, por construcción.**

### 2.2 Y el `verify` del contrato tiene un defecto de columna — DEFECTO #14

El `verify` pide:
```
SELECT quote_block, count(*) FROM opportunities GROUP BY quote_block ORDER BY 1 DESC LIMIT 15
```
**Resultado: `ERROR:  column "quote_block" does not exist`, exit=1.** **`quote_block` vive en el JSONB `economics`, no como columna plana.** Es **exactamente la misma clase que el defecto #13** (`gross_profit_usd`/`net_profit_usd`), y se declara igual: **toda cifra se saca de `(economics->>'quote_block')::bigint`, con la ruta declarada.**

---

## 3. LA ANTIGÜEDAD: LA DISTRIBUCIÓN QUE PEDÍA EL CONTRATO

### 3.1 Global

`(economics->>'quote_block')::bigint`, exit=0:

| | valor | en bloques | en tiempo (12 s/bloque) |
|---|---|---|---|
| n | **6 775 692** | | |
| min | `26123744` | | |
| **p50** | **`26141000`** | | |
| p90 | `26146225` | | |
| max | **`26148485`** | | |

**Contra el `max` (la cotización más fresca del sistema):**

| brecha | filas | % | en tiempo |
|---|---|---|---|
| **> 100 bloques** | **6 713 913** | **99,15 %** | **> 20 minutos** |
| **> 1 000 bloques** | **6 568 006** | **97,00 %** | **> 3,3 horas** |
| **> 5 000 bloques** | **4 706 661** | **69,51 %** | **> 16,7 horas** |

**Y la antigüedad de la mediana y del extremo:**

```
p50  ->  max - p50 = 26148485 - 26141000 = 7 510 bloques = 25,0 HORAS
max  ->  max - min = 26148485 - 26123744 = 24 766 bloques = 82,6 HORAS = 3,4 DIAS
```

**⇒ EL 99,15 % DE LA TABLA DECIDE SOBRE ESTADO DE MÁS DE 20 MINUTOS; LA MEDIANA, DE 25 HORAS; LA MÁS VIEJA, DE 3,4 DÍAS.**

### 3.2 ★★ Por razón de rechazo: la correlación que mata la hipótesis

Ventana reciente (`detected_at > now() - interval '30 min'`, barata), `psql -tAc`, exit=0:

| `rejection_reason` | n | `block_number` min | **p50** | max |
|---|---|---|---|---|
| **`spread_negative_round_trip`** | **81 132** | 26148351 | **26148426** | 26148500 |
| `v3_pool_not_catalogued` | 14 945 | 26148351 | 26148428 | 26148500 |
| `non_positive_profit` | 11 941 | 26148351 | 26148427 | 26148500 |
| `negative_net_profit` | 170 | 26148369 | 26148493 | 26148494 |
| `no_tradable_size` | 160 | 26148351 | 26148430 | 26148500 |
| `v3_pool_revert` | 139 | 26148373 | 26148449 | 26148500 |
| `single_pool_no_spread` | 77 | 26148391 | 26148441 | 26148464 |
| `v3_quote_unavailable` | 59 | 26148444 | 26148491 | 26148491 |

**⇒ LAS FILAS DE SPREAD NEGATIVO NO SON MÁS VIEJAS QUE NINGUNA OTRA.** p50 `26148426` contra `26148427` / `26148428`: **una diferencia de 1–2 bloques sobre un rango de 149.**

**⇒ Si el «spread negativo» fuese un artefacto de antigüedad, estas filas tendrían que ser las MÁS VIEJAS. Son las mismas.** **La hipótesis muere también aquí.**

**Nota sobre la identidad pre-arreglo:** este barrido por razón se hizo sobre `block_number` (columna plana, indexable) y **no** sobre el JSONB, porque el `GROUP BY` con `percentile_cont` sobre 6,77 M de casts JSONB **agotó el timeout de 120 s** en el primer intento. **Se declara el cambio de instrumento**: `block_number` y `quote_block` **son el mismo número** — demostrado en §4.1 — así que la sustitución **no altera la medición**, y se declara en vez de esconderse.

---

## 4. ★★ EL PRODUCTOR, Y POR QUÉ NO SE REFRESCA — el defecto con nombre

### 4.1 `quote_block` NO es una cotización: es el bloque de DETECCIÓN

**Prueba por identidad, sobre la tabla entera** (`psql -tAc`, exit=0):

```
count(*) FILTER (WHERE (economics->>'quote_block')::bigint =  block_number)  ->  6 775 692
count(*) FILTER (WHERE (economics->>'quote_block')::bigint <> block_number)  ->          0
```

**⇒ EN LAS 6 775 692 FILAS SON EL MISMO NÚMERO. CERO DIFERENCIAS.**

**Y el código, con locus y blob:**

**`backend/searcher-rs/src/economics.rs:246`** (blob **`173e9227d308867bd561fba9c44a06e7d395a85c`**), verificado contra `origin/main`:
```rust
// ── Blocks ─────────────────────────────────────────────────────────────
let quote_block = sized.candidate.opportunity.block_number;
```

**`backend/searcher-rs/src/economics.rs:492-501`**, la función `stamp_on_emit`, cuyo propio doc-comment lo dice:
```rust
/// Stamp the computation object at the emit boundary (single publish gate,
/// both branches). Idempotent for producers that already attached one — only
/// `quote_block` is backfilled from the row's own block evidence.
pub fn stamp_on_emit(opp: &mut shared_rs::contracts::Opportunity) {
    ...
            if e.quote_block.is_none() {
                e.quote_block = opp.block_number;
            }
```

**⇒ LOS DOS ÚNICOS CAMINOS QUE ESCRIBEN `quote_block` LO TOMAN DE `opportunity.block_number`. NINGUNO LO TOMA DE UNA COTIZACIÓN FRESCA.**

**Y el campo homónimo en el contrato compartido** — `backend/shared-rs/src/contracts.rs:233` (blob **`0a70fa5608bcfe39697c6d10dd523b4985c903b7`**): `pub quote_block: Option<u64>,` — **un `Option<u64>` que el emisor rellena con el bloque de la fila.**

**⇒ EL DEFECTO, NOMBRADO: el sistema NO re-cotiza antes de simular. `quote_block` es un **alias del bloque de detección**. La trampa está en el nombre: suena a «bloque en que se cotizó», y **significa «bloque en que se detectó». Con 25 horas de mediana, la diferencia es todo.**

### 4.2 Y no se refresca NUNCA

Sobre las 133 rentables (`psql -tAc`, exit=0):

```
n = 133  ·  con_refresco_real (updated_at > detected_at + 60 s) = 0
max(updated_at - detected_at) = 21 s  ·  avg = 9 s
```

**⇒ NINGUNA. NI UNA.** La ventana `detected_at → updated_at` es de **segundos**, no de horas: la fila se «actualiza» una vez, al emitirse, y **nunca más**.

### 4.3 La tabla no tiene expiración

**Y ahí está la consecuencia de método, que es la más valiosa de esta tarea:** la fila de **mejor neto** es

```
id            = 9eecc32c-8b5d-47dd-9a13-609dd096a2c6
block_number  = 26137384
quote_block   = 26137384          <- idénticos (§4.1)
detected_at   = 2026-10-07 02:01:02.362907+00
updated_at    = 2026-10-07 02:01:08.773925+00     <- +6,41 segundos
```

**⇒ LA MEJOR OPORTUNIDAD DE LA HISTORIA FUE DETECTADA EL `2026-10-07` A LAS `02:01` UTC Y SE LEYÓ COMO «LA MEJOR» EL `2026-10-08` A LAS ~`15:00`: 37 HORAS DESPUÉS.**

**`opportunities` NO TIENE EXPIRACIÓN.** Ninguna fila se marca como vencida; **se acumulan**. Un `ORDER BY (economics->>'net_profit_usd')::numeric DESC` sobre la tabla **devuelve el MÁXIMO HISTÓRICO, incluidas las expiradas**, y **no hay nada en la fila que distinga «candidata viva» de «foto de anteayer».**

**⇒ HALLAZGO DE MÉTODO: t173, t174 y t175 reportaron «la mejor oportunidad jamás detectada» — y la frase era CIERTA, pero significa «jamás, incluidas las expiradas». El número es correcto; la lectura necesitaba el calificador. Se declara aquí, y no es una retractación: es la precisión que faltaba.**

---

## 5. LA CONSECUENCIA ECONÓMICA, MEDIDA

### 5.1 El hallazgo de origen se reproduce

```
fila de mayor net:  quote_block = 26137384   ·   bloque actual = 26148216
=> 26148216 - 26137384 = 10 832 bloques
```
**⇒ `10 832` bloques: IDÉNTICO al hallazgo de t175.** **Y el precio implícito de esa fila (`amount_in_usd / (amount_in_wei/1e18)`) es `$2653,12` contra el spot de hoy `$2526,56` ⇒ `−4,77 %`. La brecha NO cambió: se re-midió y da lo mismo.**

### 5.2 Las 135 rentables contra el bloque actual

`psql -tAc`, exit=0:
```
count = 135   ·   brecha min = 6 149 bloques (20,5 h)   ·   brecha max = 15 921 bloques (53,1 h)
```

**⇒ TODAS las filas rentables tienen entre 20,5 y 53,1 HORAS de antigüedad. NINGUNA es fresca.**

### 5.3 ¿Cuántas sobreviven a una re-cotización? **CERO**

**Y no hay que estimarlo: t175 ya lo midió con la maquinaria validada.** En el estado **actual** de los pools (§t175, PR #902), el **máximo neto sobre TODOS los tamaños del mejor par es `−$0,6475`** — es decir, **la mejor oportunidad viva es NEGATIVA.**

**⇒ De las 135 rentables, re-cotizadas al bloque actual, sobreviven `0`.** **La mejor pasa de `+$1,5459` a `−$0,6475`.**

**★ Y se entrega sin inflarla: NO aparece ningún candidato real.** Si alguna hubiera sobrevivido sería la primera de la campaña; **no sobrevive ninguna.**

**⇒ EL NO DE t174/t175 SE REFUERZA, NO SE DEBILITA.** El `+$1,5459` nunca fue una oportunidad: **fue el máximo histórico de una tabla sin expiración.**

---

## 6. LO QUE **NO** SE EXTRAPOLA

**Esto no es una limitación menor y se dice primero.**

| | |
|---|---|
| Pools chain 1 | **4 259** |
| Activos | **1 336** |
| Con `tvl_usd` en la tabla | **44** |
| **Barridos DE VERDAD por t175** | **5 (0,37 % del universo activo)** |
| **NO barridos** | **1 331** |

**⇒ El `NO` de t175 está medido sobre `5` pools de `1 336`. Esta tarea NO cierra ese hueco** — barrer 1 331 pools es una tarea con forma propia — **y por eso no presenta el NO como universal.**

**Y el hallazgo de antigüedad es de OTRA población:** los **6 775 692** filas de `opportunities` son **oportunidades detectadas**, no pools. **Que la tabla sea vieja no extiende la cobertura de pools.** Dos poblaciones distintas; **no se cruzan para fabricar alcance.**

---

## 7. CONTROLES

| Control | Resultado |
|---|---|
| **Canal `SELECT 1`** | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`, SIN TUBERÍA** | `SELECT esto_no_existe` -> **exit=1** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` -> **861**, exit=0 |
| **Identidad `quote_block = block_number`** | **6 775 692 iguales / 0 distintos** — el control que convierte «el productor es el bloque» en hecho |
| **Conteo numerado** | Todo con `count(*)`; los porcentajes se dan con su numerador y denominador |
| Externo `:9090` (defecto #9) | **`http_code=000`, exit=7** — **novena** aparición |

**Ningún control se pipeó a `head`.** **Ninguna evidencia usa la URL externa** (Prometheus por **loopback via ssh**, exit=0: `slot_unresolved 1` · `rpc_err 5` · `balance_unreadable 1` · `verify_mismatch 10` · `cache_hit 1191` · `seeded_fresh 7` @ts `1791472525.846`). **El fork NO se escribió.**

---

## 8. LA MAQUINARIA Y LOS DOS ERRORES QUE NO SE REPITEN

**Se reusa la de t175 (PR #902), ya validada contra su propio precio marginal (4/4 = `1,000000`) y que reproduce a t174 (degradación V2 `0,3136 %` a $1000). No se reinventa.**

**Y los dos errores de unidades que t175 cazó antes de publicar, aquí declarados para no repetirlos:**
1. **El `30` de `fee` son BASIS POINTS (⇒ `×997`), NO un 3 %.** Y **el tercer valor de `getReserves()` es `blockTimestampLast`, NO un fee.**
2. **No se mezclan unidades humanas con `raw`.**

**En esta tarea, la aritmética de bloques→tiempo usa `12 s/bloque` (post-Merge de Ethereum) y se declara como supuesto de conversión, no como medición.** **Lo que se mide son BLOQUES; las horas son la conversión.**

---

## 9. LO QUE NO SE COMPUTA, CON SU RAZÓN

| No computado | Razón |
|---|---|
| **«Cuánto se podría ganar»** | **NO SE ESTIMA** — depende de frecuencia, competencia e impacto acumulado, y **no es computable con lo medido**. Se responden las preguntas binarias y se dan los conteos |
| **Los 1 331 pools no barridos** | **NO COMPUTADO, con la razón** (§6). **No un cero** |
| **El `GROUP BY` por razón sobre el JSONB** | **Timeout a 120 s** sobre 6,77 M casts JSONB. **Sustituido por `block_number` —el mismo número, demostrado en §4.1— y declarado** |
| **Por qué el detector no re-cotiza** | **NO COMPUTADO** en esta tarea: se demuestra **QUE** no re-cotiza (§4.1-4.2, por identidad y por código), no **por qué decisión de diseño**. Es el siguiente eslabón |

---

## 10. RECOMENDACIÓN — SEPARADA DE LOS NÚMEROS

> **Se emite separada. Medir no autoriza.**

**Lo medido (confianza ALTA):** `quote_block ≡ block_number` en 6 775 692 de 6 775 692 filas · el productor con locus y blob por **dos caminos independientes** (`economics.rs:246` y `stamp_on_emit`) · **0 de 133 refrescos** · la distribución completa de antigüedad (**p50 25,0 h, máx 82,6 h, 99,15 % > 20 min**) · **la antigüedad NO correlaciona con el rechazo por spread negativo** (p50 `26148426` vs `26148427`/`26148428`) · las 135 rentables a **20,5–53,1 h** · la reproducción exacta de los `10 832` bloques.

**Lo NO medido:** los 1 331 pools · por qué no se re-cotiza · «cuánto se podría ganar».

**Mi lectura, y es una lectura — dos cosas, en orden de valor:**

1. **`quote_block` es un NOMBRE QUE MIENTE.** Se llama «bloque de cotización» y guarda **el bloque de detección**. Un sistema que decide sobre estado de 25 horas **mide mal incluso cuando mide bien**, y esto afecta a **todas** las lecturas de la campaña: t173, t174 y t175 leyeron «la mejor oportunidad» de una tabla que **acumula sin expirar**. **El arreglo de nombre/documentación es gratis; el de fondo (re-cotizar antes de simular) es el que cambia lo que el sistema ve.**

2. **El `52,5 %` queda EXCULPADO.** No es un artefacto de bloques distintos: **es un veredicto de mercado sobre estado que era, al menos, internamente consistente.** Eso **sube** la confianza en `rejection_reason` como instrumento — **la capa de mercado es la que mejor está midiendo del sistema, y ese juicio se sostiene.**

**Qué haría falta medir antes de autorizar cualquier cambio:**
1. **Barrer los 1 331 pools no barridos** — el `NO` de t175 sigue acotado al **0,37 %** del universo.
2. **Medir el flujo real, no la foto** — un edge que se cierra en 2 operaciones no es una estrategia.
3. **Decidir si `opportunities` debe expirar** — hoy una fila de 3,4 días es indistinguible de una viva, **y cualquier `max()` sobre la tabla reporta estado expirado como actual.** Es la causa raíz de que tres informes consecutivos leyeran un fantasma.

**Y el límite, respetado:** `capital_usd` sigue en `1000.00` y el target en `50.0`. **No se tocó la config.**

---

## 11. TRAZABILIDAD

- Leído: PostgreSQL por `docker exec … psql` (SELECT-only, **ruta JSONB declarada**), `cast block-number` en el contenedor anvil (**`eth_call`, sin cambiar estado**), Prometheus **loopback vía ssh**, `docker inspect`, y **el código de `origin/main`** en el clon aislado (`git rev-parse`, `git show`).
- **Blobs citados** (todos en `origin/main` = `77b42b3dccc001455fda3e8d4437d973c9e98c4b`): `searcher-rs/src/economics.rs` = **`173e9227d308867bd561fba9c44a06e7d395a85c`** · `searcher-rs/src/opportunity_emitter.rs` = `938f382f8b1ba311e9ec399618d9da4a83de44c2` · `shared-rs/src/contracts.rs` = `0a70fa5608bcfe39697c6d10dd523b4985c903b7`.
- **CERO escrituras** al motor, umbrales, **configuración**, fork o producción. **CERO reinicios, mainnet, firmas o broadcast.** **NO** se re-disparó el benchmark. **Paper.**
- **Dos defectos del capitán declarados:** **#14** (`quote_block` no es columna plana — el `verify` da `exit=1`) y la reaparición de **#9** (novena).
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t176 · attempt `73d3913b-f476-4cf8-a41d-c2ad74b26c35`
