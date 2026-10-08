# PRODUCER-INFLATION-BLAST-01 — el radio de los dos defectos aguas arriba

**Resultado en una línea:** el **fantasma (a)** está **confinado a `hop_cycle_bridge`** (0 de 351.026 en `dex_engine`); el **precio (b)** tiene radio **medido**: **4,45 %** de las filas con gross>0 de `hop_cycle_bridge`, **0,59 %** de las de `dex_engine`, factor **193,03×–328,49×** (mediana **194,63×**) sobre **1.247** filas con precio real conocido, y **5 tokens**. El locus de (b) es **`size_optimizer.rs:2191`** — y **contradice su propio doc-comment**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `1ca5d0a6-44e6-4b3f-813b-55d399718662`
**Alcance:** chain 1 · **solo lectura** sobre PG y código · **CERO cambios al motor** · **In scope:** `docs/data/`

---

## 0. Control de instrumento

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal VPS→Postgres** | `psql -U postgres -d arbitragex -tAc "SELECT 1"` | **`1`** — antes de cada cifra | 07:44:51Z · 07:47:46Z |
| **rutas verificadas ANTES de usarlas** | `Test-Path` sobre las 4 rutas del verify | `EXISTE backend/searcher-rs/src/engines/dex_engine.rs` · **`NO EXISTE backend/searcher-rs/src/workers/hop_cycle_bridge.rs`** · `EXISTE backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs` · `EXISTE backend/searcher-rs/src/workers/` | 07:41Z |
| **citas de código releídas contra el archivo** | `read backend/searcher-rs/src/size_optimizer.rs` 2120-2219 y `dex_engine.rs` 800-849 | ver §1 — **las citas de línea de mi propio borrador estaban corridas y fueron descartadas** | 07:52Z |

**Dos errores de método propios, declarados porque sus 0 eran falsos:**

1. **Condición equivocada.** Mi primera condición de radio asumió que el `pair_symbol` contiene `"WETH"`. Medido, es falso: el `pair_symbol` es `"514910(5-hop cycle)"` (prefijo hex + conteo de patas). La primera medición dio **0 filas** y ese 0 **era de mi hipótesis, no del fenómeno**. Re-hecha, el radio aparece.
2. **Citas de línea corridas.** Mi borrador citaba `size_optimizer.rs:3866-3918` como el locus. **Al releer el archivo, 3857-3948 es el módulo de tests** (`flashloan_wrapped_subtracts_fee`, `make_sized`). **El locus real es `:2142-2194`.** Las citas de línea del borrador fueron **descartadas y reemplazadas por las releídas**; ninguna cita de §1 proviene del borrador.

Las tres citas de §1 las re-leí con `read` sobre el archivo en este checkout, en las líneas que se indican.

---

## 1. ★ DEFECTO (b) — EL LOCUS, con `archivo:línea` releído

**El conversor que asignaría el precio del token de entrada en `dex_engine` está GUARDADO por dirección** y **no puede** dispararse para UNI ni LINK:

```rust
// backend/searcher-rs/src/engines/dex_engine.rs:807-832  (canonical_token_price_usd)
let addr = token?;                                       // :812
let addr_str = format!("0x{:040x}", addr);                // :813
if let Some(sym) = canonical_token_symbol(&addr_str) {     // :819
    let sym_upper = sym.to_uppercase();
    if let Some(&p) = token_prices_usd.get(&sym_upper) { if p > 0.0 { return Some(p); } }
}
// WETH fallback: base_token_price_usd if configured (>0).
if addr_str == "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" && base_token_price_usd > 0.0 {  // :828  ← GUARDADO
    return Some(base_token_price_usd);
}
None // R8: unpriced (no stable, no Redis price, no config)          // :831
```
Y `canonical_token_symbol` **sí mapea UNI (`:843`) y LINK (`:844`)**. Por este camino, UNI y LINK se pricearían bien. **El defecto está en otro archivo.**

### El locus real: `backend/searcher-rs/src/size_optimizer.rs:2142-2194`

