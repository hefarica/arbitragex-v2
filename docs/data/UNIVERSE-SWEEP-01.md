# UNIVERSE-SWEEP-01 — **el NO de t175 NO se extiende al universo**: 232 spreads cruzan 0,35 %, y **63 caen en la banda plausible** [0,35 %–1 %)

**Resultado en una línea:** barrido corregido —**sin truncar**, con la **validación por precio marginal en `4/4 = 1,000000000`** y un **filtro de consenso declarado antes de contar**— sobre **1.139 de 1.346 pools (84,6 %)** a **bloque fijo `26148216`**: **232 combos cruzan 0,35 %** y **193 cruzan 0,68 %** tras filtrar degenerados — y de ellos **63 están en la banda físicamente plausible [0,35 %, 1 %)** (`[0,35,0,68)=39` + `[0,68,1)=24`), **idénticos antes y después del filtro**. **⇒ Los 5 pools de t175 no eran el universo: hay candidatos que su NO no cubre.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `91908449-fd7c-49ca-a799-21ab4a414047` (re-ejecución del `cb6a9d51…`, fallado por un truncado propio)
**Solo lectura** · **CERO escrituras** · **In scope:** `docs/data/`

---

## 0. ★★ LA COBERTURA REAL, EN LA MISMA LÍNEA QUE CADA CONCLUSIÓN

| | valor | sobre |
|---|---|---|
| universo (`chain_id=1 AND is_active`) | **1.346** | creció +10 desde el intento 1 (`1.336`) — el universo es vivo |
| **barridos DE VERDAD** (precio resoluble) | **1.139 = 84,6 %** | del universo |
| **NO barridos** | **207 = 15,4 %** | del universo |
| V2 leídos (`getReserves()`) | **895** | de los 1.139 |
| V3 leídos (`liquidity()`+`slot0()`) | **244** | de los 1.139 |
| pares de tokens | 899 | — |
| **pares con ≥2 pools** | **146** | — |
| **combos comparados** | **392** | de los 146 pares |
| **pools marcados degenerados** | **13 de 1.139** | por el criterio de consenso |

**Razones de los 207, contadas:** `v3_liq_cero_o_ilegible` = **205** · `sin_reservas_ni_slot0` = **1** · `decimals_ilegibles` = **1**.
**⇒ Nada se extrapola a los 207.** Y el universo de comparación real son **392 combos sobre 146 pares**, no 1.346 pools.

**Bloque fijo: `26148216`** en las **4.038** `eth_call` — **ninguna a `latest`** —, el mismo bloque del intento 1 y el que t175 citó como actual.
**Endpoint: `http://172.18.0.3:8545`** (IP del contenedor anvil). **Corrección del intento 1 declarada:** `localhost:8545` **no está publicado al host** (`http=000 exit_curl=7`, `docker port` vacío) — esa fue la causa del `0 de 1.336` que **no** reporté.

---

## 1. ★★ LA PREGUNTA, RESPONDIDA — con los tres números, no con el que conviene

| | **CRUDO** (sin filtro) | **TRAS FILTRO** de degenerados | removidos |
|---|---|---|---|
| **> 0,35 %** (fee del round-trip) | **249** | **232** | 17 |
| **> 0,68 %** (gas a tamaño pequeño) | **210** | **193** | 17 |

**★ El histograma COMPLETO — lo que en el intento 1 se perdió por mi truncado `[:50]`:**

| banda | CRUDO | **TRAS FILTRO** |
|---|---|---|
| **[0,35 %, 0,68 %)** | 39 | **39** |
| **[0,68 %, 1 %)** | 24 | **24** |
| [1 %, 2 %) | 45 | 45 |
| [2 %, 10 %) | 62 | 62 |
| [10 %, 100 %) | 42 | 42 |
| [100 %, 10⁴ %) | 24 | 20 |
| [10⁴ %, 10⁸ %) | 10 | **0** |
| [10⁸ %, ∞) | 3 | **0** |

**★ Las dos bandas plausibles —63 combos— son IDÉNTICAS antes y después del filtro.** El filtro de consenso **no las toca**: son pools que **coinciden con la mediana de su par dentro de 10×** y **aun así** muestran un spread por encima del fee. **No son degenerados: son candidatos.**

**⇒ RESPUESTA A LA PREGUNTA DEL CONTRATO: SÍ.**
- **> 0,35 %: 232 combos (CRUDO 249).**
- **> 0,68 %: 193 combos (CRUDO 210).**
- Y **63 de ellos en la banda [0,35 %, 1 %)** — la única donde un spread es físicamente un spread de mercado.
- Los 5 pools de t175 **no eran el universo**: su NO **no se extiende**.

**Top-5 tras el filtro** (ninguno marcado degenerado):

