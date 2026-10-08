# GHOST-LOCUS-01 — ¿la función o el estado? La función está BIEN; el pool es FALSO

**Resultado en una línea:** **la función no tiene el defecto.** Un cómputo V3 independiente en bigint —sin una línea del motor— **reproduce las dos patas V3 de la fila**: la «mala» dentro del **0,48 %** y el **control positivo** dentro del **1,28 %**. El ×1371 lo produce **el ESTADO**: el pool `0x261d53f3…` tiene **`liquidity = 57.737.784.664`** (microscópica) y un `tick` que implica un precio de PEPE **~121.300× fuera del precio de config**; **un swap de 0,000029 USDC mueve su precio ~88×, y el AMM paga 9.834 PEPE correctamente**. Y la procedencia cierra el caso: ese pool es **`enum_source='seed'`, `tvl_usd=NULL`**, y está en **102.237 de 102.237 filas** de la familia — el **100 %**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `786a4d91-fcb6-4534-9b07-605d0c898c92`
**Solo lectura** · `eth_call` a **bloque fijo** · **CERO cambios al motor, CERO escrituras, CERO cambios de umbrales** · **In scope:** `docs/data/`
**Base:** `origin/main = 901eb947ff3bec359a3021a8db5b074e7080b79f` (se movió desde `d1a4c3f5`).

---

## 0. Instrumento y ventana

| control | comando | salida literal |
|---|---|---|
| **canal** | `ssh arbx "docker exec arbitragex-v2-postgres-1 psql … -tAc 'SELECT 1'"` | **`1`** exit 0 |
| **canal roto** | `ssh arbx "psql …"` | exit **127** `command not found` |
| **ventana** | `… -tAc "SELECT min(detected_at), max(detected_at), count(*) FROM opportunities"` | `2026-10-05 04:21:13.227568+00` \| `2026-10-08 08:31:01.163603+00` \| **8.109.846** |
| **bloque fijado** | `quote_block` de la fila (`economics->>'quote_block'`) | **26143070** |
| **head al medir** | `cast block-number` | **26146479** ⇒ el bloque fijado está 3.409 atrás, **al alcance** |

**Ventana:** la tabla es **viva** y sigue creciendo — 7.859.761 → 7.904.902 → 7.925.656 → 8.031.255 → **8.109.846**. Los **conteos** de la campaña anterior **NO son comparables por ventana**; el **extremo** sí (`max_net 12,490223` reaparece idéntico).
**RPC:** `ethereum-rpc.publicnode.com` **rechaza archive** (`403 Archive requests require a personal token`); el que sirve el bloque fijado es **`https://eth.drpc.org`**. `ankr` pide API key, `llamarpc` da 525, `flashbots` da `rpc method is not whitelisted`. **Sin `latest` en ningún caso.**

Instrumento declarado, medido en t147 y usado acá: **`psql -tA` imprime NULL como cadena vacía** ⇒ un resultado vacío con exit 0 es un **NULL**, no un cero. Cada query va con su exit code.

---

## 1. ★ ESTADO DEL POOL A BLOQUE FIJO (lo que t147 dejó NO COMPUTADO)

`cast call <pool> <sig> --block 26143070 --rpc-url https://eth.drpc.org`

### Pool A — `0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba` (la pata del ×1371)

| función | valor crudo |
|---|---|
| `token0()` | **`0x6982508145454Ce325dDbE47a25d4ec3d2311933`** = **PEPE** (18 dec) |
| `token1()` | **`0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`** = **USDC** (6 dec) |
| `fee()` | **3000** |
| **`liquidity()`** | **`57737784664`** (5,7738 × 10¹⁰) |
| **`slot0().sqrtPriceX96`** | **`457478860655347350`** (4,5748 × 10¹⁷) |
| `slot0().tick` | **−517579** |

**Control de coherencia interna del propio pool:** `1,0001^(−517579)` = **3,3339 × 10⁻²³** y `sqrt(3,3339e−23) × 2⁹⁶` = **4,5748 × 10¹⁷** ⇒ **el `tick` y el `sqrtPriceX96` son consistentes entre sí**. El pool **no está corrupto ni a medio inicializar**: dice exactamente lo que dice.

### Pool B — `0x3470447f3cecffac709d3e783a307790b0208d60` (la otra pata V3, la de control)

