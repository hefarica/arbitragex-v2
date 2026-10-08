# PRODUCER-INFLATION-BLAST-01 — el radio de los dos defectos aguas arriba

**Resultado en una línea:** el **fantasma (a)** está **confinado a `hop_cycle_bridge`** (0 de 351.026 en `dex_engine`); el **precio (b)** tiene radio **medido**: **4,45 %** de las filas con gross>0 de `hop_cycle_bridge`, **0,59 %** de las de `dex_engine`, factor **193,03×–328,49×** (mediana **194,63×**) sobre **1.247** filas con precio real conocido, y **5 tokens**. El locus de (b) es **`size_optimizer.rs:3915`**, y la red de seguridad que su propio comentario invoca **es estructuralmente inalcanzable**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `1ca5d0a6-44e6-4b3f-813b-55d399718662`
**Alcance:** chain 1 · **solo lectura** sobre PG y código · **CERO cambios al motor** · **In scope:** `docs/data/`
**Base medida:** `origin/main = d1a4c3f5445917a5f5c361e79daf3351ef9f2983`. Todas las citas de línea de §1 están **releídas contra ese árbol** con `git show HEAD:<ruta>` en el clon aislado.

---

## 0. Control de instrumento, y DOS correcciones de método propias

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal VPS→Postgres** | `ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'"` | **`1`** (exit 0) | 08:0xZ |
| **rutas verificadas ANTES de usarlas** | `Test-Path` sobre las 4 rutas del verify | `EXISTE …/engines/dex_engine.rs` · **`NO EXISTE backend/searcher-rs/src/workers/hop_cycle_bridge.rs`** (vive en `route_discovery/`) | 07:41Z |
| **canal ROTO, declarado** | `ssh arbx "psql …"` | **`bash: line 1: psql: command not found`** exit **127** | 08:0xZ |

**El canal `psql` directo por SSH NO existe** (exit 127, `psql` no está en el PATH del host): el acceso real es **`docker exec arbitragex-v2-postgres-1`**. Un 127 leído como "0 filas" habría sido un rojo vestido de verde; se declara el instrumento roto en vez de reportar su silencio.

### Corrección de método 1 — el checkout compartido NO es `main`

Mi **primer borrador** citaba `size_optimizer.rs:2142-2194` y `dex_engine.rs:807-849` como locus. **Son de un árbol viejo.** El checkout compartido `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)` está en `858b943f` y su working tree es **sustancialmente más chico que `main`**:

| archivo | checkout compartido | **`main` (d1a4c3f5)** |
|---|---|---|
| `backend/searcher-rs/src/engines/dex_engine.rs` | 1 824 líneas | **3 040** |
| `backend/searcher-rs/src/size_optimizer.rs` | 4 649 líneas | **7 905** |

Citar `:2142` o `:807` contra `main` apunta a **otra función**. Las citas del borrador fueron **descartadas**; toda cita de §1 proviene de **releer `main`** en el clon aislado.

### Corrección de método 2 — un hallazgo que NO es mío, y lo casi reporto como nuevo

En el árbol viejo leí `dex_engine.rs:841` con una dirección de DAI (`0x6b175474e8f94a44ad05d02b745dcc163a999080`) distinta de la constante canónica, y lo redacté como **"tercer hallazgo"**. **En `main` eso ya está arreglado y documentado:**

```
1129:        // DAI-SYMBOL-ADDR-01 (audit 2026-09-29): this arm used to read
1130:        // "0x6b175474e8f94a44ad05d02b745dcc163a999080", which differs from the
1131:        // canonical DAI address in 27 of its 42 characters. Consequence: the REAL
1132:        // DAI address fell through to `None` (so DAI … was silently unpriced, the
1133:        // `no_price_oracle` family), while an address that is NOT DAI was labelled "DAI".
1138:        DAI_MAINNET_LC => Some("DAI"),
```

**`main` usa la constante canónica (`:1138`) y tiene test del typo (`:1880`).** El "tercer hallazgo" queda **retirado como hallazgo** y se reporta como **lo que es: un defecto ya cerrado por `DAI-SYMBOL-ADDR-01`**, visible sólo desde el árbol viejo. *(Si este PR hubiera salido sin releer `main`, habría reabierto un bug cerrado y contaminado la decisión de integración.)*

---

## 1. ★ DEFECTO (b) — EL LOCUS, releído contra `main`

**El conversor de `dex_engine` está GUARDADO por dirección** y **no puede** dispararse para UNI ni LINK:

