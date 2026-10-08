# CANDIDATE-NET-01 — **de los 63 plausibles sólo 37 tienen neto computable, y los 37 son NEGATIVOS**: el máximo es **−$0,674209** y el gas es **352×** su bruto

**Resultado en una línea:** con las **cuatro direcciones** por combo (el fix del intento 1) hay **276 de 392 combos con neto computable**. En la banda plausible **[0,35 %, 1 %)**, de los **63 combos** sin degenerar **26 no tienen precio USD en ninguna dirección ⇒ NO COMPUTADOS** (no ceros), y **los 37 medibles son TODOS negativos**: su **máximo es −$0,674209** @ $1,00, y **el que los mata es el GAS — $0,676131 contra un bruto de $0,0019 = 352×**. Los **12 combos que sí cruzan el floor de $1,95** tienen **todos** spread **≥ 5,6307 %**, **cero por debajo del 1 %** ⇒ son la misma patología por debajo del factor 10 del filtro de consenso, **no oportunidades**. Y mi **−$0,674209** reproduce el **−$0,6475 de t175** por otra población y otro método.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `84e5f084-408b-437b-8ee4-fa9ac383da8b` (intento 2; el 1 falló por tres defectos propios)
**Solo lectura** · **CERO escrituras** · **No se escribió en el fork** · **In scope:** `docs/data/`

---

## 0. ★★ LA PREGUNTA BINARIA, CON SU DENOMINADOR

| pregunta | respuesta | población |
|---|---|---|
| **¿ALGUNO de los 63 plausibles cruza $1,95?** | **NO en los 37 medibles · 26 NO COMPUTADOS** | 63 combos sin degenerar con s∈[0,35 %, 1 %) |
| **máximo neto de la banda plausible** | **−$0,674209** @ **$1,00** · spread **0,8574 %** · `0x0149ebe93026…` / `0x18cd890f4e23…` · `deg=False` | 37 medibles |
| **distribución del neto en la banda** | min **−$1,676131** · mediana **−$0,972156** · **max −$0,674209** | 37 medibles |
| **¿ALGUNO de los 392 cruza $1,95?** | **SÍ — 12 combos** | 276 con neto |
| **máximo neto global** | **+$494,165080** @ **$500** · spread **414,8518 %** · `0x440bb7a5fc57…` (V2 f30) / `0xc530434eed95…` (V3 f3000) · `deg=False` | 276 con neto |
| **★ spread MÍNIMO entre los 12 que cruzan** | **5,6307 %** | 12 |
| **★ cuántos cruzan con spread < 1 %** | **0** | 12 |

**★ LOS DOS «63» Y LOS DOS «13» — el instrumento imprime poblaciones distintas en la misma línea, y separarlas es parte del resultado:**

| número | población | fuente |
|---|---|---|
| **63** | **combos** sin degenerar con s∈[0,35 %, 1 %) — sobre los 392 | `/tmp/t183b.log` línea `plausibles_0.35_1=63` |
| **37** | esos mismos **combos, restringidos a los que tienen neto computable** | `RESULTADO … plausibles=37` |
| **232** | **combos** sin degenerar con s>0,35 % | `limpias_0.35=232` |
| **13** | **POOLS** marcados degenerados por el filtro de consenso (**no** combos) | `pools_marcados_degenerados=13 de 1139` (t177) / `len(deg)` en t183b |
| **17** | **combos** contaminados por esos 13 pools | recomputado sobre `todos`; coincide con `removidos_por_filtro=17` de t177 |

**⇒ Sin esa separación, «13 degenerados» se leería como 13 combos (son 17) y «63 plausibles» como 63 medidos (son 37). Los 26 restantes son NO COMPUTADO — no un cero y no un NO.**

**★★ LA BIFURCACIÓN ES EL RESULTADO:** los 12 que cruzan **no tienen nada que ver con los 63**. Sus spreads viven en `[1,10)=1 · [10,100)=4 · [100,10⁴)=4 · [10⁴,∞)=3` — **5,63 % a 180.834 %**, fuera de cualquier banda de mercado. **7 de los 12 son `deg=False`**, o sea pasaron el filtro de consenso **por diseño** (el factor 10 tolera razones hasta 10× ⇒ spreads hasta 900 %): **no lo ajusto, lo reporto.**