```rust
// :2142
fn resolve_token_in_symbol(candidate: &StrategyCandidate, state: &TradingConfigState) -> Option<String> {
    let native_symbol = match candidate.opportunity.chain_id {          // :2148
        1 | 10 | 8453 | 42161 | 11155111 => "WETH",                     // :2149  ← chain 1 ⇒ "WETH"
        137 => "WMATIC", 56 => "WBNB", _ => "WETH",                     // :2150-2152
    };
    if let Some(leg) = candidate.route_plan.legs.first() {               // :2155
        let token_in_lower = leg.token_in.to_ascii_lowercase();          // :2156
        if candidate.opportunity.chain_id == 1 {                         // :2158   ← CINCO, y sólo cinco
            if token_in_lower.contains("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") { return Some("WETH".into()); }  // :2159-2161
            if token_in_lower.contains("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") { return Some("USDC".into()); }  // :2162-2164
            if token_in_lower.contains(&USDT_MAINNET_LC[2..])                     { return Some("USDT".into()); }  // :2165-2167
            if token_in_lower.contains("6b175474e89094c44da98b954eedeac495271d0f") { return Some("DAI".into()); }   // :2168-2170
            if token_in_lower.contains("2260fac5e5542a773aa44fbcfedf7c193bc2c599") { return Some("WBTC".into()); }  // :2171-2173
        }
        let pair = &candidate.opportunity.pair_symbol;                   // :2176   ← fallback por pair_symbol
        if pair.contains("WETH") || pair.contains("weth") { return Some("WETH".into()); }   // :2177-2179
        if pair.contains("USDC") { return Some("USDC".into()); }         // :2180-2182
        if pair.contains("USDT") { ... } if pair.contains("DAI") { ... } // :2183-2188
        // Last resort: the chain's native token (documented heuristic — the
        // caller's UnknownTokenPrice gate still catches unresolvable tokens).
        return Some(native_symbol.to_string());                          // :2191  ★★ ACÁ
    }
    Some(native_symbol.to_string())                                      // :2193  ★ y ACÁ
}
```

**CÓMO SELECCIONA EL PRECIO — la cadena completa:**

1. **`:2158-2174`** — la rama de **DIRECCIÓN** reconoce **exactamente CINCO** tokens mainnet: WETH, USDC, USDT, DAI, WBTC. **UNI (`0x1f9840a8…`), LINK (`0x51491077…`), PAXG, YELLOW, AXS NO están en esa lista.**
2. **`:2176-2188`** — el fallback que parsea el `pair_symbol` con `contains` **tampoco los reconoce**: el `pair_symbol` de estos ciclos es **`"514910(5-hop cycle)"`**, así que no contiene `"WETH"/"USDC"/"USDT"/"DAI"`.
3. **`:2189-2191`** — **ÚLTIMO RECURSO: `return Some(native_symbol.to_string())`**, y `native_symbol` para chain 1 es **`"WETH"`** (`:2149`).
4. **`:2199-2219`** — `resolve_token_price(state, "WETH")`: el mapa por token no lo resuelve para el token real, entra el símbolo `"WETH"` y devuelve **`state.base_token_price_usd`** (`:2211-2213`).

**⇒ LA LÍNEA EXACTA donde el precio de UNI/LINK/PAXG termina siendo el de WETH es `size_optimizer.rs:2191`, con `native_symbol = "WETH"` fijado en `:2149` y el valor cobrado en `:2211-2213`.**

### ★ El hallazgo que el borrador no tenía: el código CONTRADICE su propio doc-comment

El doc-comment del propio resolvedor, releído en `:2131-2141`, declara:

> «CORE-05 fix (2026-09-24): the previous version hardcoded 5 MAINNET addresses + a "WETH" fallback — on Base/Arbitrum/Polygon a non-mainnet token resolved to "WETH" ($3000) when its real price was $0.01, **inflating cap_wei by ~300000×**. Now: (a) the address→symbol map is GATED by the candidate's chain_id (mainnet addresses only resolve on chain 1); (b) the fallback chain prefers the pair_symbol, then native (WETH/WMATIC/WBNB per chain), **then honest None (the caller rejects with UnknownTokenPrice)**,»

**(a) y (b) están implementadas. La parte de "then honest None" NO existe en el código.** `:2191` y `:2193` devuelven `Some(native_symbol.to_string())` **incondicionalmente**: la función **no tiene ninguna rama que devuelva `None`** salvo… ninguna. Las dos salidas de la función son `Some(...)`.