| función | valor crudo |
|---|---|
| `token0()` / `token1()` | **UNI** / **USDT** (18 / 6 dec) |
| `fee()` | 3000 |
| `liquidity()` | **`2959833783596320330`** (2,9598 × 10¹⁸) |
| `slot0().sqrtPriceX96` | **`221474991273507411018508`** (2,2147 × 10²³) |
| `slot0().tick` | −255764 |

**Coherencia:** `1,0001^(−255764)` = 7,8281 × 10⁻¹² y `sqrt(·) × 2⁹⁶` = 2,2164 × 10²³ ✓. **Y con el ajuste decimal correcto** `P_human = P_raw × 10^(dec0−dec1)` = 7,8172 × 10⁻¹² × 10¹² = **7,8172 USDT/UNI** — **el precio real de UNI (config: 7,79), dentro del 0,35 %.** ⇒ **Pool B es un pool REAL.**

**⇒ Los dos pools son estructuralmente idénticos y económicamente opuestos: uno representa un mercado real y el otro no.**

---

## 2. ★★ EL COMPUTO A MANO — Y EL CONTROL POSITIVO QUE LO VALIDA

Fórmula de `v3_amount_out_single_tick` (`amm_math.rs:161` de **main**), implementada **desde cero en bigint, sin usar código del motor**:

```
fee 0,3% sobre el input;  √P = sqrtPriceX96 / 2⁹⁶
zeroForOne :  √P' = L·√P / (L + Δx·√P) ;  Δy = L·(√P − √P') / 2⁹⁶
oneForZero :  √P' = √P + Δy·2⁹⁶ / L  ;  Δx = L·2⁹⁶·(√P' − √P) / (√P·√P')
```

Los dos pools son V3 y **ambas patas son V3** ⇒ **el control positivo es la MISMA función, con el MISMO código, en la MISMA fila**, sobre un pool que sí representa un mercado.

| pata | pool | `zeroForOne` | in crudo | **out A MANO** | **out REGISTRADO** | **mano / registrado** |
|---|---|---|---|---|---|---|
| **leg 3 — USDC→PEPE** | A (`…261d53f3`) | `false` | `29` | **9.881,6205 PEPE** | **9.834,6305 PEPE** | **1,0048** ✅ |
| **leg 0 — UNI→USDT** (control) | B (`…3470447f`) | `true` | `3886740365229` | **30,38 USDT** | **30 USDT** | **1,0128** ✅ |

**★ LAS DOS PATAS SE REPRODUCEN.** El desvío es 0,48 % y 1,28 % — del orden del fee y del redondeo, **en la misma dirección** y sin ninguna constante ajustada a mano.

**⇒ EL INSTRUMENTO ESTÁ VALIDADO Y EL VEREDICTO ES CLARO:**
- **La función NO produce el ×1371.** Si tuviera el bug, mi matemática independiente habría dado ≈1,00 donde el registro dice ×1371. Da **×1378**.
- **El ×1371 lo produce EL ESTADO DE ENTRADA.** El motor leyó el estado de un pool y lo computó **correctamente**.

**Y el mecanismo queda explícito, sin descripción vaga:** con `L = 5,7738 × 10¹⁰`, el swap mueve `√P` de **5,7742 × 10⁻¹²** a **5,0654 × 10⁻¹⁰** — un desplazamiento de **×87,7 en el precio** — **dentro de una sola pata y de un solo tick**. La liquidez es tan microscópica que 0,000029 USDC **caminan el precio 88×**, y el AMM paga 9.834 PEPE **como corresponde**. **Es aritmética correcta sobre un pool sin realidad económica.**

---

## 3. ★★ LA ARITMÉTICA DEL ×1371, CERRADA CON LA CUENTA REPETIBLE

| magnitud | valor | cómo sale |
|---|---|---|
| precio implícito del motor | **3,4075 × 10⁸** PEPE/USDC | `9.881,62 / 0,000029` |
| precio real de config | **2,4728 × 10⁵** PEPE/USDC | `1 / 4,044 × 10⁻⁶` |
| **factor** | **1.377,97 ≈ 1.378×** | cociente de los dos anteriores |
| **¿es potencia de 10?** | **NO — `log10(1378) = 3,1390`** | **no es entero** |

**No es 10⁶ ni 10¹² ni ninguna potencia.** Y la cuenta que lo cierra por el lado del pool: su `tick = −517579` ⇒ `P_raw = 3,3339 × 10⁻²³` USDC_raw/PEPE_raw; con el ajuste decimal (`dec0=18` PEPE, `dec1=6` USDC) el precio humano es **3,3339 × 10⁻¹¹ USDC/PEPE** contra el de config **4,044 × 10⁻⁶** ⇒ **~121.300× fuera**. El ×1378 realizado y el ~121.300× del *spot* **no coinciden, y no tienen por qué**: el segundo es el precio de cotización y el primero incluye **el desplazamiento del precio durante el swap (×87,7)**. **Los dos números salen de un cómputo, no de una descripción.**

