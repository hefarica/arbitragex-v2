# NO-LINEAR-EXTRAP-01 — `linear-extrap` y `net-per-usd-nonpositive`: un gross no-positivo × size grande da MÁS negativo

- **Rol:** Specification · tarea **t194** · `attempt_id 92ae7aa2-d9ad-41a6-95cd-4c7dbeb92ee6`
- **Ámbito:** `docs/ledger/` (único path escrito). **CERO cambios al motor, umbrales o configuración.**
- **`origin/main` medido:** `4902a47c16ef21b7683e5bf405ab49cd1c525b77`
- **Instante de la foto de datos:** **`2026-10-08 20:27:09.472956+00`** (todos los conteos de este documento son de ese instante)
- **Naturaleza:** contrato escrito. **No crea EV.** Impide que un número falso llegue a la pantalla como si fuera una oportunidad.

---

## 0. Controles de instrumento (cada uno con su exit code)

| Control | Comando | Resultado |
|---|---|---|
| **Canal** | `SELECT 1` con `--set=ON_ERROR_STOP=1 -tAc` | **`1` · exit=0** |
| **Negativo** | `SELECT esto_no_existe` **sin tubería** | **exit=1** + `ERROR: column "esto_no_existe" does not exist` |
| **Positivo del `LIKE`** | `count(*) FROM simulations WHERE fail_reason LIKE 'reverted:%'` | **5607** · exit=0 @ `20:27:09Z` |
| **Negativo `:9090` externo** | `curl -o NUL -w '%{http_code}' --max-time 8 http://195.201.235.70:9090/api/v1/query?query=up` | **`external_9090_http_code=000` · exit=7** — **medido**, no inferido |
| **`forge fmt --all --check`** | no existe | no ejecutado |

**La tabla es viva — toda cifra lleva su instante.** Serie del control positivo del `LIKE`: **4007** (t184) → **5322** (t190, apertura) → **5498** (t190, cierre) → **5607** (aquí, `20:27:09Z`). **Ninguna se reutiliza como si fuera de hoy.**

---

## 1. ★★ LOS DOS CÓDIGOS: path + línea + blob

**Ambos EXISTEN.** No hay etiqueta fantasma en esta pareja.

| Código | Path | Línea | Blob (`origin/main`) |
|---|---|---|---|
| **`net-per-usd-nonpositive`** | `backend/api-server/src/simulation/computeSimulatedNet.ts` | **`533`** (el `return` que lo emite) · **`170`**/`177` (contrato del tipo) · **`656-657`** (donde se propaga a `notes`) | **`b8ded1f91cfc6c3c1aa7d3601259fd7a71b9a22d`** |
| **`linear-extrap`** | `backend/api-server/src/simulation/computeSimulatedNet.ts` | **`600`** (`notes.push("linear-extrap")`) · **`591-593`** (la extrapolación en sí) | **`b8ded1f91cfc6c3c1aa7d3601259fd7a71b9a22d`** |
| `required_is_infinite` (el compañero) | `.../computeSimulatedNet.ts` | **`669`** (true) · **`714`** (false) · **`200`** (tipo) | *(mismo blob)* |
| La UI que lo pinta | `frontend/components/opportunities/OpportunityDetailTabs.tsx` | **`754-762`** | **`0ead2ae4f2cb4dfd15fdcff17880d294ffbcdaf3`** |
| El contrato del wire | `shared-ts/src/api-contracts.ts` | **`183-190`** | **`75d77d5558eff0fe3dbd6a9e1a07f15e903232df`** |

### 1.1 La condición EXACTA que dispara cada uno

**`net-per-usd-nonpositive`** — `computeSimulatedNet.ts:533`, función `solveDualFloors`:

```ts
if (r <= 0) return { required: Infinity, binding: "net-per-usd-nonpositive" };
```

**Condición declarada** (`:170`): *«route doesn't even cover variable costs»*. Es decir: **`r <= 0`**, donde `r` es el **net-per-USD** (`gross-per-USD` menos la tasa de costo variable, `:521-524`). **Si la ruta no cubre ni los costos variables, no hay tamaño finito que alcance el target.**

**`linear-extrap`** — `computeSimulatedNet.ts:589-600`:

```ts
if (forward != null && forward.amount_in_usd > 0) {
  const grossPerUsd = forward.gross_usd / forward.amount_in_usd;
  const effectiveGrossRate = grossPerUsd * (1 - cfg.p_copied_max - relayRate);
  netPerUsd = effectiveGrossRate - varCostRate;
  ...
  notes.push("linear-extrap");
}
```

**Condición exacta:** se entra **sólo** cuando hay un `forward` con `amount_in_usd > 0` — la rama `observed-gross`. `linear-extrap` es **una etiqueta de procedencia que declara el MÉTODO** («este `netPerUsd` se extrapoló linealmente desde el gross registrado»), **no un camino de código separado**.

