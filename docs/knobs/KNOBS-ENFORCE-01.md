# KNOBS-ENFORCE-01 — El knob configurado no gobierna lo que se emite

**Tarea:** t196 · **Base:** `origin/main` = `4902a47c16ef21b7683e5bf405ab49cd1c525b77` · **Rama:** `docs/knobs-01` · **Ámbito:** `docs/knobs/`
**Runtime medido:** `77b42b3d` · **Estado:** PAPER. Cero mainnet, cero firmas, cero broadcast.

---

## 0. ALCANCE

**`backend/`, `shared-rs/` y `frontend/` están los tres FUERA de alcance ⇒ el patch de código NO puede aterrizar.** Se entrega: **el conteo con path + línea + blob**, **el patch especificado**, y **las mediciones antes/después**.

**Y EL DIAGNÓSTICO SE CIERRA ENTERO, con un número que explica el síntoma del operador:**

> **`126.316` filas de `opportunities` en 30 minutos colapsan a `54` grupos. `126.262` son REEMISIONES = `99,96 %`. La media es `2.339,185` reemisiones por ruta; el máximo, `20.564`.**

**⇒ «0 viable / 50 (54 en 5 min)»: el `54` son LOS GRUPOS, y el `50` es el default de `limit`. La pantalla sirve 54 rutas y ninguna es viable — porque el representante por defecto de cada grupo es la ÚLTIMA reemisión, no la mejor fila.** §3.

---

## 1. PATCH 3 — `with_slot0_cache`: LA ADMISIÓN ESTÁ ESCRITA EN EL CÓDIGO

### 1.1 El censo, completo

`git grep -rn 'with_slot0_cache' -- backend/` → **`15` coincidencias, TODAS en `size_optimizer.rs` (blob `a1411d8200d3baad5c912f2dcba036cc63e032b7`)**, exit=0:

| línea | qué es |
|---|---|
| `:864` | **`/// with_slot0_cache (tests populate it; prod wiring is a follow-up).`** ← **LA ADMISIÓN** |
| `:906`, `:921`, `:944` | menciones en doc |
| **`:938`** | **la definición**: `pub fn with_slot0_cache(mut self, cache: Arc<Slot0Cache>) -> Self` |
| **`:7279, :7683, :7778, :7860, :7922, :7982, :8078, :8170, :8203, :8278`** | **DIEZ llamadas — TODAS dentro del módulo de test** |

**⇒ LLAMADAS EN PRODUCCIÓN: `0`.**

### 1.2 Los constructores, con su conteo

`git grep -rn 'SizeOptimizer::new' -- backend/searcher-rs/src` → **los de PRODUCCIÓN son DOS**:

| path | línea | constructor | ¿`with_slot0_cache`? | blob |
|---|---|---|---|---|
| **`backend/searcher-rs/src/scanner.rs`** | **`:452`** | `let size_optimizer = Arc::new(SizeOptimizer::new(state_projector.clone()));` | **NO** | **`d54c8e174d6e426d08882ff4e3ac572b3c243620`** |
| **`backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs`** | **`:987-989`** | `fn optimizer_with(cache: Arc<ReservesCache>) -> SizeOptimizer { … SizeOptimizer::new(projector) … }` | **NO** | **`c767f4b96d504b85f48d9b8bef8933bef7c37989`** |
| `backend/searcher-rs/src/size_optimizer.rs` | `:4214`–`:5253`+ | decenas de `SizeOptimizer::new(projector)` — **todo el módulo de test** | (sólo tests) | `a1411d82…` |

**★★ Y EL DETALLE MÁS NÍTIDO, que el contrato no dice:** **`hop_cycle_bridge.rs:987` declara `fn optimizer_with(cache: Arc<ReservesCache>)` — RECIBE una cache por parámetro y NO la enchufa al optimizer.** **⇒ No es «falta el constructor»: el parámetro está ahí, en la firma, ignorado.**