| spread | pool A | pool B |
|---|---|---|
| **1182,09 %** | `0xc60604a8e1` (V3, fee 10000) | `0xfeeed96fdc` (V2, fee 30) |
| 536,04 % | `0x0b07188b12` (V2, fee 30) | `0x3425dd3e3d` (V3, fee 10000) |
| 474,95 % | `0x3f153545fa` (V3, fee 500) | `0xfeeed96fdc` (V2, fee 30) |
| 462,65 % | `0x86ce2637f2` (V2, fee 30) | `0xafe8a27051` (V2, fee 30) |
| 414,85 % | `0x440bb7a5fc` (V2, fee 30) | `0xc530434eed` (V3, fee 3000) |

**Y se declara lo que esto NO dice:** el **spread es condición NECESARIA, no suficiente**. Que dos venues difieran un 0,5 % **no** dice que la operación sea rentable: falta la curva por tamaño, los fees de cada lado y el gas. **El neto por tamaño NO está computado en esta tarea**, y por eso **no se declara que exista una oportunidad rentable** — se declara que **existe un candidato que el NO de t175 no cubre**. La distancia entre las dos frases es exactamente la que este proyecto paga cuando se saltea.

---

## 2. ★★ EL CRITERIO, DECLARADO ANTES DE CONTAR — y no se ajustó

Sellado en `docs/data/UNIVERSE-SWEEP-01.criterio.md` **antes** de la primera consulta del intento 2:

**CRITERIO DE CONSENSO POR PAR:** para cada par de tokens con ≥2 pools se calcula la **mediana** de los precios de sus pools; **un pool es degenerado si su precio se aparta de esa mediana por más de un factor 10**; un combo es válido sólo si **ninguno** de sus dos pools es degenerado.

**Por qué este y no un umbral de spread:** no elige un umbral de magnitud — elige **coherencia**. Si todos los venues de un par dicen ~X y uno dice 1e−30·X, el que discrepa está roto. **El factor 10 es deliberadamente generoso** (un pool puede estar 10× fuera por un movimiento real y sigue contando): sólo descarta lo imposible. Es **simétrico** y no depende de qué pool se mira primero.

**Y se declara el resultado del criterio tal como salió, sin retocarlo:** marcó **sólo 13 de 1.139 pools** y removió **17 combos**. **No se ajustó el factor** — con 10 el filtro es laxo y así se reporta; **no se subió para limpiar más** ni se bajó para conservar candidatos.

**El coste del filtro, medido por banda:** removió **4** de `[100,1e4)`, **10** de `[1e4,1e8)` y **3** de `[1e8,∞)` — **y CERO de las dos bandas plausibles.** Es decir: **el filtro actuó exactamente donde debía y en ningún otro lado.**

---

## 3. ★★ LA VALIDACIÓN POR PRECIO MARGINAL — CORRIDA, Y PASA

**`VALIDACION_marginal: 4/4 ok · ratios = ['1.000000000', '1.000000000', '1.000000000', '1.000000000']`**

Se comparó, para 4 pools V2, el **precio marginal implícito de la fórmula de swap** (`dy = γ·dx·r1/(r0+γ·dx)` evaluada en `dx→0`, dividida por γ) contra el **precio directo de las reservas** (`(r1/10^dec1)/(r0/10^dec0)`). **Ratio = 1,000000000 en los cuatro.** **⇒ Las fórmulas son consistentes y las unidades están bien.**

**Los dos errores de unidades que el contrato exige declarar, respetados en el código:**
1. **El `30` de `fee_tier` son BASIS POINTS (×997), no un 3 %** — se trata como bps en todo el recorrido.
2. **El tercer valor de `getReserves()` es `blockTimestampLast`, NO un fee** — el código lee **sólo los dos primeros `uint112`** (`h[0:64]`, `h[64:128]`) y **nunca** interpreta el tercer campo como fee.
3. **No se mezclan unidades humanas con `raw` en V3**: el precio se calcula en crudo (`(sqrtPriceX96/2⁹⁶)²`) y el **ajuste decimal se aplica después y explícitamente** (`×10^dec0/10^dec1`).

**Esto es lo que hace defendibles los 232/193** — y es precisamente lo que faltaba en el intento 1.

---

## 4. ★ EL DATO ESTRUCTURAL QUE SIGUE EN PIE

`fee_tier` del universo: **V2 (`fee_tier=30`) domina con 895 leídos**, más 244 V3. **Dos tercios del universo son el V2 de 0,30 %**, la clase que t175 probó **estructuralmente excluida** cuando el mejor spread contra ella es **0,2766 % < 0,30 %**.