**⇒ El fix CORE-05 gateó la rama de dirección por `chain_id` (correcto), pero el último recurso quedó intacto, y el comentario documenta como hecho un `None` que nunca se escribió.** La consecuencia es la misma que el incidente que el comentario dice haber cerrado —inflación de `cap_wei`—, por una causa distinta: **no un token no-mainnet, sino un token mainnet fuera de la lista de cinco**. El `UnknownTokenPrice` que el comentario invoca como red de seguridad **no puede dispararse por esta vía**, porque la función nunca le entrega un `None`.

---

## 2. ★ BLAST RADIUS DE (b) — medido

Criterio: **`lower(token_in)` FUERA de los cinco** que reconoce `:2158-2174` ⇒ el resolvedor colapsa al nativo **WETH** y cobra `base_token_price_usd`.

| detector | filas con `expected_profit_usd>0` | de ésas, **fuera de los 5** | **%** |
|---|---|---|---|
| `dex_engine` | 8 430 | **50** | **0,59 %** |
| **`hop_cycle_bridge`** | **103 113** | **4 592** | **4,45 %** |

**Distribución del factor de inflación** (24 h, fuera de los 5, con precio real conocido en `token_prices_usd`):

| n | con precio conocido | factor mínimo | **mediana** | factor máximo |
|---|---|---|---|---|
| 4 471 | **1 247** | **193,03×** | **194,63×** | **328,49×** |

**Tokens distintos que caen en el radio (24 h, 5 en total):**

| token | dirección | filas | ¿precio real en el mapa? |
|---|---|---|---|
| **PAXG** | `0x45804880de22913dafe09f4980848ece6ecbaf78` | **3 223** | **no** |
| **LINK** | `0x514910771af9ca656af840dff83e8264ecf986ca` | **1 180** | sí — 13,17 |
| YELLOW | `0x236eb848c95b231299b4aa9f56c73d6893462720` | 78 | **no** |
| **UNI** | `0x1f9840a85d5af5bf1d1762f925bdaddc4201f984` | **67** | sí — 7,84 |
| AXS | `0xbb0e17ef65f82ab018d8edd776e8dd940327b28b` | 1 | **no** |

**★ Control de consistencia interna — el conteo cierra exacto:** los tokens con precio real conocido en el radio son **LINK (1 180) + UNI (67) = 1 247**, y las filas con precio conocido medidas son **1 247**. **Coincide al entero.** Y la forma de la distribución lo confirma: ordenada ascendente, las 1 180 filas de LINK ocupan los rangos 1–1 180 y las 67 de UNI los 1 181–1 247 ⇒ la **mediana cae en el cúmulo de LINK (194,63×)** y el **máximo es UNI (328,49×)**. Con `WETH = 2 569,64`: 2 569,64/13,17 = **195,1** y 2 569,64/7,84 = **327,8** — el mismo orden y magnitud que los dos cúmulos medidos.

**⇒ El radio NO es «UNI y LINK»: es cualquier token mainnet que no esté en la lista de cinco, y el más golpeado por conteo es PAXG (3 223 filas, 72 % del radio), no los dos que t137 había visto.**

---

## 3. ★ BLAST RADIUS DE (a) — el fantasma, CONFINADO

| detector | filas con ciclo | `retorno>10×` | `retorno>2×` | mediana del retorno |
|---|---|---|---|---|
| **`hop_cycle_bridge`** | **120 394** | **1 247** | **98 787** | **4,049×** |
| **`dex_engine`** | 351 026 | **0** | **0** | **0,991×** |

**El fantasma es EXCLUSIVO de `hop_cycle_bridge`.** En `dex_engine` el retorno del ciclo está **centrado en 0,991×** (por debajo de 1, como corresponde a un ciclo con fees) y **ninguna** de sus 351.026 filas supera 2×.

**Mecanismo, por medición:** el retorno sale de `leg_amounts_out[-1] / leg_amounts_in[0]` del `route_metadata`, y **la cadena de patas es internamente consistente** (cada pata recibe lo que entregó la anterior), así que no hay salto en el encadenado. El salto está **en la magnitud de una pata**: en la muestra de t137, `PAXG/LINK→USDT→USDC→PEPE→WETH→LINK` pasaba de **0,000 050 USDC a 9 914,66 PEPE** — **×800 en valor** en la pata **USDC→PEPE**. Un AMM sano no puede hacer eso. **⇒ El mecanismo es un escalado de magnitud en una pata V3 (cantidad o decimales), no en el encadenado.** La aritmética exacta de esa pata **no está medida acá** (§5).