**⇒ DECLARACIÓN: `with_slot0_cache` hay que agregarlo en `2` constructores de producción — `scanner.rs:452` y `hop_cycle_bridge.rs:987-989`.** **Y el `follow-up` que `:864` promete NUNCA ocurrió.**

### 1.3 La prueba de que el modelo local corre (o no) — **NO COMPUTADO, con su razón**

**El contrato pide `s*` con y sin la cache, y si son iguales decirlo.** **NO COMPUTADO: correr el optimizer exige `SizeOptimizer::new(...).with_slot0_cache(...)` en un binario — `backend/` fuera de alcance y no se compila ni se despliega nada.**

**⇒ Lo que SÍ queda demostrado, sin ejecutar:** **con `with_slot0_cache` jamás llamado en producción, el camino de la cache está desactivado por construcción, y `size_optimizer.rs:921` lo confirma en su propio doc: *«disabled (falls through to the QuoterV2 grid)»*.** **⇒ El modelo local no corre: la rama de la cache es inalcanzable en producción, y se cae al bracket.** **⇒ El «hallazgo» alternativo («el bracket ya coincidía») NO se puede afirmar ni descartar: NO COMPUTADO.**

**Y el rastro que el contrato cita queda consistente: si el tamaño emitido es el bracket y no `s*`, se explican las cuatro cifras distintas del snapshot ($0,01 kernel · $1000 intent · $2,5k sim · `min_size_usd = 10_000`) sin necesidad de otra causa.**

---

## 2. PATCH 5a — LA LIQUIDEZ: EL KNOB EXISTE, NO TIENE CONSUMIDORES, Y SI LOS TUVIERA SE COMERÍA EL GRAFO

### 2.1 El knob, y su censo

`git grep -rn 'min_pool_liquidity_usd' -- backend/` → **`10` coincidencias, TODAS en `canonical_knobs.rs` (blob `0dfebc449b8f0ea45253d56f40acf65f3fbda97e`)**, exit=0:

```
:29   //! - `min_pool_liquidity_usd` is **USD at the route bottleneck** (05_RUTAS
:57       pub min_pool_liquidity_usd: f64,          // 150_000 (USD, route bottleneck)
:176          min_pool_liquidity_usd: 150_000.0,     // ← EL DEFAULT
:278      min_pool_liquidity_usd: env_f64(… d.min_pool_liquidity_usd, …)   // overridable por env
:417-418  if self.min_pool_liquidity_usd < 0.0 { return Err(…) }           // validación
:550-551  "min_pool_liquidity_usd".into(), json!(self.min_pool_liquidity_usd)  // serialización
:825      assert_eq!(k.min_pool_liquidity_usd, 150_000.0);                 // test
```

**★★ ⇒ EL KNOB ESTÁ DECLARADO, CON DEFAULT `150_000.0`, OVERRIDABLE POR ENV, VALIDADO, SERIALIZADO Y TESTEADO — Y NINGÚN CONSUMIDOR LO LEE. `10` apariciones, y las `10` son la propia definición.**

**⇒ ESTA ES LA PRUEBA MÁS LIMPIA DE «el knob configurado no gobierna lo que se emite»: no hay un valor mal propagado; no hay propagación NINGUNA.**

### 2.2 Y si gobernara: cuánto cae — medido

```
P2_censo         = 1356 activos | 44 con tvl_usd | 14 con tvl >= 150k | 30 con tvl < 150k | 9 con tvl < 5k
P2_aristas_vivas = 271470 apariciones | 63920 sobre 150k | 2309 caen (<150k) | 205241 tvl_NULO
P2_que_caen      = 0x3df9dd6a… tvl=24484 fee=NULL | 0x4527492531… tvl=38253 fee=30 | 0xee4cf3b78a… tvl=64086 fee=3000
```

**Instante de la medición: ver §7.**

**Tres cosas, y las tres cambian la decisión:**