```rust
// backend/searcher-rs/src/engines/dex_engine.rs:1095  (canonical_token_price_usd)
if let Some(sym) = canonical_token_symbol(&addr_str) {                              // :1104
    let sym_upper = sym.to_uppercase();
    if let Some(&p) = token_prices_usd.get(&sym_upper) { if p > 0.0 { return Some(p); } }   // :1109-1113
}
// WETH fallback: base_token_price_usd if configured (>0).
if addr_str == "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" && base_token_price_usd > 0.0 {  // :1116  ← GUARDADO
    return Some(base_token_price_usd);
}
None // R8: unpriced (no stable, no Redis price, no config)          // :1119
```
Y `canonical_token_symbol` (`:1123`) **sí mapea WETH (`:1125`), UNI (`:1140`) y LINK (`:1141`)**. Por este camino, UNI y LINK se pricearían bien. **El defecto está en otro archivo.**

### El locus real: `backend/searcher-rs/src/size_optimizer.rs:3866-3918`

```rust
// :3866
fn resolve_token_in_symbol(candidate: &StrategyCandidate, state: &TradingConfigState) -> Option<String> {
    let native_symbol = match candidate.opportunity.chain_id {      // :3872
        1 | 10 | 8453 | 42161 | 11155111 => "WETH",                 // :3873  ← chain 1 ⇒ "WETH"
        137 => "WMATIC", 56 => "WBNB", _ => "WETH",                 // :3874-3876
    };
    if let Some(leg) = candidate.route_plan.legs.first() {           // :3879
        let token_in_lower = leg.token_in.to_ascii_lowercase();      // :3880
        if candidate.opportunity.chain_id == 1 {                     // :3882   ← CINCO, y sólo cinco
            … WETH :3883-3885 · USDC :3886-3888 · USDT :3889-3891 ·
              DAI :3892-3894 · WBTC :3895-3897 …
        }
        let pair = &candidate.opportunity.pair_symbol;               // :3900   ← fallback por pair_symbol
        if pair.contains("WETH") || pair.contains("weth") { … }      // :3901-3903
        … USDC :3904 · USDT :3907 · DAI :3910 …
        // Last resort: the chain's native token (documented heuristic — the
        // caller's UnknownTokenPrice gate still catches unresolvable tokens).
        return Some(native_symbol.to_string());                      // :3915  ★★ ACÁ
    }
    Some(native_symbol.to_string())                                  // :3917  ★ y ACÁ
}
```

**CÓMO SELECCIONA EL PRECIO — la cadena completa:**

1. **`:3882-3897`** — la rama de **DIRECCIÓN** reconoce **exactamente CINCO** tokens mainnet: **WETH, USDC, USDT, DAI, WBTC**. **UNI (`0x1f9840a8…`), LINK (`0x51491077…`), PAXG, YELLOW y AXS NO están en esa lista.**
2. **`:3899-3912`** — el fallback que parsea el `pair_symbol` con `contains` **tampoco los reconoce**: el `pair_symbol` de estos ciclos es **`"514910(5-hop cycle)"`**, así que no contiene `"WETH"/"USDC"/"USDT"/"DAI"`.
3. **`:3913-3915`** — **ÚLTIMO RECURSO: `return Some(native_symbol.to_string())`**, y `native_symbol` para chain 1 es **`"WETH"`** (`:3873`).
4. **`:3923-3944`** — `resolve_token_price(state, "WETH")`: el mapa por token no lo resuelve para el token real, entra el símbolo `"WETH"` y devuelve **`state.base_token_price_usd`** (`:3934-3941`, arm `"WETH"`).

**⇒ LA LÍNEA EXACTA donde el precio de UNI/LINK/PAXG termina siendo el de WETH es `size_optimizer.rs:3915`, con `native_symbol = "WETH"` fijado en `:3873` y el valor cobrado en `:3936`.**

### ★★ El hallazgo filoso: la red de seguridad es ESTRUCTURALMENTE INALCANZABLE

El comentario del último recurso (`:3913-3914`) invoca un gate como backstop:

> `// caller's UnknownTokenPrice gate still catches unresolvable tokens.`

Ese gate **existe** (`:1014-1024`):

```rust
let token_price_usd = resolve_token_price(state, &token_in_symbol);   // :1013
let Some(token_price_usd) = token_price_usd else {                    // :1014
    … return Ok(OptimizeOutcome::Rejected(OptimizeRejectReason::UnknownTokenPrice, None));  // :1021-1024
};
```