---

## 4. ★ TERCER HALLAZGO — dirección de DAI corrupta en `dex_engine`, del mismo lote

Al releer `canonical_token_symbol` apareció un literal **que no coincide con la constante canónica que el propio archivo importa**:

| fuente | DAI |
|---|---|
| **canónica** — `backend/shared-rs/src/chains.rs:98` (`pub const DAI_MAINNET_LC`) | `0x6b175474e89094c44da98b954eedeac495271d0f` |
| **`backend/searcher-rs/src/engines/dex_engine.rs:841`** (`match` de `canonical_token_symbol`) | `0x6b175474e8f94a44ad05d02b745dcc163a999080` |
| `backend/searcher-rs/src/size_optimizer.rs:2168` (mismo token, otro archivo) | `…94c44da98b954eedeac495271d0f` — **coincide con la canónica** |

`dex_engine.rs:55` **ya importa** `DAI_MAINNET_LC` (`use shared_rs::chains::{DAI_MAINNET_LC, …}`), así que la constante correcta estaba disponible en el archivo y el `match` escribió un literal divergente.

**Consecuencia, en la dirección OPUESTA a (b):** `canonical_token_symbol` devuelve `None` para DAI ⇒ `canonical_token_price_usd` cae a `:831 None` ⇒ **rechazo por precio desconocido de un token que sí está pricado** (`token_prices_usd` tenía 22 tokens al 07:27:47Z). Es un **falso rechazo**, no una inflación: no infla el gross, **borra oportunidades DAI**. Los otros cuatro de la lista (`:837` WETH, `:839` USDC, `:840` USDT, `:842` WBTC) **sí coinciden** con las constantes canónicas — el divergente es uno solo.

**Alcance declarado:** verifiqué la discrepancia **por comparación de literales contra la constante canónica**; **no** medí cuántas filas DAI se perdieron por esta vía (ver §5).

---

## 5. Límites del instrumento, y puntos NO COMPUTADOS

| # | NO COMPUTADO | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **La aritmética de la pata del ×800** (pool, reserves/sqrtPrice, decimales) | Medí **que** el salto está en la pata V3 USDC→PEPE y **que** el encadenado es consistente; **no** la aritmética de esa pata | `pool_addresses` y `leg_amounts_*` están en `route_metadata`: leer `getReserves()`/`slot0()` de esa pata al bloque del `quote_block` y recalcular |
| 2 | **Filas DAI perdidas por el literal de `dex_engine.rs:841`** | Verifiqué la divergencia de literales; **no** conté oportunidades DAI rechazadas por esta causa | `FILTER` sobre `rejection_reason` de tokens DAI en `dex_engine`, o corregir el literal y re-medir |
| 3 | **Radio de (b) en filas SIN gross>0** | `resolve_token_in_symbol` alimenta el **sizing** (`cap_wei`); puede afectar filas que no llegaron a tener gross>0 y acá medí sólo las que sí | Correr el mismo `FILTER` sin la condición `expected_profit_usd>0` — el total de la tabla ya está medido: **7 743 795 filas** |
| 4 | **Familia `spread_negative_round_trip` con token fuera de los 5** | Medí la condición sobre `expected_profit_usd>0`, no sobre las 3,15 M de esa familia | El mismo `FILTER` sobre `rejection_reason='spread_negative_round_trip'` |
| 5 | **PAXG, YELLOW y AXS a valor real** | No están en `token_prices_usd` (22 tokens al 07:27:47Z) ⇒ su factor **no es computable con el mapa actual**; su ausencia es la razón exacta de que el conteo con precio conocido sea 1 247 y no 4 471 | Precio real de esos tres en `token_prices_usd` o un oráculo equivalente |

**El límite simétrico, declarado:** **(a) tiene radio CERO fuera de `hop_cycle_bridge`** — `0` de `351 026` filas en `dex_engine`, mediana **0,991×**. Ese cero es un resultado y cierra el miedo de que el fantasma contamine el camino principal. **(b) NO tiene radio cero**: **4,45 %** en su productor y **0,59 %** en `dex_engine`, con **5 tokens** identificados — acotado, no difuso.

---

## 6. ¿AFECTAN LAS CIFRAS ECONÓMICAS YA REPORTADAS? — respuesta por cifra