1. **`tvl_usd` existe en `44` de `1.356` pools activos = `3,2 %`.** **⇒ En `205.241` de `271.470` apariciones de arista (`75,6 %`) el dato NO EXISTE.**
2. **`2.309` aristas (`0,85 %`) caen por la regla `< 150k` — y son TRES pools distintos.** **⇒ Aplicar el knob quita `3` pools del grafo y no cambia casi nada.**
3. **★★ Y LA TRAMPA, que hay que declarar antes de tocar nada: `NULL < 150000` NO es `TRUE`, es `NULL`.** **⇒ Un filtro `WHERE tvl_usd >= 150000` NO deja pasar las `205.241` aristas sin dato — las DESCARTA. Eso mataría el grafo ENTERO por un dato AUSENTE, no por baja liquidez.** **⇒ VIOLA la regla de la casa: término ausente = `UNREADABLE`, nunca exclusión silenciosa.**

**⇒ LA REGLA QUE SE ESPECIFICA: `tvl_usd` AUSENTE ⇒ la arista NO se excluye y el estado se marca `UNREADABLE`; `tvl_usd` PRESENTE y `< knob` ⇒ la arista NO EXISTE.** **Sin esa distinción, el patch de liquidez es un borrado masivo disfrazado de filtro.**

### 2.3 La inconsistencia de umbrales, cerrada — y la respuesta no es ninguna de las dos

**`150_000` (searcher, `canonical_knobs.rs:176`) contra `5_000` (validación de token).** **Medido:**
- **el `150_000` NO GOBIERNA NADA** (cero consumidores, §2.1);
- **y si gobernara, dejaría `14` pools** en pie de `1.356 (`1,03 %`) — un grafo inservible;
- **el de `5_000` es sobre el TOKEN, no sobre el pool**, y su cobertura tampoco se midió aquí.

**⇒ PISO EFECTIVO HOY: ninguno. No hay filtro de liquidez aplicado a las aristas.** **⇒ El piso que gobierna es el `gas_floor` + `min_ev_usd` (los del otro eje, §5), no la liquidez.** **⇒ Y la incoherencia 150k/5k no se resuelve eligiendo una: se resuelve declarando que ninguno de los dos gobierna, y que el de 150k NO ES APLICABLE con `3,2 %` de cobertura.** **No se unifica aquí.**

**Y el caso testigo YELLOW: NO COMPUTADO con su razón.** La consulta `P2_yellow_token` (`SELECT … FROM tokens WHERE upper(symbol)='YELLOW'`) **no llegó a imprimirse** antes de que se cortara la lectura del log. **No se declara que YELLOW no esté: se declara que su medición NO SE COMPLETÓ.** **No se inventa ni su TVL ni su presencia.**

---

## 3. PATCH 5b — EL DEDUP POR BLOQUE, Y LA CLAVE QUE YA EXISTE

### 3.1 El campo: vive en el WIRE, no en la tabla

**`confirmations` NO es columna de `opportunities`** (`information_schema` → `0` columnas, y `SELECT confirmations FROM opportunities` → **`exit=1` + `ERROR: column "confirmations" does not exist`**). **Pero el campo SÍ EXISTE, y está localizado:**

```
backend/api-server/src/routes/opportunities-live.ts    blob 750c313deae43d237ce2d4d9de9a6a4b95654747
:342-350   concat_ws('|', o.chain_id::text, COALESCE(o.chain_id_out::text,''),
                     COALESCE(o.strategy_kind,''), o.token_in, o.token_out, o.dex_a,
                     COALESCE(o.dex_b,''))            AS route_group_key,