### 1.2 ★ LA INTERACCIÓN QUE IMPORTA (y que responde a la tarea)

`linear-extrap` fija `netPerUsd` extrapolando; **inmediatamente después** `solveDualFloors(netPerUsd, …)` (`:649-654`) evalúa `r <= 0` y **corta con `Infinity` antes de extrapolar ningún tamaño** (`:656-670`, el `return` temprano).

> ⇒ **El sistema NO extrapola un tamaño sobre un gross no-positivo.** La extrapolación lineal existe para **derivar la tasa**, y cuando esa tasa es `≤ 0` el motor **declara que no hay tamaño** en vez de fabricar uno más grande. **La regla que #915 fijó —gross no-positivo × size grande = MÁS negativo— es exactamente la que este `return` temprano respeta.**

---

## 2. ★★ EL SINTOMA: `Infinity` — de dónde sale, y qué hace la UI

### 2.1 De dónde sale el `Infinity`

`computeSimulatedNet.ts:533` (blob `b8ded1f9…`) devuelve `required: Infinity` cuando `r <= 0`. Y se serializa **como la STRING `"Infinity"`** (`:663-669`):

```ts
// ALWAYS-COMPUTE: the "Infinity" STRING, never the number —
// JSON.stringify(Infinity) === null is how this field used to vanish
// silently on this exact branch (41/41 live rows null while the kernel
// HAD computed the verdict). The companion boolean is the machine
// filter; Number("Infinity") === Infinity recovers the numeric value.
required_amount_in_usd: "Infinity",
required_is_infinite: true,
```

**La razón del string está documentada y es deliberada** (`:183-192`): `JSON.stringify(Infinity) === null`, y `null` es **indistinguible de «no computado»** — que es **exactamente cómo el campo desaparecía en silencio en esa rama (41/41 filas vivas)**. El sentinel string es **lossless y auto-descriptivo**.

### 2.2 ★ LO QUE LA UI HACE — y aquí corrijo el enunciado del encargo

**El `∞` NO llega a la pantalla como un número que miente.** `OpportunityDetailTabs.tsx:754-762` (blob `0ead2ae4…`):

```tsx
label="Sizing: required / cap / sugerido"
value={`${
  target.required_is_infinite === true ||
  (target.required_amount_in_usd != null && !Number.isFinite(target.required_amount_in_usd))
    ? "∞ (sin tamaño finito)"
    : usd4(target.required_amount_in_usd)
} / ${usd4(target.cap_amount_in_usd)} / ${usd4(target.suggested_amount_in_usd)}`}
```

Y el comentario que lo acompaña (`:746-753`) declara la intención exacta: *«Render it as an explicit ∞ verdict — **never a dash that looks like "not computed"** (the kernel DID compute: **"no finite size reaches the target"**)»*.

> **⇒ El `∞` ya se renderiza como `∞ (sin tamaño finito)`.** La frase del encargo —*«un `Infinity` que llega a la UI es un número que miente: no es infinito, es no computable»*— **describe el riesgo correcto, y el código ya lo resuelve: el `∞` se lee como «no hay tamaño», que es exactamente «no computable», y se distingue expresamente de «no computado».** Esto **no se re-deriva: se verifica y se declara.** Lo que queda por defender es que ese manejo **no se degrade**.

### 2.3 ★★ EL SÍNTOMA NO SE REPRODUCE CONTRA LA BASE — y el dato que lo reemplaza

El encargo fija el síntoma como **«WBTC 6-hop sim a ~$81k ⇒ neto −$760»**. Medido sobre la base persistida (`opportunities.economics`, foto `20:27:09Z`):

| Medición | Resultado |
|---|---|
| 6-hop con `amount_in_usd > 50000` | **0 filas** |
| 6-hop con `economics.amount_in_usd` poblado | **11 626 filas** |
| **`max(amount_in_usd)` en 6-hop** | **1000.00** |
| `avg(amount_in_usd)` en 6-hop | 136.51 |
| `min(amount_in_usd)` en 6-hop | 0.01 |
| **`min(net_profit_usd)` en 6-hop** | **−1000.6887** |
| `min(net)` global (con `economics`) | **−999 949 711 198.8328** ← **outlier de contaminación**, se aísla y no se usa |
| `max(net)` global | 1.8706 |

> **DICTAMEN: el síntoma `−$760 @ ~$81k` se declara `NO REPRODUCIDO` contra la base persistida.** No existe ninguna fila de 6 hops con `amount_in_usd` por encima de **$1000**.
>
> **Y el dato que sí aparece es MÁS fuerte para esta tarea:** el tamaño máximo en 6-hop es **exactamente `1000.00`, que es el `capital_usd`**, y la pérdida máxima es **−1000.6887**. ⇒ **la pérdida está ACOTADA por el techo de capital, no amplificada por él.** Que es precisamente la regla del encargo: **el techo no se sube con `capital_usd` porque un gross no-positivo × size grande da más negativo — y el sistema ya lo acota.** El `−1000.6887` es el cap trabajando.