| cifra | veredicto | evidencia |
|---|---|---|
| **`spread_negative_round_trip` 84,05 %** | **NO AFECTADA por (a).** Por (b): **acotada** | `spread_negative_round_trip` en 24 h: **`dex_engine` n=3 148 192** (100 % de la familia). **(a) no puede tocarla: el fantasma tiene `retorno>10× = 0` en `dex_engine`.** (b) alcanza al **0,59 %** de las filas de `dex_engine` con gross>0 — y el conteo de *esta* familia con token fuera de los 5 **no está medido** (§5.4) |
| **`gross_profit_usd > 0` = 0 sobre 10 000** | **NO AFECTADA — la dirección la refuerza** | (b) **infla**: corrige hacia abajo. Un valor medido **0** queda **0 o más negativo**, nunca positivo. **Un defecto que infla no puede haber fabricado un 0** |
| **`max_net = 12,490223`** (t123) | **AFECTADA** | Filas del bucket `hop_cycle_bridge`: **ambos defectos aplican**. Con el precio real el gross cae a **13,143591 / 194,63 ≈ 0,0675 USD**; t137 midió el **gross real máximo = 0,040051**. El `net` correspondiente es **NEGATIVO** (t137: `0` de 1 247 sobreviven) |
| **`max_gross = 13,14359056`** (t123) | **AFECTADA — factor 194,63×–328,49×** | Misma fila: el gross registrado está inflado por el precio de WETH aplicado a LINK |

---

## 7. Permisos y reproducción

**Solo lectura**: `psql -tAc` con SELECT y lectura de código con `read`/`git grep`. **CERO cambios al motor, CERO escrituras en PG/Redis, CERO cambios de umbrales o configuración.** No se re-ejecutó nada que escriba.

```bash
# el locus, releído en este checkout (las citas de §1 salen de estas dos lecturas)
sed -n '2131,2194p' backend/searcher-rs/src/size_optimizer.rs   # resolve_token_in_symbol: :2191 = último recurso
sed -n '2196,2219p' backend/searcher-rs/src/size_optimizer.rs   # resolve_token_price:  :2211-2213 = base_token_price_usd
sed -n '807,849p'   backend/searcher-rs/src/engines/dex_engine.rs  # :828 guardado; :841 literal DAI divergente
sed -n '95,98p'     backend/shared-rs/src/chains.rs             # :98 = DAI_MAINNET_LC canónico
```
```sql
-- radio de (b): filas cuyo token_in queda fuera de los cinco de :2158-2174
SELECT 'detector='||detector_id||' | con_gross>0='||count(*) FILTER (WHERE expected_profit_usd>0)
    ||' | fuera_de_los_5='||count(*) FILTER (WHERE expected_profit_usd>0 AND lower(token_in) NOT IN (…5 direcciones…))
FROM opportunities GROUP BY detector_id;
-- radio de (a): retorno del ciclo por productor
WITH r AS (SELECT detector_id, (route_metadata->'leg_amounts_in'->>0)::numeric a_in,
                  (route_metadata->'leg_amounts_out'->>-1)::numeric a_out
           FROM opportunities WHERE route_metadata ? 'leg_amounts_in')
SELECT detector_id, count(*), count(*) FILTER (WHERE a_out/NULLIF(a_in,0)>10) FROM r GROUP BY 1;
```

---

## 8. Integridad

`sha256` declarado **en el cierre de t143**. Verificación **por blob de git**, nunca con `Out-File` (re-codifica y mete CRLF):

```bash
git hash-object docs/data/PRODUCER-INFLATION-BLAST-01.md
git rev-parse HEAD:docs/data/PRODUCER-INFLATION-BLAST-01.md
```

---

*El fantasma está confinado: **0 de 351.026** en el camino principal. El precio no lo está, pero tiene radio medido y el conteo cierra contra el conjunto de tokens: **1 180 + 67 = 1 247**. El locus no era el fallback de WETH — guardado por dirección y por tanto incapaz de disparar para UNI ni LINK — sino el **último recurso del resolvedor de símbolos**, que devuelve `Some("WETH")` cuando su propio comentario promete `None`. Y en el mismo lote de literales hay un **DAI con la dirección mal copiada**, que borra en vez de inflar. La cifra que más importa — `gross_profit_usd > 0 = 0` — **no puede haber sido fabricada por un defecto que infla**.*