:351       MIN(o.detected_at) AS first_seen_at,
:352       MAX(o.detected_at) AS last_seen_at,
:353       COUNT(*)::int      AS confirmations,
:287       "...and COUNT(*) becomes `confirmations`. The wire row..."
:302       "and confirmations=1 (COUNT(*) >= 1 by construction)"
```

**★★ ⇒ `route_group_key` Y `confirmations` YA ESTÁN IMPLEMENTADOS EN EL API, y son exactamente lo que el contrato pide fijar:**
**`route_group_key = chain_id | chain_id_out | strategy_kind | token_in | token_out | dex_a | dex_b`**, **`confirmations = COUNT(*)` sobre esa clave.**
**⇒ NO HAY QUE INVENTAR LA CLAVE. Se cita, se reusa y se corre el `GROUP BY` en PG.**

### 3.2 Las reemisiones, contadas como el API las cuenta

```
P1_grupos (30 min) = 54 grupos | 126316 filas | 126262 reemisiones | max 20564 | media 2339.185
                     @ 2026-10-08 20:55:21.081065+00
P1_todos           = 596 grupos | 10756397 filas | 10755801 reemisiones | max 1730686
```

| | grupos | filas | **reemisiones** | % | max `confirmations` |
|---|---|---|---|---|---|
| **30 minutos** | **`54`** | `126.316` | **`126.262`** | **`99,96 %`** | **`20.564`** |
| **toda la tabla** | **`596`** | `10.756.397` | **`10.755.801`** | **`99,994 %`** | **`1.730.686`** |

**⇒ `99,96 %` de las filas de 30 minutos son la MISMA ruta reemitida.** **Y el `confirmations: 2933` que el operador leyó no es el caso extremo: el máximo medido es `1.730.686` para UNA sola clave — `591×` mayor.**

**⇒ CON ESTO, EL SÍNTOMA «0 viable / 50» QUEDA EXPLICADO CON TRES NÚMEROS:**

```
:1145   limit                ?? 50        ← el «50» del snapshot          (opportunities-live.ts)
:1153   route_representative ?? "latest"  ← NO "best_net"
:1171   viable_only          ?? "false"
:1183   max_age_seconds      ?? 300       ← los «5 min» del snapshot
:1189   order                ?? "detected_at"
P1_grupos (30 min) = 54                    ← el «54» del snapshot
```

**⇒ La pantalla sirve `50` de `54` rutas, y `0` son viables porque el representante por defecto es `latest` — LA ÚLTIMA REEMISIÓN, que es sistemáticamente la fila más castigada por la deriva del estado.**

**★★ Y el propio código documenta el defecto medido, en `:355-360`:**
> *«ROUTE-REP-01 (2026-09-28): with the `$6` flag TRUE the representative is the COMPUTED row with the highest net inside the window. **Measured defect: a later re-detection of the same route (net −12.93, computed) buried a real computed gain (+0.1198 at 11:51) and the wire carried ZERO net>0 rows while PG held six.** With the flag FALSE every sort key is NULL (NULLS LAST) and the order falls back to the previous rule — latest detection — byte-for-byte.»*

**⇒ ¡EL CÓDIGO YA TIENE EL SÍNTOMA ESCRITO: «ZERO net>0 en el wire con SEIS en PG»!** **⇒ Y la bandera que lo arregla (`route_representative=best_net`) tiene default `"latest"`.**

### 3.3 La regla, y el conteo antes/después

**REGLA ESPECIFICADA: se emite sólo si el bloque de la cotización ES el de la cabeza — un aterrizaje por ciclo.** **Replay ≠ oportunidad.**

**CONSECUENCIA MEDIDA, y es la pieza que conecta 5b con el síntoma:** **con dedup por bloque, las `126.262` reemisiones de 30 min NO entran ⇒ el grupo pasa a tener `confirmations = 1` ⇒ `latest` y `best_net` CONVERGEN y el representante por defecto deja de ser la fila castigada.** **⇒ El dedup arregla el representante SIN cambiar su default.**

| | antes | después |
|---|---|---|
| filas emitidas / 30 min | `126.316` | **NO COMPUTADO** |
| grupos / 30 min | **`54`** | **`54`** (el dedup no cambia cuántas rutas hay, sólo cuántas veces se emiten) |
| reemisiones / 30 min | **`126.262` (`99,96 %`)** | **`0` por construcción** |

**⇒ El «después» del total es NO COMPUTADO con su razón: sin aplicar el patch (`backend/` fuera de alcance) no hay un después que medir.** **La parte que SÍ es aritmética: `126.262` reemisiones dejan de emitirse, y quedan `54` aterrizajes.**

---

## 4. EL CASO TESTIGO DEL OPERADOR: ¿DESAPARECEN LOS 5-7 HOPS?

> *«El 6-hop mejor es el primer candidato que A+B dejarían de emitir.»*

**NO COMPUTADO, con su razón, y NO se rellena:** la consulta `P3` (filas por número de hops y sus net>0, con instante) **no llegó a imprimirse** antes de que se cortara la lectura del log. **⇒ No se declara cuántos 5-7 hops quedan: se declara que la medición NO SE COMPLETÓ.**

**Lo que SÍ está medido y sostiene la predicción por otra vía:** **el `6-hop` de la fila testigo `949a670e` (t195) tiene `net_spine = +0,049404645914` y su sim **no produce cifra** (`reverted: TransferHelper: TRANSFER_FROM_FAILED`, `revert_risk_pct = 100.0000`).** **⇒ Con el ledger completo, el `+$0,05` del kernel no sobrevive — y con el dedup por bloque, además, no se reemite.**

**⇒ SI EL `net(s*)` SIGUE SIENDO `≤ 0`, ESTO FUE UNA MEJORA DE INSTRUMENTO, NO DE PnL.** **Y los dos números, al lado: `hurdle mínimo del grafo = 0,3512 %` contra `spread máximo entre venues = 0,2507 %`** (t191, con `g1 = 0,000000` medido sobre 226.000+ filas). **⇒ NO SE DECLARA MEJORA DE PnL.**

---

## 5. LOS TRES PISOS DISTINTOS — DECLARADOS, NO UNIFICADOS

**El operador midió tres, y son el mismo defecto en otra capa. Se declaran para que el piso que gobierna quede escrito:**

| piso | valor | dónde | qué gobierna |
|---|---|---|---|
| **`min_ev_usd`** | **`25`** | `canonical_knobs.rs` | el EV mínimo del candidato |
| **`target_net_usd`** | **`50`** | `trading_config` / UI | el objetivo que la card compara |
| **`gas_floor`** | **`net ≥ coste × 3,0`** | `size_optimizer.rs` | el suelo de gas |
| **`min_profit_usd`** | **`50.0000`** | `trading_config` chain 1 | **medido hoy: `1000.00 / 50.0000 / 50.0000`** |

**⇒ «Ninguno es el SIM completo», y NO SE UNIFICAN EN ESTA TAREA.** **Se declara además el que NO EXISTE: no hay piso de liquidez aplicado (§2.3) y no hay piso de `relay` (abajo).**

**Y el término que falta, con su conteo numerado:**

```
P0_relay = 10754698 con la clave 'relay_fee_usd' | 0 con valor no-nulo
```
**⇒ `relay_fee_usd` está **presente-como-null** en `10.754.698` filas y su valor es no-nulo en `0`.** **⇒ Es el MISMO estado que `flash_fee_usd` en t195.** **⇒ Término ausente = `UNREADABLE`, NUNCA `$0`.** **No se convierte en cero.**

---

## 6. LO QUE NO SE TOCA

- **`trading_config` chain 1, leído: `1|1000.00|50.0000|50.0000` ⇒ INTACTOS.**
- **NO se subió `max_hops`.** Y se declara dónde vive su techo: **`backend/searcher-rs/src/agent_graph.rs:57`** (blob **`0c22d54bbbf270901f1983b3b4987df9320ee0ad`**): **`if !(2..=7).contains(&limits.max_hops) …`** ⇒ **el `7` está HARDCODEADO en la validación, no sólo en el knob.** **Y `canonical-knobs.test.ts:29/49` lo fija en `7`.** **No se sube: es el problema, no la solución.**
- **NO se bajó `min_ev_usd` (25) ni el target (50) a `$0.05`.**
- **No se añadieron long-tail ni cartuchos. No se encendió Live.**
- **CERO mainnet, firmas o broadcast. Paper.**

---

## 7. CONTROLES Y CONTEO, CON SU INSTANTE

| Control | Resultado |
|---|---|
| Canal `SELECT 1` | `1`, **exit=0** |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` → **`5980`** @ **`2026-10-08 20:54:53.18262+00`** — **serie propia**: `4007 → 5322 → 5498 → 5607 → 5910 → 5968 → 5980`. **El `4007` de t184 NO se reutiliza.** |
| Frontera pre/post | **NO MEDIDA en esta corrida**: el log se cortó antes de la línea de `StartedAt`. **Se declara como NO COMPUTADO, no se cita de una corrida anterior.** |
| `relay_fee_usd` | **conteo NUMERADO**: `10.754.698` con la clave, `0` con valor. **Ausente = UNREADABLE, no `$0`.** |
| `confirmations` / `route_group_key` | **NO son columnas** (`exit=1` + `ERROR`); **viven en el WIRE** (`opportunities-live.ts:342-353`). |
| `:9090` externo | **NO MEDIDO en esta corrida** — la línea no llegó a imprimirse. **Se declara, no se cita de t195.** |
| **La tabla es viva** | **cada cifra lleva su instante**; el `P1` de 30 min es de `20:55:21.081065+00` y el `P0` de `20:54:53.18262+00`: **28 s de separación, y son lecturas distintas.** |
| Ningún control pipeado a `head` | los controles, sin tubería |