**⇒ La respuesta honesta tiene tres partes y ninguna se elige:**
1. **Sobre los 37 medibles: NO. Ninguno cruza el floor a ningún tamaño.** Cerrado por medición.
2. **Sobre los 26 restantes: NO COMPUTADO**, causa exacta `sin_precio_4_direcciones` — ninguno de los **dos** tokens del par tiene precio USD configurado (`precios_config=21`), y las cuatro direcciones cubren ambos tokens. **No se degrada a NO.**
3. **Sobre los 12 que cruzan: no son oportunidades.** Un spread de 5,63 %–180.834 % entre dos venues del **mismo par** no es un edge de mercado: es la degeneración que t177 apartó, **por debajo del umbral de su filtro**.

---

## 1. ★★ CUÁL DE LAS TRES FRICCIONES MATA — **y depende de la población**

### En la banda plausible: **GAS**

Mejor candidato: **neto −$0,674209** con gas **$0,676131** ⇒ **bruto = +$0,0019223508**.
**⇒ el gas es `0,676131 / 0,0019223508` = 351,7× el bruto.**
t175 había medido **24×** en *su* pool; en el mejor de los 37 es **casi 15× peor**. **A ese tamaño el fee ($0,006) y el impacto ($0,00065) son ruido: el bruto entero es ruido frente al gas.**

**★ El gas NO se midió acá: `GAS_USD=0.676131` está fijado en el instrumento como valor de t175 y el comentario del código lo dice — `# de t175, NO se recalcula`.** Igual el floor `1.95`, también reusado. **Declarado: este informe no re-mide gas ni floor.**

### Entre los 12 que cruzan: **IMPACTO** (y el gas deja de existir como fricción)

| @ tamaño | ideal | **fee** | **impacto** | bruto | gas | neto |
|---|---|---|---|---|---|---|
| **$500** | 2074,259152 | 151,500000 | **1427,917940** | 494,841211 | 0,676131 | **+494,165080** |
| **$1000** | 872,053533 | **303,000000** | 291,943768 | 277,109765 | 0,676131 | +276,433634 |
| **$1** | 1808,341879 | 0,053000 | **1739,278914** | 69,009965 | 0,676131 | +68,333834 |

**★ El impacto se come el 68,8 % del ideal en el máximo** (1427,917940 de 2074,259152 a $500). Y hay una **inversión por tamaño que el dato muestra sin que yo la imponga**: a **$1** el fee es **$0,053** y el impacto **$1.739,28** (manda el impacto); a **$1000** el fee trepa a **$303** y el impacto baja a **$291,94** (manda el fee, porque crece linealmente con el notional). **El gas es 0,14 % del bruto en el máximo: a esos tamaños el gas no decide nada.**

**⇒ Las tres fricciones se separan con números y NO matan a la misma población: el gas mata a los 37 medibles; el impacto (y luego el fee) a los 12 — que no deberían contarse.**

### Dónde está el máximo, declarado

**En `$1,00`** el máximo de la banda plausible **y 3 de los 12**; reparto completo del tamaño del máximo de los 12: `{$1: 3, $10: 3, $50: 2, $500: 2, $1000: 2}`. La escalera del instrumento es `[1, 10, 50, 100, 200, 500, 1000]`.
**★ El máximo plausible está en `$1,00` — el tamaño MÁS CHICO de la escalera.** El mejor candidato ya es negativo en su mejor punto **y empeora con el tamaño**: no hay tamaño donde el operador pueda rescatarlo.

---

## 2. ★★ LA VALIDACIÓN POR PRECIO MARGINAL, RE-CORRIDA SOBRE LA CURVA

**`VALIDACION_marginal: 3/4 ok · ratios = ['1.000000000', '0.999999984', '1.000000000', '1.000000000']`**

**3 de 4 dan `1.000000000` exacto; el cuarto `0,999999984` — desvío `1,6 × 10⁻⁸`**, precisión de punto flotante, **no** error de unidades. **Se declara tal cual y NO se redondea a 4/4.** Y **declaro la diferencia con t177**: su corrida imprimió `4/4 ok` (samples `'1.000000'`, tolerancia `|v−1|<1e-9`) ⇒ **el cuarto ratio es dependiente de la muestra** (t177 caminó 1.139 pools, esta corrida 1.140), y en esta corrida cae **1,6e-8 afuera**. **3/4 estricto, 4/4 dentro de 1e-6.**

**Los dos errores de unidades que el contrato manda cazar, respetados:** `fee_tier` tratado como **BASIS POINTS** (`gamma = 1 − fee/10000`) y **el tercer valor de `getReserves()` (`blockTimestampLast`) no se lee como fee** (sólo los dos primeros `uint112`).