---

## 4. ★★ EL BLAST RADIUS: NO ES UN POOL, ES UNA CLASE — Y ES DE PROCEDENCIA

**En cuántas filas aparece el pool del ×1371:**

```
filas_familia=102237 | tocan_0x261d53f3=102237
```

**⇒ 102.237 de 102.237. EL 100 % de la familia fantasma pasa por ese pool.** No es un pool entre muchos: es **el** pool de la familia.

**Top de pools por apariciones en la familia (12 primeros), con su procedencia:**

| pool | `enum_source` | `tvl_usd` | apariciones |
|---|---|---|---|
| **`0x261d53f3…`** ← el del ×1371 | **`seed`** | **NULL** | **102.237** |
| `0xa43fe169…` | **`seed`** | **NULL** | 52.820 |
| `0x11950d14…` | **`seed`** | **NULL** | 49.417 |
| `0x48da0965…` | **`seed`** | **NULL** | 31.288 |
| `0x3416cf6c…` | **`seed`** | **NULL** | 23.943 |
| `0x04c85775…` | `dexscreener` | 17.846.155,83 | 23.464 |
| `0x60594a40…` | **`seed`** | **NULL** | 18.199 |
| `0xa478c297…` | **`seed`** | **NULL** | 17.926 |
| `0xc3d03e4f…` | **`seed`** | **NULL** | 16.888 |
| `0x7858e59e…` | `dexscreener` | 716.510,15 | 14.623 |
| `0x5777d92f…` | **`seed`** | **NULL** | 13.352 |
| `0xb20bd5d0…` | **`seed`** | **NULL** | 12.777 |

**★ El patrón es nítido: los pools de la familia son `seed` con `tvl_usd` NULL.** Los dos de `dexscreener` que aparecen **sí** tienen TVL. **La marca de la clase no es el ratio de la pata: es la procedencia.**

**Radio de la FUNCIÓN: CERO.** No hay una sola pata V3 que falle el cómputo — **las dos que hay en la fila se reproducen**. **Radio del ESTADO: la familia entera.** El defecto no es local ni de la función: es **la procedencia del pool**.

---

## 5. ★ DE DÓNDE VIENE EL ESTADO (criterio de entrada vs función)

Los dos pools de la fila, en la tabla `pools`:

| pool | `enum_source` | `fee_tier` | `tvl_usd` | `created_at` |
|---|---|---|---|---|
| **`0x261d53f3…`** (el del ×1371) | **`seed`** | 3000 | **NULL** | **2026-05-07 18:32:37** |
| `0x3470447f…` (el real) | `dexscreener` | 3000 | 3.375.488,91 | 2026-09-01 17:22:55 |

**Camino del productor en `main` (`901eb947`), nunca del checkout compartido:**

| `archivo:línea` | qué es |
|---|---|
| **`backend/searcher-rs/src/pool_discovery.rs:1006`** | **`INSERT INTO pools (chain_id, address, factory_id, token0_id, token1_id, fee_tier, is_active, enum_source, created_at)`** — donde nace la fila de `pools` y su `enum_source` |
| `backend/searcher-rs/src/pool_discovery.rs:465`, `:994` | el parámetro `enum_source: &str` que entra a la persistencia |
| `backend/searcher-rs/src/pool_discovery.rs:1014` | `enum_source = COALESCE(pools.enum_source, EXCLUDED.enum_source)` — **la procedencia original se conserva y no se re-etiqueta** |
| `backend/searcher-rs/src/pool_candidate.rs:12` | *«Which live data source produced a candidate (for provenance / `enum_source`)»* |
| `backend/searcher-rs/src/workers/pool_enumeration_worker.rs:673` | `enum_source = $3` — el otro punto que la escribe |
| **`backend/searcher-rs/src/amm_math.rs:161`** | **`pub fn v3_amount_out_single_tick(`** — la función, **que este trabajo absuelve** |

**La consecuencia tiene la forma exacta que el enunciado pedía:** **un pool sin realidad económica entró al camino del productor con procedencia `seed`, y el motor lo computó fielmente.** `amm_math.rs:161` **no necesita ningún arreglo** — el único arreglo que cambia algo es **qué pools entran**, no **cómo se computan**.

