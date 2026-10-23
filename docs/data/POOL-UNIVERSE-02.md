# POOL-UNIVERSE-02 — los 41 pares: ¿hay dislocación cross-venue real fuera del universo activo?

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · `phase_id=implementation` · **Dueño:** Data · **Intento:** `b5124fe9-966d-49f8-9964-18926436f592`
**Respuesta corta: NO.** Ninguno de los 41 pares es **a la vez** líquido y admisible, así que **no hay dislocación ejecutable** que el cuarto techo pueda habilitar. El razonamiento son tres puertas medidas, no una intuición.
**Alcance:** chain 1 sola · lecturas `eth_call` a **bloque fijado** · **CERO escritura on-chain** · **In scope:** `docs/data/` · **Out of scope:** `backend/`, `shared-rs/`, `docker/`, `contracts/`, `.github/`, `frontend/`, `docs/backend/`, `docs/release/`, `docs/sre/`, `docs/review/`, `docs/spec/`, `.gitignore`

---

## 0. Control de instrumento

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal VPS→Postgres** | `psql -U postgres -d arbitragex -tAc "SELECT 1"` | **`1`** (antes de cada cifra) | 2026-10-08T07:13:10Z |
| `cast` | `cast --version` | `cast Version: 1.7.2-nightly` | 2026-10-08T07:12Z |
| **selectores** | `cast sig "getReserves()"` etc. | `0x0902f1ac` · `slot0()`→`0x3850c7bd` · `token0()`→`0x0dfe1681` · `fee()`→`0xddca3f43` · `liquidity()`→`0x1a686502` | 2026-10-08T07:12Z |
| **control de `cast sig`** | `cast sig "transfer(address,uint256)"` | `0xa9059cbb` (esperado `0xa9059cbb`) ✔ | 2026-10-08T07:12Z |
| **bloque fijado** | `cast block-number` en DOS RPC | **`26146111`** en `eth.drpc.org` **y** `ethereum-rpc.publicnode.com` | 2026-10-08T07:12Z |

Nunca usé `latest`: **todas** las lecturas on-chain son a **bloque `26146111`**.

### Tres defectos de instrumento MIOS, cazados y declarados

1. **Cota de lote por RPC (medida):** `eth.drpc.org` **capea el batch en 3** (`n=10/25/40 → HTTP 500`), mientras `publicnode` aguanta 40. Diacnóstico explícito; el primario pasó a publicnode y drpc quedó de control cruzado.
2. **★ Guardia de liquidez ausente → +2,6e51 bps.** Mi primer cálculo devolvió bordes de **10⁵¹ bps**, que es físicamente imposible. **No lo reporté: lo diagnostiqué.** Causa medida: **35 pools V3 con `liquidity() == 0`** (y `sqrtPriceX96` en los **bordes**: uno exactamente en `MAX_SQRT_RATIO`, otros en `2^96`). *Un pool sin liquidez no tiene precio ejecutable; su `sqrtPriceX96` es un residuo.*
3. **★ Decimales intercambiados en la rama V2 invertida → factor 10²⁴.** Escribí `(r0/10^d_in)/(r1/10^d_out)` donde va `(r0/10^d_out)/(r1/10^d_in)`. Lo cacé **preguntándole al número absurdo**: P13 daba +1e28 bps cuando la cuenta a mano da ~0. Corregido, P13/14/19/21 dan **+0,03 bps**, que es exactamente lo que predice la mano.

**Los dos defectos producían números plausibles-en-apariencia.** Van declarados porque ese es el modo de fallo que esta campaña ya pagó diez veces.

### Controles cruzados que SÍ cerraron

| control | resultado |
|---|---|
| `token0()` on-chain vs la base, **88/88** pools | **coinciden=88, discrepan=0** — valida el mapeo de tokens de la base **y** mi decodificador |
| `decimals()` on-chain vs la base, 47 tokens | **coinciden=47, discrepan=0** |
| `getReserves()` en los DOS RPC (muestra) | **byte-idéntico** en los 3 pools que responden; los otros 3 **revierte en ambos** (difieren sólo en que drpc añade `'data':'0x'`) — discrepancia **explicada**, no ignorada |