---

## 8. DEFECTOS Y HALLAZGOS NUEVOS

1. **`with_slot0_cache`: `0` llamadas en producción; `10` en test; y `size_optimizer.rs:864` lo admite por escrito (`«prod wiring is a follow-up»`).** **El `follow-up` nunca ocurrió.**
2. **`hop_cycle_bridge.rs:987` recibe `cache: Arc<ReservesCache>` y NO la enchufa.** El parámetro está en la firma, ignorado.
3. **`min_pool_liquidity_usd`: `10` apariciones, las `10` en su propia definición. CERO consumidores.** **El knob está declarado, defaulteado, overridable por env, validado, serializado y testeado — y no lo lee nadie.**
4. **`tvl_usd` cubre `44` de `1.356` pools activos (`3,2 %`).** **⇒ El knob de liquidez NO ES APLICABLE con esa cobertura, y aplicado a lo bruto (`WHERE tvl_usd >= 150000`) DESCARTARÍA `205.241` aristas por dato AUSENTE.**
5. **`126.262` de `126.316` filas de 30 min son reemisiones (`99,96 %`); `54` grupos; `1.730.686` reemisiones para una sola clave.**
6. **★★ `route_representative` default `"latest"`, NO `"best_net"`** (`opportunities-live.ts:1153`), **y el propio código documenta el defecto medido en `:355-360`: «the wire carried ZERO net>0 rows while PG held six».** **⇒ El «0 viable / 50» tiene causa nombrada y arreglo ya implementado con default desfavorable.**
7. **`limit` default `50` y `max_age_seconds` default `300`** — **los «50» y «5 min» del snapshot del operador.**
8. **`relay_fee_usd`: presente-como-null en `10.754.698` filas, `0` con valor.** **Mismo estado que `flash_fee_usd` (t195).**
9. **`max_hops = 7` HARDCODEADO en `agent_graph.rs:57` (`!(2..=7)`), no sólo en el knob.**