**Y NO PUEDE DISPARARSE para la población que cae al último recurso.** La razón es de una línea: el símbolo ya fue colapsado a `"WETH"` en `:3873`/`:3915`, y `"WETH"` **siempre tiene precio** por el arm `:3934-3941` (`base_token_price_usd`, medido en `2 569,64`). La cadena es:

| paso | línea | efecto |
|---|---|---|
| 1 | `:3915` | token real (LINK) → `Some("WETH")` — **la función nunca devuelve `None`** |
| 2 | `:988` | `resolve_token_in_symbol(&candidate, state).unwrap_or_else(\|\| "WETH".to_string())` — **segunda coerción a "WETH"** en el sitio de llamada |
| 3 | `:1013` | `resolve_token_price(state, "WETH")` |
| 4 | `:3936` | `"WETH"` ⇒ `Some(base_token_price_usd)` — **nunca `None`** |
| 5 | `:1014` | el gate `UnknownTokenPrice` **no se activa** |

**⇒ El gate que el comentario nombra como red de seguridad es inalcanzable por construcción para exactamente los tokens que necesitan red.** No es que falle: **no llega a evaluarse en el caso que lo justifica.**

Y el comentario del propio resolvedor lo dice todavía más fuerte — contradiciéndose con el código:

> `:3863  /// chain), then honest None (the caller rejects with UnknownTokenPrice).`

**El código no tiene ninguna rama que devuelva `None`**: `:3915` y `:3917` son las dos únicas salidas y ambas son `Some(...)`. **El fix CORE-05 gateó la rama de dirección por `chain_id` (correcto) y documentó como hecho un `None` que nunca se escribió.**

**★ El propio archivo, 900 líneas más arriba, admite la verdad — y con eso se cierra la contradicción interna:** el comentario del sitio de llamada (`:983-986`) dice

> `// CORE-05: chain-gated address map + pair-symbol fallback + native fallback. In practice this always resolves (native last-resort); None is kept for the future TokenIdentityIndex wiring.`

O sea: **`:986` reconoce que "siempre resuelve" y que el `None` está ahí para un cableado futuro; `:3863` declara que ese `None` desemboca en un rechazo del caller. Los dos comentarios del mismo archivo describen comportamientos distintos del mismo código — y el que manda es el código: `:3915`.**

**Alcance del símbolo colapsado:** `token_in_symbol` alimenta **dos** consumidores, no uno — `:999` `state.effective_capital_for(&token_in_symbol, …)` (**el techo de capital**) y `:1013` `resolve_token_price` (**la conversión a USD**). El mismo colapso contamina los dos.

---

## 2. ★ BLAST RADIUS DE (b) — medido

Criterio: **`lower(token_in)` FUERA de los cinco** que reconoce `:3882-3897` ⇒ el resolvedor colapsa al nativo **WETH** y cobra `base_token_price_usd`.

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

**★ Control de consistencia interna — el conteo cierra exacto:** los tokens con precio real conocido en el radio son **LINK (1 180) + UNI (67) = 1 247**, y las filas con precio conocido medidas son **1 247**. **Coincide al entero.** La forma de la distribución lo confirma: ordenada ascendente, las 1 180 filas de LINK ocupan los rangos 1–1 180 y las 67 de UNI los 1 181–1 247 ⇒ la **mediana cae en el cúmulo de LINK (194,63×)** y el **máximo es UNI (328,49×)**. Con `WETH = 2 569,64`: 2 569,64/13,17 = **195,1** y 2 569,64/7,84 = **327,8** — mismo orden y magnitud que los dos cúmulos medidos.

**⇒ El radio NO es «UNI y LINK»: es cualquier token mainnet que no esté en la lista de cinco, y el más golpeado por conteo es PAXG (3 223 filas, 72 % del radio), no los dos que t137 había visto.**

---

## 3. ★ BLAST RADIUS DE (a) — el fantasma, CONFINADO

| detector | filas con ciclo | `retorno>10×` | `retorno>2×` | mediana del retorno |
|---|---|---|---|---|
| **`hop_cycle_bridge`** | **120 394** | **1 247** | **98 787** | **4,049×** |
| **`dex_engine`** | 351 026 | **0** | **0** | **0,991×** |