---

## 1. Los 41 pares: de dónde salen y de qué medición

**Mi propia medición** (el capitán corrigió mi contrato: usar la mía y declararla):

```sql
SELECT 'activos='||count(*) FILTER (WHERE is_active)||' total='||count(*) FROM pools;
-- activos=1314 total=4216          (2026-10-08T07:13:10Z, chain 1 sola)
```
```sql
WITH p AS (SELECT LEAST(token0_id,token1_id) a, GREATEST(token0_id,token1_id) b, is_active FROM pools WHERE chain_id=1),
     g AS (SELECT a,b, count(*) FILTER (WHERE is_active) n_act, count(*) n_tot FROM p GROUP BY a,b)
SELECT 'con_>=2_activos='||count(*) FILTER (WHERE n_act>=2)
    ||' pasarian_por_inactivos='||count(*) FILTER (WHERE n_act<2 AND n_tot>=2)||' pares_distintos='||count(*) FROM g;
-- con_>=2_activos=199 pasarian_por_inactivos=41 pares_distintos=3754
```

**El universo es un sistema VIVO y esto ya se ve en tres mediciones:** t123 **4207/1305** → t135 **4215/1313** (07:02:31Z) → **t136 4216/1314** (07:13:10Z). Los 41 pares se derivan de **mi** medición (4216/1314), y el conteo **coincide con la verificación independiente del capitán** (`199 | 41 | 240`). `FILTER` explícito y columna **`is_active`** (el verify del contrato traía `WHERE active`, que habría dado `column active does not exist`).

**Inventario medido: 88 pools** en los 41 pares (27 V2 + 61 V3 según qué función responde on-chain, que es como se clasifican — **no** por el metadato de la base). Lista completa de direcciones en el artefacto de trabajo y en la tabla de §4.

---

## 2. Las tres puertas, medidas

### Puerta 1 — ¿HAY LIQUIDEZ? **No, en 34 de los 41 pares.**

| estado de los 88 pools (on-chain, bloque 26146111) | n |
|---|---|
| **utilizables** | **33 / 88** |
| descartados: V3 con **`liquidity() == 0`** | **35** |
| descartados: V2 con **una reserva en 0** | 4 |
| descartados: V2 en polvo (< 1 unidad humana por lado) | 10 |
| descartados: V3 en polvo virtual (< 1 unidad humana por lado) | 6 |

**34 de los 41 pares NO tienen ni siquiera 2 pools con liquidez.** Sin dos patas con algo que mover, **no hay round trip que calcular** — no es un cero, es una imposibilidad estructural.

### Puerta 2 — ¿EL MOTOR PODRÍA ADMITIR LOS TOKENS? **No: 41 de 47 tokens están inactivos.**

```sql
... SELECT 'tokens_distintos='||count(*)||' activos='||count(*) FILTER (WHERE is_active)||' INACTIVOS='||count(*) FILTER (WHERE NOT is_active) FROM tt;
-- tokens_distintos=47 activos=6 INACTIVOS=41
```

Los **6** admitidos son **USDT, WETH, USDC, DAI, TUP, ZCX**. Los otros **41** —`HSBC`, `Tether` (dos duplicados de 18 y 6 decimales), `MUSDT`, `CBTC`, `AURA`, `LEGION`, `KMX`, `SWDC`, `ZVT`, `SARP`, `CRAZY`, `RCH`, `USR`, …— tienen `tokens.is_active = false`.

**El motor rechaza por token antes de evaluar** (`TokenNotAllowed:0x…` es una familia de rechazo viva). Un par con un token no admitido **no llega a candidato aunque sus pools se indexen**.

### Puerta 3 — LA ARITMÉTICA, para los que sobreviven: 7 pares, y **ninguno admisible**

Con las dos puertas anteriores, la única tabla con sentido es la de los pares que tienen ≥2 pools con liquidez (7 de 41). Aritmética por par: **bruto** = producto de las tasas marginales de las dos patas sin fees; **fees** = suma de las dos patas (V2 = 30 bps, constante del protocolo; V3 = `fee()`/100); **neto** = bruto aplicando ambas fees. **Hurdle = 59,91 bps**, y ya incluye las dos patas, así que la lectura primaria es **bruto > hurdle**.