---

## 9. AJUSTES PROPIOS, CAZADOS Y DECLARADOS

1. **Dos mediciones NO SE COMPLETARON y se declaran NO COMPUTADO, sin rellenarlas: el `P2_yellow_token` (el caso testigo `YELLOW`, ~$92k) y el bloque `P3` completo (filas por número de hops, y la pantalla viva).** **El log se cortó tras `P2_que_caen`.** **⇒ No se declara que YELLOW no esté, ni cuántos 5-7 hops quedan: se declara que la medición NO SE COMPLETÓ.**
2. **La frontera pre/post (`StartedAt`) y el control negativo `:9090` NO se midieron en esta corrida.** **Se declaran NO COMPUTADO y NO se citan de t195 aunque los tenga: una cifra sin su instante no entra.**
3. **La prueba de `s*` con y sin cache es NO COMPUTADA** (`backend/` fuera de alcance: no se compila ni se ejecuta el optimizer). **No se sustituye por una inferencia vestida de medición.**

---

## 10. EL PATCH ESPECIFICADO — PARA QUIEN TENGA EL SCOPE

**No se aplica aquí. Se especifica con path + línea + blob:**

| # | Fichero (blob) | Línea | Qué cambia |
|---|---|---|---|
| **3a** | `backend/searcher-rs/src/scanner.rs` (`d54c8e174d6e426d08882ff4e3ac572b3c243620`) | **`:452`** | **`.with_slot0_cache(…)` en el constructor — hoy `SizeOptimizer::new(...)` pelado.** |
| **3b** | `backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs` (`c767f4b96d504b85f48d9b8bef8933bef7c37989`) | **`:987-989`** | **usar la `cache: Arc<ReservesCache>` que YA recibe el parámetro.** |
| **3c** | `backend/searcher-rs/src/size_optimizer.rs` (`a1411d82…`) | **`:864`** | **el doc `«prod wiring is a follow-up»` deja de ser cierto: actualizarlo o borrarlo.** |
| **5a** | `backend/searcher-rs/src/canonical_knobs.rs` (`0dfebc449b8f0ea45253d56f40acf65f3fbda97e`) | **`:176`** | **`min_pool_liquidity_usd` necesita un consumidor; y el consumidor necesita la regla de §2.2 (`NULL` ⇒ `UNREADABLE`, no exclusión).** |
| **5b** | `backend/api-server/src/routes/opportunities-live.ts` (`750c313deae43d237ce2d4d9de9a6a4b95654747`) | **`:342-353`** (el `GROUP BY` y `COUNT(*)`) + **`:1153`** (el default) | **dedup por bloque: un aterrizaje por ciclo. Y consecuentemente revisar el default `"latest"`.** |