**El fantasma es EXCLUSIVO de `hop_cycle_bridge`.** En `dex_engine` el retorno del ciclo está **centrado en 0,991×** (por debajo de 1, como corresponde a un ciclo con fees) y **ninguna** de sus 351.026 filas supera 2×.

**Mecanismo, por medición:** el retorno sale de `leg_amounts_out[-1] / leg_amounts_in[0]` del `route_metadata`, y **la cadena de patas es internamente consistente** (cada pata recibe lo que entregó la anterior), así que no hay salto en el encadenado. El salto está **en la magnitud de una pata**: en la muestra de t137, `PAXG/LINK→USDT→USDC→PEPE→WETH→LINK` pasaba de **0,000 050 USDC a 9 914,66 PEPE** — **×800 en valor** en la pata **USDC→PEPE**. Un AMM sano no puede hacer eso. **⇒ El mecanismo es un escalado de magnitud en una pata V3 (cantidad o decimales), no en el encadenado.** La aritmética exacta de esa pata **no está medida acá** (§5.1).

---

## 4. ★ ¿AFECTAN LAS CIFRAS ECONÓMICAS YA REPORTADAS? — respuesta por cifra

| cifra | veredicto | evidencia |
|---|---|---|
| **`spread_negative_round_trip` 84,05 %** | **NO AFECTADA por (a).** Por (b): **acotada, no medida** | `spread_negative_round_trip` 24 h: **`dex_engine` n=3 148 192** (100 % de la familia). **(a) no puede tocarla: el fantasma tiene `retorno>10× = 0` en `dex_engine`; su radio es CERO ahí.** (b) alcanza al **0,59 %** de las filas de `dex_engine` con gross>0 — pero el conteo de *esta* familia con token fuera de los 5 **no está medido** (§5.4) |
| **`gross_profit_usd > 0` = 0 sobre 10 000** | **NO AFECTADA — la dirección la refuerza** | (b) **infla**: corrige hacia abajo. Un valor medido **0** queda **0 o más negativo**, nunca positivo. **Un defecto que infla no puede haber fabricado un 0** |
| **`max_net = 12,490223`** (t123) | **AFECTADA** | Filas del bucket `hop_cycle_bridge`: **ambos defectos aplican**. Con el precio real el gross cae a **13,143591 / 194,63 ≈ 0,0675 USD**; t137 midió el **gross real máximo = 0,040051**. El `net` correspondiente es **NEGATIVO** (t137: `0` de 1 247 sobreviven) |
| **`max_gross = 13,14359056`** (t123) | **AFECTADA — factor 194,63×–328,49×** | Misma fila: el gross registrado está inflado por el precio de WETH aplicado a LINK |

---

## 5. Límite del instrumento, y puntos NO COMPUTADOS

| # | NO COMPUTADO | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **La aritmética de la pata del ×800** (pool, reserves/sqrtPrice, decimales) | Medí **que** el salto está en la pata V3 USDC→PEPE y **que** el encadenado es consistente; **no** la aritmética de esa pata | `pool_addresses` y `leg_amounts_*` están en `route_metadata`: leer `getReserves()`/`slot0()` de esa pata al bloque del `quote_block` y recalcular |
| 2 | **Radio de (b) en filas SIN gross>0** | `token_in_symbol` alimenta también el **techo de capital** (`:999` `effective_capital_for`); puede afectar filas que no llegaron a tener gross>0 y acá medí sólo las que sí | Correr el mismo `FILTER` sin la condición `expected_profit_usd>0` — el total de la tabla ya está medido: **7 743 795 filas** |
| 3 | **Familia `spread_negative_round_trip` con token fuera de los 5** | Medí la condición sobre `expected_profit_usd>0`, no sobre las 3,15 M de esa familia | El mismo `FILTER` sobre `rejection_reason='spread_negative_round_trip'` |
| 4 | **PAXG, YELLOW y AXS a valor real** | No están en `token_prices_usd` (22 tokens al 07:27:47Z) ⇒ su factor **no es computable con el mapa actual**; su ausencia es la razón exacta de que el conteo con precio conocido sea 1 247 y no 4 471 | Precio real de esos tres en `token_prices_usd` o un oráculo equivalente |
| 5 | **Cuántas filas `dex_engine` (50) caen en el colapso por la vía `:3915` vs ya venían con otro símbolo** | Medí el conteo fuera de los 5, no la traza por fila del símbolo resuelto | El símbolo resuelto no está expuesto como columna; requeriría instrumentar o replicar la función sobre `route_plan` |