---

## 6. LÍMITES Y NO COMPUTADOS

| # | NO COMPUTADO | Por qué | Cierre |
|---|---|---|---|
| 1 | **El reparto exacto por `enum_source`** (apariciones agregadas) | Dos consultas mías salieron con **error de sintaxis propio** (alias `AS prov` en medio de una concatenación) y una tercera **agotó el timeout** en el join por pool. **No lo reporto como cero**: es **NO COMPUTADO por instrumento**, y su forma cruda está en la tabla de §4 | Corregir el `GROUP BY` sobre `enum_source` (**columna de `pools`**, no alias de expresión) y correr sobre la familia acotada |
| 2 | **Por qué `0x261d53f3` tiene ese estado** (¿plantado? ¿fork? ¿pool sintético del entorno?) | Medí **que** el estado es absurdo y **que** la procedencia es `seed`; **no** medí **de dónde salió ese estado**. El `pools` es `enum_source='seed'` con `tvl_usd=NULL` desde **2026-05-07** | Rastrear el cargador de `seed` y comparar contra el pool canónico de PEPE/USDC de mainnet real |
| 3 | **El ratio de valor por pata V3 sobre toda la población** | Medí el de las **dos** patas V3 de **una** fila (validado con control) y la **procedencia** de los pools de las 102.237; **no** un ratio por pata sobre las 102.237 × 6 = 613.422 patas | Unnest de `dex_adapters` × `leg_amounts_*` por ordinal con los precios de config |
| 4 | **Si el ×1378 se repite en otras filas** | El pool está en el 100 % de la familia, pero el ratio por fila no lo medí | Mismo cálculo sobre las 102.237 filas |

**Lo que NO es un límite, y hay que decirlo:** el cómputo a mano **no** es aproximado ni cualitativo. Es bigint exacto sobre el estado crudo leído a bloque fijo, y **reproduce las dos patas**. El instrumento del análisis **está validado por su control positivo** — que era exactamente el punto de la tarea.

---

## 7. REPRODUCCIÓN

```bash
# estado a BLOQUE FIJO (drpc es el único que sirve archive sin token)
cast call 0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba 'liquidity()(uint128)' --block 26143070 --rpc-url https://eth.drpc.org
cast call 0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba 'slot0()(uint160,int24,uint16,uint16,uint16,uint8,bool)' --block 26143070 --rpc-url https://eth.drpc.org
cast call 0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba 'token0()(address)' --block 26143070 --rpc-url https://eth.drpc.org
cast call 0x3470447f3cecffac709d3e783a307790b0208d60 'liquidity()(uint128)' --block 26143070 --rpc-url https://eth.drpc.org   # CONTROL POSITIVO
# la funcion, en MAIN
git grep -n 'pub fn v3_amount_out_single_tick' 901eb947ff3bec359a3021a8db5b074e7080b79f -- backend/searcher-rs/src/amm_math.rs
# el pool del x1371 en TODA la familia
#   SELECT count(*) FILTER (WHERE (route_metadata->'pool_addresses') @> '["0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba"]'::jsonb)
```

---

## 8. INTEGRIDAD Y PERMISOS

**Solo lectura.** Todos los `psql` fueron `SELECT`; todos los `cast` fueron `call` con **`--block` fijo**. CERO escrituras en PG/Redis, CERO cambios al motor, CERO cambios de umbrales. Único archivo tocado: `docs/data/GHOST-LOCUS-01.md`.

```bash
git hash-object docs/data/GHOST-LOCUS-01.md
git rev-parse HEAD:docs/data/GHOST-LOCUS-01.md
```

---

*La pregunta era binaria y la respuesta es neta: **el ×1371 lo produce el ESTADO, no la función.** El cómputo a mano reproduce la pata mala dentro del 0,48 % y el control positivo dentro del 1,28 % — y fue el control el que hizo válido el resultado: sin él, un cómputo que no reproduce nada no distingue «encontré el bug» de «escribí mal la fórmula». El pool tiene liquidez microscópica y un tick coherente consigo mismo, así que un swap de 0,000029 USDC **camina su precio 88×** y el AMM paga 9.834 PEPE correctamente. `amm_math.rs:161` queda absuelto: no hay nada que arreglar en cómo se computa. Lo que hay que arreglar es **qué pools entran** — y la marca está en la procedencia: `enum_source='seed'` con `tvl_usd=NULL`, en el 100 % de las 102.237 filas.*