**Rama + PR en DRAFT, SIN MERGE** (§11).

---

## 11. PUBLICACIÓN Y HUECOS

**Rama `docs/knobs-01`, PR en DRAFT, sin merge.** **El patch de código NO aterriza: `backend/` fuera de alcance.** `git add` normal · `git hash-object` · **nunca `Out-File`** · `mergeStateStatus` como esté.

**HUECOS ABIERTOS, DECLARADOS:**
1. **`s*` con y sin cache — NO COMPUTADO** (§1.3).
2. **`YELLOW` (~$92k) — NO COMPUTADO** (§2.3). **El caso testigo que el contrato pide declarar explícitamente NO SE MIDIÓ.**
3. **Las filas de 5-7 hops y su net — NO COMPUTADO** (§4).
4. **La pantalla viva por número de hops — NO COMPUTADO** (§4, §7).
5. **Frontera `StartedAt` y `:9090` — NO MEDIDOS en esta corrida** (§9.2).
6. **El `después` de PATCH 5b — NO COMPUTADO** por ausencia de patch (§3.3). **Lo único aritmético: `126.262` reemisiones dejan de emitirse y quedan `54` aterrizajes.**
7. **La causa de por qué `tvl_usd` sólo cubre el `3,2 %` de los pools activos — NO MEDIDA.**