**El límite simétrico, declarado:** **(a) tiene radio CERO fuera de `hop_cycle_bridge`** — `0` de `351 026` filas en `dex_engine`, mediana **0,991×**. Ese cero **es un resultado** y cierra el miedo de que el fantasma contamine el camino principal; el comando que lo prueba es el `FILTER` de retorno por `detector_id` (§6), que devuelve 0 para `dex_engine` **y** 1 247 para `hop_cycle_bridge` en la misma pasada, de modo que el 0 no proviene de una consulta vacía. **(b) NO tiene radio cero**: **4,45 %** en su productor y **0,59 %** en `dex_engine`, con **5 tokens** identificados — acotado, no difuso.

**No se extrapola:** `dex_engine` **no** pasa por `resolve_token_in_symbol` en el camino que medí — su conversor es `canonical_token_price_usd` (`:1095`), con el fallback de WETH **guardado por dirección** (`:1116`). El 0,59 % de `dex_engine` se reporta como **población con el mismo síntoma medible** (token fuera de los cinco + factor de inflación), **no** como el mismo camino de código; atribuir el 0,59 % a `:3915` sería extrapolar, y §5.5 lo declara como no computado.

---

## 6. Permisos y reproducción

**Solo lectura**: `psql -tAc` (SELECT) y lectura de código con `git show HEAD:<ruta>` / `git grep`. **CERO cambios al motor, CERO escrituras en PG/Redis, CERO cambios de umbrales o configuración.**

```bash
# citas de §1 — releídas contra main (d1a4c3f5) en el clon aislado, NO contra el checkout compartido
git show HEAD:backend/searcher-rs/src/size_optimizer.rs | sed -n '3855,3944p'   # :3863 honest None · :3915 ultimo recurso
git show HEAD:backend/searcher-rs/src/size_optimizer.rs | sed -n '983,1025p'     # :988 coercion · :999 cap_usd · :1013-1024 gate
git show HEAD:backend/searcher-rs/src/engines/dex_engine.rs | sed -n '1095,1141p' # :1116 guardado · :1138 DAI canonico
```
```sql
-- radio de (b): filas cuyo token_in queda fuera de los cinco de :3882-3897
SELECT 'detector='||detector_id||' | con_gross>0='||count(*) FILTER (WHERE expected_profit_usd>0)
    ||' | fuera_de_los_5='||count(*) FILTER (WHERE expected_profit_usd>0 AND lower(token_in) NOT IN (…5 direcciones…))
FROM opportunities GROUP BY detector_id;
-- radio de (a): retorno del ciclo por productor — el MISMO query prueba el 0 de dex_engine y el 1247 de hop_cycle_bridge
WITH r AS (SELECT detector_id, (route_metadata->'leg_amounts_in'->>0)::numeric a_in,
                  (route_metadata->'leg_amounts_out'->>-1)::numeric a_out
           FROM opportunities WHERE route_metadata ? 'leg_amounts_in')
SELECT detector_id, count(*), count(*) FILTER (WHERE a_out/NULLIF(a_in,0)>10) FROM r GROUP BY 1;
```

---

## 7. Integridad

Verificación **por blob de git**, nunca con `Out-File` (re-codifica y mete CRLF):

```bash
git hash-object docs/data/PRODUCER-INFLATION-BLAST-01.md
git rev-parse HEAD:docs/data/PRODUCER-INFLATION-BLAST-01.md
```
`sha256` y hash de blob declarados en el cierre de t143.

---

*El fantasma está confinado: **0 de 351.026** en el camino principal, medido en la misma consulta que ve 1 247 en su productor. El precio no lo está, pero tiene radio medido y el conteo cierra contra el conjunto de tokens: **1 180 + 67 = 1 247**. El locus no era el fallback de WETH — guardado por dirección y por tanto incapaz de disparar para UNI ni LINK — sino el **último recurso del resolvedor de símbolos** (`:3915`), que devuelve `Some("WETH")` cuando su propio comentario (`:3863`) promete `None`, y cuya red de seguridad (`:1014`) es **inalcanzable por construcción** porque el símbolo ya colapsó una línea antes. La cifra que más importa — `gross_profit_usd > 0 = 0` — **no puede haber sido fabricada por un defecto que infla**. Y dos correcciones quedan declaradas: las citas del primer borrador venían de un checkout que **no es `main`**, y un "hallazgo" que iba a reportar como nuevo **ya estaba cerrado en `main`** por `DAI-SYMBOL-ADDR-01`.*