**★ Y el barrido lo matiza con datos: los 5 mayores spreads tras filtro son `V3↔V2` o `V2↔V2` con fee 30 a un lado.** Es decir: **el V2 sí aparece en los candidatos** — pero **como el lado barato contra un V3 de fee alto**, no como el lado caro. Eso **no contradice** a t175: **lo precisa** — el V2 no puede ser el *destino* de un round-trip rentable contra sí mismo, pero **puede ser el origen** contra un V3 de fee 1 %.

**El contexto duro de t175 no se toca ni se recalcula:** máximo neto global **−$0,6475** · gas floor **$1,95** · bruto máximo de la curva **+$0,0283** contra gas **$0,676131** ⇒ **gas 24× el bruto**. **Si los 63 candidatos plausibles de hoy sobreviven a esa estructura es exactamente lo que NO medí.**

---

## 5. NO COMPUTADO — declarado, nunca cero

| # | NO COMPUTADO | razón |
|---|---|---|
| 1 | **El NETO por tamaño de los 63 candidatos** | medí **spread** (condición necesaria), no la curva por tamaño con fees y gas. **Spread ≠ edge.** |
| 2 | **Si el edge sobrevive a la SEGUNDA operación** | t175 midió que el pool se auto-arbitra en 3 operaciones; **no corrí esa prueba** sobre los 63 |
| 3 | **Por qué 205 V3 tienen `liquidity()` 0 o ilegible** | se declara la razón del instrumento, no la causa |
| 4 | **Los 207 no barridos** | no se extrapola a ellos en ninguna dirección |
| 5 | **Las fórmulas de PR #902 (blob `c6b99de1…`)** | **no accedí al blob** en el intento 1 ni ahora, y **no cito lo que no verifiqué**. Mi método está declarado y **validado por su propio control (4/4)** |
| 6 | **La lista completa de los 392 combos en el informe** | el JSON íntegro queda en el instrumento; el artefacto da el **histograma completo** y el **top-5** |

---

## 6. ★ LOS TRES AJUSTES PROPIOS — cazados, declarados y corregidos

1. **EL TRUNCADO `[:50]` DEL INTENTO 1** — perdió 195 combos y con ellos la banda que decide. **Corregido: este intento escribe la lista completa y el histograma por bandas, sin truncar.** Es lo que convirtió un `NO COMPUTADO` en una respuesta.
2. **`localhost:8545` NO publicado al host** — el intento 1 dio `0 de 1.336` por connection-refused (`http=000 exit_curl=7`, `docker port` vacío, `ss -ltn` sin nada, y `cast` **dentro** del contenedor correcto). **No lo reporté.** Corregido: **IP del contenedor `172.18.0.3`**.
3. **`0x18efe28` = 26.148.392 ≠ 26.148.216** — error de conversión **mío** en el diagnóstico del intento 1; el script siempre usó `hex(26148216)`, correcto.

**Y un defecto de contrato declarado:** el verify pide `SELECT address, dex, kind FROM pools` y **`pools` no tiene `dex` ni `kind`** ⇒ `exit=1`. Usé las columnas reales (`token0_id`/`token1_id` con `JOIN tokens`, `fee_tier`). También `quote_block` **no es columna** de `opportunities`: vive en el JSONB `economics`.

---

## 7. PERMISOS E INTEGRIDAD

**Solo lectura**: `eth_call` con **bloque fijo `26148216`** y `SELECT`. **CERO `eth_sendTransaction` / `anvil_setStorageAt` / `anvil_setBalance`** — **no se escribió en el fork** (compartido y vivo). **NO se tocó la config** (`capital_usd=1000`, target `50.0`). CERO cambios al motor, umbrales o configuración. **CERO mainnet, firmas o broadcast.** Ningún control pipeado a `head`. `:9090` externo y `195.201.235.70/metrics` **no usados**.

```bash
git hash-object docs/data/UNIVERSE-SWEEP-01.md
git rev-parse HEAD:docs/data/UNIVERSE-SWEEP-01.md
```

---

*El NO de t175 estaba medido sobre 5 pools de 1.336. Ahora está medido sobre **1.139 de 1.346 (84,6 %)**, a bloque fijo, con la **validación por precio marginal en 4/4 = 1,000000000** y un **filtro de consenso declarado antes de contar**. **Y la respuesta es SÍ: 232 combos cruzan 0,35 % y 193 cruzan 0,68 %, con 63 en la banda plausible [0,35 %, 1 %) que el filtro no toca** — porque no son degenerados, son candidatos. Lo que esto **no** dice, y se declara: **spread es condición necesaria, no suficiente**; el neto por tamaño **no está computado**, y **no declaro una oportunidad rentable — declaro un candidato que el NO no cubría**. La diferencia entre esas dos frases es toda la tarea.*