**⇒ La validación pasó ⇒ la maquinaria es correcta y la curva es defendible.** Es lo que permite entregar un **NO** en vez de un NO COMPUTADO **sobre los 37**.

---

## 3. ★ EL CONTROL QUE HIZO POSIBLE ESTE RESULTADO: la reproducción de t177

El re-barrido a **bloque fijo `26148216`** reproduce a t177 **al entero, con sus mismas definiciones**:

| métrica | t177 (`/tmp/t177b.log`) | t183b (`/tmp/t183b.log`) |
|---|---|---|
| `pares=899` · `pares_multi=146` | 899 · 146 | combos **392** (mismo grafo) |
| `pools_marcados_degenerados` | **13** de 1.139 | `degenerados` **13** |
| `LIMPIO >0,35 %` | **232** (CRUDO 249, removidos 17) | `limpias_0.35` **232** |
| banda `[0,35 %, 1 %)` limpia | **63** (hist `39 + 24`) | `plausibles_0.35_1` **63** |
| `cruzan_plausibles` | — | **0** |

**★ La banda [0,35 %, 1 %) = 63 idéntica antes y después del filtro** (t177: `hist_todo` y `hist_limpio` coinciden en `[0.35,0.68)=39` y `[0.68,1)=24` ⇒ el filtro **no removió ni un combo de la banda plausible**). **El filtro de consenso no toca la banda que importa — y la banda igual muere por gas.**

Y el resultado de hoy **reproduce el número de t175 por otra vía**: **t175 midió neto global `−$0,6475`; mi mejor plausible da `−$0,674209`** — **mismo signo, mismo orden, 2,7 centavos de diferencia**, sobre **otra población (37 combos vs sus 5 pools)** y **otro método (curva por tamaño + 4 direcciones vs su barrido)**. **Dos mediciones independientes que coinciden en signo y magnitud es lo más cerca que esta campaña ha estado de confirmar algo.**

---

## 4. COBERTURA, EN LA MISMA LÍNEA QUE CADA CONCLUSIÓN

| | valor |
|---|---|
| universo (`chain_id=1 AND is_active`) | **1.346** |
| **barridos DE VERDAD** | **1.140 = 84,7 %** (`con_precio=1140`) |
| **NO barridos** | **206** — `v3_liq_cero_o_ilegible` **205** · `decimals_ilegibles` **1** |
| **la ÚNICA diferencia con t177** | t177 tenía **1.139 / 207** con `sin_reservas_ni_slot0: 1`; **ese pool pasó a ser legible** y el resto es idéntico |
| `decimals_ok` | **902/903** |
| combos | **392** (sobre **899 pares**, **146 con ≥2 pools**) |
| **con neto computable** | **276** |
| **sin neto** | **116** — motivo único **`sin_precio_4_direcciones`** |
| precios configurados | **21 tokens** (`precios_config=21`) |
| `eth_call` ejecutadas | **4.038**, **todas** con bloque fijo (`hex(BLOCK)` en los params) |

**★ El `116` y los `26` de la banda son NO COMPUTADO, no ceros.** Y **nada de este informe se enuncia sobre «el universo»**: se enuncia **sobre los 392 combos** (276 con neto, 37 plausibles medibles), y se dice.

---

## 5. LA FRONTERA CANDIDATO / OPORTUNIDAD — y el veredicto que se puede firmar

t177 la fijó: *«No declaro una oportunidad rentable: declaro un candidato que el NO no cubría.»*

**Hoy el NETO se computó donde había precio, y hace lo que el neto hace: decide.**
- **Plausibles medibles: candidatos → NO.** El neto los mata y los mata el **gas (351,7× el bruto)**. **Cierre del lado de detección por la banda que t177 abrió**, medido, no extrapolado.
- **Plausibles sin precio: NO COMPUTADO** con causa exacta. **No se visten de NO.**
- **Los 12 que cruzan: siguen sin ser oportunidades.** No porque el neto sea negativo —es **positivo**, hasta **+$494**— sino porque **su spread (≥ 5,63 %) no es un spread de mercado**. **Un neto positivo sobre un precio roto no es un edge: es la misma degeneración con un número más grande.**
- **Y ni siquiera los 12 se declaran rentables en producción:** flujo real, competencia e impacto acumulado **no son computables con lo medido**. **No se mide «cuánto se podría ganar».**