**Dónde SÍ está medido el −$760:** en el **test** `computeSimulatedNet.test.ts:527`, que **no es producción** — fija `cfg.capital_usd = 100_000` para reproducir la rama `net-per-usd-nonpositive` y su sentinel. **Es un test, y se declara como tal**; **no se extrapola a la base.**

---

## 3. ★★ ¿EL SISTEMA EXTRAPOLA HOY? Se declara sitio por sitio

| Sitio | ¿Extrapola? | En qué consiste | Diblo |
|---|---|---|---|
| `computeSimulatedNet.ts:591-593` (`linear-extrap`) | **SÍ, de forma LINEAL** | deriva `netPerUsd` = `gross_usd/amount_in_usd × (1 − p_copied_max − relayRate) − varCostRate`, **desde el gross MEDIDO de la fila** | `b8ded1f9…` |
| `computeSimulatedNet.ts:533` (`net-per-usd-nonpositive`) | **NO** | **Corta**: `r <= 0 ⇒ required = Infinity`. **No hay extrapolación de tamaño sobre un gross no-positivo** | `b8ded1f9…` |
| `computeSimulatedNet.ts:614-620` (Path B, `roi-assumed`) | **SÍ, pero declarada** | sin `forward`: usa `target.roi_pct/100` como tasa asumida; **si no hay ROI floor ni forward, `return null`** (`:615`) — no extrapola | `b8ded1f9…` |
| Tope de tamaño | `cap_amount_in_usd` (`:202`, `:670`, `:715`) | capa el tamaño sugerido al **techo de capital** | `b8ded1f9…` |

**Dictamen:** el sistema extrapola **una sola cosa: la TASA**, y lo declara con la etiqueta `linear-extrap`. **No extrapola TAMAÑO cuando la tasa es no-positiva** — y `cap_amount_in_usd` acota el resultado. **`NO COMPUTADO` no aplica aquí: los cuatro sitios se localizaron y se declaran.**

---

## 4. ★★ AUSENTE ≠ CERO — con una corrección a #915

**La regla se usa, no se re-deriva** (verbatim del encargo): *«un término que no está es `UNREADABLE`, nunca `$0`; `0,0` solo cuando es un CERO COMPUTADO»*. El ejemplo bueno citado: **`bribe_usd: 0.0`** en `economics.rs:162`, que **sí** es un cero computado (el código lo dice: *«0.0 is its TRUE computed value today»*).

### 4.1 ★ CORRECCIÓN A #915 — y cambia la lectura, no el hecho

**#915 afirmó que `dex_fees_usd` está «ausente en las 10 571 603 filas».** Medido de nuevo, y **más fino**:

| Medición @ `20:27:09Z` | Resultado | Lectura |
|---|---|---|
| `count(*)` de `opportunities` | **10 645 314** | total |
| `count(economics)` | **10 645 314** | todas traen `economics` |
| **`count(economics->>'dex_fees_usd')`** | **0** | **ninguna trae un VALOR** |
| **`economics ? 'dex_fees_usd'`** | **10 645 314** | **la CLAVE está presente en TODAS** |
| valor literal en la fila testigo | *(vacío)* + `clave_presente = t` + razón `included_in_amount_out_post_fee` | |

> **⇒ `dex_fees_usd` NO está «ausente»: está PRESENTE-COMO-`null`, con una RAZÓN DECLARADA** (`not_computed_reasons.dex_fees_usd = "included_in_amount_out_post_fee"`, constante en `economics.rs:103`).
> **Eso NO contradice** la regla de #915 (`ausente = UNREADABLE, nunca $0`) — **la afina**: aquí no hay ni ausencia ni cero; hay **un `null` con razón**, que es la forma **correcta** de declarar un término no emitido. **Lo que #915 refuta con razón no es el formato, sino que la razón sea CIERTA** (la sim mide LP $7,38 sobre la misma familia; `git grep` de `lp_fee|slippage_usd|flash_fee` en `sim-ctl/src` = 0 coincidencias).
> **La corrección se registra porque #915 la redactó como ausencia y la base dice presencia-con-null.** El hecho de fondo (el LP no se carga como línea) queda igual.

### 4.2 La regla aplicada al caso `net-per-usd-nonpositive`