| par | tokens | pools / utilizables | bruto (bps) | fees (bps) | neto (bps) | ¿ambos tokens admitidos? |
|---|---|---|---|---|---|---|
| **P29** | SWDC / USDC | 2 / 2 | **+33 919,41** | 1+25 = 26 | +33 805,23 | **NO** (SWDC inactivo) |
| **P2** | RND / LTN | 2 / 2 | **+32 634,01** | 30+30 = 60 | +32 378,59 | **NO** (ambos inactivos) |
| **P11** | OPN / BTN | 2 / 2 | **+4 608,54** | 30+30 = 60 | +4 521,02 | **NO** (ambos inactivos) |
| **P20** | SWDC / USDT | 2 / 2 | **+1 703,68** | 5+25 = 30 | +1 668,58 | **NO** (SWDC inactivo) |
| **P30** | BO / USDC | 2 / 2 | **+100,85** | 100+30 = 130 | **−30,16** | **NO** (BO inactivo) |
| **P9** | Tether / Tether | 4 / 4 | **+88,00** | 30+30 = 60 | **+27,56** | **NO** (ambos inactivos) |
| **P13** | LEVY / USDT | 2 / 2 | **+0,03** | 30+30 = 60 | **−59,88** | **NO** (LEVY inactivo) |

**Distribución del mejor borde bruto (7 pares):** mín **+0,03** · mediana **+1 703,68** · máx **+33 919,41** bps. **6 de 7 superan el hurdle** en bruto; **2 de 7** lo superan en neto.

**Y la línea que decide la tarea:**

> **Pares con AMBOS tokens admisibles por el motor: 1** (P10, TUP/WETH).
> **Pares admisibles Y con ≥2 pools con liquidez: 0.**
> **De los computables admisibles, borde bruto > hurdle: `0/0`.**

**P10 no es computable**: de sus 2 pools, el V3 tiene `liquidity() == 0` y sólo sobrevive el V2 (TUP/WETH, `act=true`). Con una sola pata con liquidez no hay round trip — es exactamente el caso `single_pool_no_spread`.

---

## 3. La respuesta

**NO hay dislocación cross-venue EJECUTABLE fuera del universo activo que exceda el hurdle de 59,91 bps. Y no por un margen: por tres razones independientes, cada una medida.**

1. **Liquidez:** **34 de 41 pares** no tienen 2 pools con liquidez. **55 de los 88 pools** son inutilizables — 35 V3 literalmente vacíos (`liquidity()==0`).
2. **Admisibilidad:** **41 de 47 tokens** están fuera de la allowlist del motor. **Un solo par** tiene ambos tokens admitidos, y **no tiene dos patas con liquidez**.
3. **Los bordes brutos que sí aparecen (6 de 7 sobre el hurdle) viven TODOS en pares con al menos un token no admitido.** Es decir: aun en el caso hipotético de que indexar los habilitara, **el gate de token los rechazaría antes de evaluar**. No son oportunidades escondidas; son pools de tokens que el motor no puede tocar.

**Un resultado negativo es un resultado:** el cuarto techo queda **medido y cerrado como «no importa»**. La foto económica se completa: no se está perdiendo dinero por el universo faltante.

**Y el COROLARIO DE t135 queda sostenido, sin re-derivarlo desde cero:** **`single_pool_no_spread` (175 555, el bucket de rechazo más grande) NO se arregla aflojando un umbral** — el par no tiene contraparte, y aflojar el gate no crearía ni una pata. Esta tarea lo confirma con el caso P10: el único par con ambos tokens admitidos tiene **una sola pata con liquidez**; su rechazo no es de umbral, es de contraparte inexistente.

**Advertencias de instrumento que acompañan al resultado (obligatorias):**