**★ LA SUPERVIVENCIA A LA SEGUNDA OPERACIÓN: NO COMPUTADA para los 12, con la razón.** No se puede medir sin **escribir en el fork** (prohibido: es compartido y vivo, y el simulador re-siembra en cada simulación), y **los 37 medibles no tienen primera operación que sobrevivir** porque ninguno cruza. El dato de t175 se reusa **sin re-derivarlo**: su pool se auto-arbitraba en **3 operaciones** (`+0,1231 % → +0,0599 % → −0,0034 %`) y a $100/op, de 930 intentos, **0 con net>0**.

---

## 6. ★ LOS TRES DEFECTOS PROPIOS DEL INTENTO 1 — cazados, corregidos y declarados

| # | defecto | cómo se cazó | corrección |
|---|---|---|---|
| 1 | **`net_curve` sólo operaba `token0 → token1`** ⇒ `sin_precio_usd = 392`, el 100 % | **WETH es `0xc02aaa39…`, dirección ALTA ⇒ es `token1`**; el precio del `token0` (exótico) no existe | **cuatro direcciones por combo**: (pool A o B) × (entrar por `token0` o `token1`) |
| 2 | **La guarda exigía pools ESPEJADOS** (`Q.t0 == P.t1`) ⇒ rechazaba **todos** los pares | tras (1) seguía en `con_neto=0`: dos pools del mismo par comparten el **mismo orden canónico** `token0 < token1`, **no** están espejados | `P.t0 == Q.t0 and P.t1 == Q.t1` (mismo par, mismo orden) |
| 3 | **Una línea basura** (`d_in,d_mid,d_out=d0,d1,_=0,0,0`) habría dado `NameError` | lectura del propio código antes de correr; `python3 -m py_compile` ⇒ **`SYNTAX_OK`** | eliminada |

**Los tres se declaran, y el efecto de los tres fue el mismo: un `0` que parecía un resultado.** El intento 1 publicó ese `0` como **NO COMPUTADO** en vez de como NO, y **esa decisión es lo que permitió que el intento 2 lo resolviera en lugar de haber cerrado la campaña con un falso NO.**

---

## 7. INSTRUMENTO Y PERMISOS

| artefacto | tamaño | md5 |
|---|---|---|
| `/tmp/t183b-net.py` | 9.760 B | **`e3b124d270fa0d75674da130e5980fc9`** |
| `/tmp/t183b-out.json` (lista **COMPLETA**, 392 combos + top10) | **139.863 B** | **`419a219490593c7b3e133d2091218695`** |

**RPC `http://172.18.0.3:8545`** (IP del contenedor anvil) — **no `localhost:8545`**. **Bloque fijo `26148216`**, leído una vez con `cast block-number` e inyectado como `hex(BLOCK)` en las **4.038** `eth_call`: **cero `latest`**.
**`grep -c 'eth_send\\|setStorageAt\\|setBalance\\|anvil_'` sobre el instrumento ⇒ `0`** (exit 1): **NO hubo una sola escritura al fork.** `capital_usd` sigue **1000.00** y el target **50.0**: **NO se tocaron** — cambiarlos es decisión del operador. CERO mainnet, firmas o broadcast. **Paper.**

**Se respeta sin rellenar:** el blob `c6b99de1…` de PR #902 **no se cita** (no se accedió; el método propio está declarado y **validado por su propio control**); no se reabre lo NO COMPUTADO de t177 (los 205 V3 con `liquidity()` 0, los 206 no barridos). **El gas y el floor son de t175 y se declaran como tales.** **La lista completa de los 392 combos vive en el instrumento, sin truncar.**

```bash
git hash-object docs/data/CANDIDATE-NET-01.md
git rev-parse HEAD:docs/data/CANDIDATE-NET-01.md
```

---

*La pregunta binaria tiene respuesta y se bifurca limpio. **Sobre los 37 medibles de la banda plausible: NO** — ninguno cruza el floor de $1,95 a ningún tamaño; su máximo es **−$0,674209 en el tamaño MÁS CHICO de la escalera**, y el que los mata es el **gas, 351,7× el bruto**. **Sobre los 26 restantes: NO COMPUTADO**, por `sin_precio_4_direcciones` — ni cero ni NO. **Sobre los 12 que sí cruzan: no son oportunidades** — todos con spread **≥ 5,63 %**, ninguno por debajo del 1 %, y siete pasaron el filtro de consenso simplemente porque el factor 10 lo permite por diseño. **El neto hizo lo que tenía que hacer: convirtió candidatos medibles en un NO, dejó los no medibles como NO COMPUTADO, y dejó a los 12 donde estaban — en la degeneración.** Y la confirmación que no esperaba: **el −$0,674209 de hoy reproduce el −$0,6475 de t175** por otra población y otro método, mismo signo y mismo orden.*