`netPerUsd` se calcula (`:593`) restando la tasa de costo variable a la tasa bruta. **Si el `gross` de la fila fuera `None`, no habría `forward.gross_usd` y no se entraría a la rama `observed-gross`** (`:589` exige `forward.amount_in_usd > 0`); se caería a la rama Path B, y **sin ROI floor ni forward el resultado es `null`** (`:615`), **no `0` ni `Infinity`**.
⇒ **Si un término falta, el resultado es «no computado» (`null`/`return null`); `Infinity` solo se emite cuando el término SÍ se computó y dio `≤ 0`.** **Es la distinción que el encargo exige, y el código la respeta.**

---

## 5. Los `fee_tier` concretos — NO MEDIDOS, con la razón

**Declarado `NO MEDIDO`**, no `000` y no ausente:

- La fuente del dato real existe: `pools.fee_tier` (catálogo) y `pool_a.fee_bps` en `dex_engine.rs:439-440`.
- **No se leyeron en esta corrida**: la sesión se consumió en los códigos, el sentinel y las mediciones de la base, y **el acceso al VPS se usó prioritariamente para los controles del contrato**. **Es una omisión de presupuesto de turno, no un fallo de instrumento** — y se declara como tal.
- #915 tampoco pudo medirlo (su SSH expiró por timeout). **Sigue pendiente, ahora por segunda vez y con razón distinta.**
- El proxy prohibido sigue identificado: `unwrap_or(30)` en `dex_engine.rs:439-440`.

---

## 6. Target y apetito — intactos

| Campo | Valor @ `20:27:09Z` | Estado |
|---|---|---|
| `capital_usd` (chain 1) | **1000.00** | **intacto** |
| `min_profit_usd` | **50.0000** | **intacto** |
| `simulation_target_profit_usd` | **50.0000** | **intacto** |

- **`net_realized_usd`: la columna NO EXISTE** en `opportunities` (`information_schema` → 0 filas) ⇒ *«sigue `NULL` hasta que exista receipt»* se cumple **vacuamente**, y se declara así, no como un campo poblado.
- **El techo no se sube con `capital_usd`**: el operador ya lo midió en las dos direcciones, y §2.3 lo confirma desde la base (el tamaño máximo es el propio `capital_usd`).
- CERO mainnet, firmas o broadcast. Paper.

---

## 7. Efecto real — declarado, no prometido

**Esto NO crea EV.** Impide que un **número falso** llegue a la pantalla **como si fuera una oportunidad**.

**El resultado correcto sigue siendo el mismo, con los dos números al lado** (de #915, usados y no re-derivados):

| Extremo | Número |
|---|---|
| spine (fila testigo `949a670e`) | **+$0,0494** (`net_profit_usd` = 0.049404645914378764) |
| gross de la misma fila | 0.6746384959143795 |
| `target_delta_usd` | **−49.95059535408562** |
| **`NO OPPORTUNITY`** | **sigue siendo el veredicto** |

Y la distinción que no se pierde —**unificar el ledger / acotar la extrapolación no crea EV**: cambia **que el operador vea el número honesto sin abrir otra pestaña.**

---

## 8. Cierre

- **LOS DOS CÓDIGOS EXISTEN**, con path + línea + blob: `net-per-usd-nonpositive` en **`computeSimulatedNet.ts:533`** (blob `b8ded1f9…`, condición `r <= 0`) y `linear-extrap` en **`:600`** (mismo blob, condición `forward.amount_in_usd > 0`). **Ninguno es etiqueta fantasma.**
- **El `Infinity` sale de `:533`** y se serializa como la **string `"Infinity"`** (`:668`) precisamente para **no confundirse con «no computado»** — el string es deliberado y está documentado.
- **La UI NO miente**: `OpportunityDetailTabs.tsx:757-760` lo renderiza **`∞ (sin tamaño finito)`**, y el comentario declara *«never a dash that looks like "not computed"»*. **El manejo correcto ya existe; lo que hay que defender es que no se degrade.**
- **El síntoma `−$760 @ ~$81k` es `NO REPRODUCIDO`** contra la base: **0 filas** de 6-hop por encima de $1000. Lo que sí aparece: `max(amount_in) = 1000.00` = el `capital_usd`, y `min(net) = −1000.6887` ⇒ **la pérdida está ACOTADA por el techo, no amplificada por él.**
- **El sistema extrapola la TASA (declarado `linear-extrap`) y NO extrapola TAMAÑO sobre un gross no-positivo** (corta con `Infinity` en `:533`). La regla del encargo se cumple en el código.
- **Corrección a #915 declarada**: `dex_fees_usd` **no está ausente** — está **presente-como-`null` en las 10 645 314 filas**, con razón declarada. Afina la regla; no la contradice.
- **`fee_tier` concretos: NO MEDIDOS** con la razón (presupuesto de turno), **no `000`, no ausentes**.

*Un número que dice «∞» no está mintiendo por sí mismo: miente si se lee como una cantidad. Aquí se lee como «no hay tamaño», que es lo que el motor computó. Lo que esta acta cierra es que eso siga siendo así.*