- **Una lectura de reservas es una FOTO de un bloque** (`26146111`), no una oportunidad persistente. Los precios marginales se mueven.
- **El hurdle ya incluye las dos patas**; por eso la lectura primaria es bruto vs hurdle. Se da también el neto para quien prefiera la lectura alternativa.
- **El valor en USD de estos tokens es NO COMPUTADO** (no usé oráculo de precios). Un borde nominal sobre tokens sin valoración **no es un borde económico** — y por eso los +33 919 bps de P29 no se reportan como dinero.

---

## 4. Límite del instrumento y puntos NO COMPUTADOS

| # | Qué NO está computado | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **Valor USD del borde** en los 7 pares computables | No usé oráculo de precios; los tokens involucrados están fuera de la allowlist y sin precio de referencia | Un oráculo de precios para esos 47 tokens (o la razón por la que el motor los mantiene fuera) |
| 2 | **Impacto por tamaño** (liquidez EJECUTABLE) | Una lectura de reservas no dice cuánto se puede mover **antes** de cruzar el precio. Declaro el **piso de profundidad**: ≥ **1,0 unidad humana por lado** (y, en V3, `liquidity() > 0` con precio **fuera de los bordes**). Para tamaños mayores, **NO COMPUTADO** | Curva de precio por tamaño (ticks de V3 / `getAmountOut` a tamaños declarados) sobre los 33 pools utilizables |
| 3 | **Censo on-chain completo** del mecanismo (i) | 41 pares derivados de la base; el `observed_unindexed_pairs` (6 666) sigue siendo proxy | `PoolCreated` por factory vía RPC sobre ventana declarada, contra `pools` |
| 4 | **Por qué estos pools tienen `liquidity()==0`** | Medido **que** ocurre (35 pools) y **que** está en el 96,8 % `alchemy` (t135) | El gate de activación de ese camino de descubrimiento |
| 5 | **Reconciliación del 25,40 % de t79** (arrastrado de t135) | Sigue sin el criterio de clasificación de t79 | Ese criterio |

---

## 5. Método y reproducción

```bash
# canal
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"
# universo (FILTER explicito, columna is_active)
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT 'activos='||count(*) FILTER (WHERE is_active)||' total='||count(*) FROM pools"
# selectores
cast sig "getReserves()"   # 0x0902f1ac     cast sig "slot0()"  # 0x3850c7bd
# reservas a BLOQUE FIJO (NUNCA latest)
cast call <pool> 'getReserves()(uint112,uint112,uint32)' --block 26146111 --rpc-url https://ethereum-rpc.publicnode.com
cast call <pool> 'slot0()(uint160,int24,uint16,uint16,uint16,uint8,bool)' --block 26146111 --rpc-url https://ethereum-rpc.publicnode.com
cast call <pool> 'liquidity()(uint128)' --block 26146111 --rpc-url https://ethereum-rpc.publicnode.com
```
**Artifacto de trabajo** con los 88 pools y sus valores crudos on-chain: `t136-onchain.json` · aritmética: `t136-final.json` (fuera del repo, efímeros). **RPC primario:** `ethereum-rpc.publicnode.com` · **control cruzado:** `eth.drpc.org`.

---

## 6. Integridad

El `sha256` de este documento se declara **en el cierre de t136** (mensaje al capitán y `output` de la tarea): un archivo no puede contener su propio hash sin cambiar ese hash. Este artefacto se publica **junto con** `docs/data/POOL-UNIVERSE-01.md` (t135, sha256 `8867292354914B518B3C4B9813A755AF3FB78B19376D13C0CEE011899C4C6E70`) para que ninguno de los dos quede sólo en disco.

```powershell
Get-ChildItem -Recurse -File docs/data | ForEach-Object { "{0}  {1}  {2}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash, $_.Length, $_.FullName }
```

---

*La pregunta era si dentro de los 41 pares hay dinero. La respuesta medida es que **no se puede ni llegar a preguntarlo**: 34 de los 41 no tienen dos patas con liquidez, 41 de 47 tokens están fuera de la allowlist, y el único par admisible tiene una sola pata. El cuarto techo queda cerrado como **medido y no importa** — no porque el número haya salido chico, sino porque las dos condiciones que lo harían posible no se dan **a la vez** en ningún par.*
