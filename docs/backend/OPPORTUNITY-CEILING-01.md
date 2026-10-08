# OPPORTUNITY-CEILING-01 — ¿De dónde sale el target de 50, y por qué el techo es +1,55?

**Tarea:** t173 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios, umbrales, escrituras o reinicios) · **Paper**
**Frontera pre/post (LA REAL):** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` -> **`2026-10-08T14:16:07.1195963Z`**, exit=0 — **NO** el `deploy.at` (`13:47:10Z`). **Toda cifra declara su corte en la misma línea.**
**Puerta:** `deploy.sha=77b42b3dccc001455fda3e8d4437d973c9e98c4b`, `id=37783694993`, `at=13:47:10Z`, exit=0.

---

## 1. VEREDICTO

**El sistema está cerrado por aritmética, en TRES capas independientes. La decisiva es la primera, y es de configuración, no un bug.**

```
target_net_usd   = 50.0        (trading_config, chain_id=1)
capital_usd      = 1000.00     (trading_config, chain_id=1)
mejor ROI JAMÁS detectado      = 0.560999 %     (en 9.323.311 oportunidades)
=> máximo geométrico = 1000 × 0.560999% = 5.6100 USD BRUTOS
=> BRECHA vs 50 = 8.91x
```

**Para netear $50 con $1000 de capital hace falta un ROI del 5,0000 %.** El mejor ROI que el detector ha producido en **9,3 millones** de oportunidades es **0,560999 %**. **⇒ El target exige 8,91 veces más ROI del que el mercado le ha ofrecido nunca.**

**Y las 122 rentables no son una excepción: son exactamente las que el gate aritmético de la capa 2 rechaza.** Las tres capas, y cada una por separado alcanza para cerrar el sistema.

---

## 2. ★★ EL TARGET: DE DÓNDE SALE, CON PRODUCTOR

### 2.1 El productor NO es un literal en Rust

**Corrección de un hallazgo propio, declarada.** Mi primer candidato fue `backend/prioritization-spine/src/config_aware.rs:1268` -> `min_profit_usd: 50.0, // Ethereum mainnet floor (migration 046: chain_id=1 → $50)`. **Esa línea está DENTRO de un `#[cfg(test)]`** — es el fixture `fn cfg() -> TradingConfigState` de un test, **no el productor**. Se retira como hallazgo. (Blob del archivo, para que quede constancia de qué se leyó: `06429758f04f5a9544f9b078f6a5c585172d002f`.)

### 2.2 El productor REAL: una fila de `trading_config` — operador, por cadena

`docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT … FROM trading_config ORDER BY chain_id"`, exit=0:

| chain_id | `capital_usd` | `min_profit_usd` | `simulation_target_profit_usd` | `enabled` | `updated_by` |
|---|---|---|---|---|---|
| **1** | **1000.00** | **50.0000** | **50.0000** | **true** | **`admin`** |
| 10 | 0.00 | 5.0000 | NULL | false | `migration_046` |
| 56 | 0.00 | 5.0000 | NULL | false | `migration_046` |
| 137 | 0.00 | 2.0000 | NULL | false | `migration_046` |
| 8453 | 0.00 | 5.0000 | NULL | false | `migration_046` |
| 42161 | 0.00 | 10.0000 | NULL | false | `migration_046` |

**⇒ El target sale de una fila de configuración, con `updated_by='admin'`: es un valor DEL OPERADOR, por cadena, y NO un literal hardcodeado sin productor.** Las otras 5 cadenas las sembró `migration_046` y **están `enabled=false`**. **La única cadena activa es la 1.**

**El lector, con su locus:** `backend/shared-rs/src/pre_execute_checklist.rs:333` -> `SELECT min_profit_usd::float8 FROM trading_config WHERE chain_id = $1 AND enabled = TRUE`. Y `backend/shared-rs/src/trading_config.rs:622` -> `effective_min_profit_usd(strategy_kind)`: override por estrategia si existe, si no el de la cadena.

**Blob citado:** `backend/shared-rs/src/trading_config.rs` = **`950122a2087a770c8345a4c69763f593c91520ed`**.

### 2.3 ¿Fue alguna vez alcanzable? **NO — y el número lo cierra**

Con `capital_usd = 1000`, alcanzar `net = 50` exige **ROI ≥ 5,0000 %**. El máximo observado en 9.323.311 oportunidades es **0,560999 %**. **Nunca estuvo cerca: 8,91×.**

**Capital alternativo:** `50 / (0.560999/100) = ` **`$8 912,67`**. **⇒ O el capital sube ~8,9×, o el target baja ~8,9×, o el detector encuentra rutas con 9× más spread. Las tres son decisiones, ninguna es un bug.**

---

## 3. ★★ LAS 122 RENTABLES: EL GATE MÁS CARO, RESUELTO

**Discriminante, declarado antes de contar:** una oportunidad con `net > 0` que no simula puede estarlo (a) porque el rechazo **es correcto** — no era viable neta de costes — o (b) porque **un gate de más** la mata. Se resuelve partiendo la población por su `rejection_reason` y midiendo si el número cierra contra el criterio del gate.

### 3.1 Los dos conteos

`psql -tAc`, corte: **toda la tabla** (el rechazo no tiene corte temporal propio; se declara):

```
oportunidades con economics->>'net_profit_usd' > 0 ........ 122
rejection_reason = 'gas_floor_breach:own_capital' ......... 122
  ... y de ésas, con net_profit_usd > 0 .................. 122
```

**⇒ LOS TRES CONTEOS SON EL MISMO 122. No es coincidencia: es identidad de conjuntos.**

```
SELECT coalesce(rejection_reason,'<NULL>'), count(*) FROM opportunities
  WHERE (economics->>'net_profit_usd')::numeric > 0 GROUP BY 1
  ->  gas_floor_breach:own_capital | 122        (una sola fila, exit=0)
```

### 3.2 El gate que las mata, con su locus exacto

`backend/searcher-rs/src/size_optimizer.rs:1223-1224` (blob `a1411d8200d3baad5c912f2dcba036cc63e032b7`):

```rust
// Gas Floor (operator directive #3): require net ≥ multiplier × cost.
if net_usd < cost_proxy_usd * state.kelly_gas_safety_multiplier {
```

Documentado en el mismo archivo, `:334` y `:1182`:
> `Net profit below the gas-safety floor: net_usd < gas_usd × kelly_gas_safety_multiplier.`
> `Gas floor: net_usd ≥ cost_usd × kelly_gas_safety_multiplier`

### 3.3 **El valor del multiplicador: `3.0`, y está CONFIRMADO por dos vías**

**Vía A — el código, con locus.** `backend/shared-rs/src/trading_config.rs:444-446` (blob `950122a2087a770c8345a4c69763f593c91520ed`), verificado byte a byte contra `origin/main`:

```rust
/// Require net ≥ 3× gas as the viability floor. Institutional standard
/// accounts for gas spikes between forecast and inclusion.
fn default_kelly_gas_safety_multiplier() -> f64 {
    3.0
}
```
Declarado en `:396-397` como `#[serde(default = "default_kelly_gas_safety_multiplier")] pub kelly_gas_safety_multiplier: f64`. **Y `trading_config` NO tiene columna `kelly%`** (`ILIKE '%kelly%'` -> 0 filas) **y `strategy_configs` para chain 1 es `{}`** ⇒ **el default de serde `3.0` está en efecto, sin override.**

**Vía B — los datos, sin mirar el código.** El ratio `net/cost` de las 122:

```
min 0.1040   max 2.2864   (n=122)
```
**Con `m = 1.0` habrían pasado las de ratio > 1 — y pasaron CERO.** ⇒ `m > 2.2864`, por medición. **`2.2864 < 3.0` ⇒ el gate `net < cost × 3.0` rechaza EXACTAMENTE a las 122. Las dos vías concuerdan.**

*(Los `1.0` que aparecen en `size_optimizer.rs:4047` y otros 7 sitios son fixtures: el propio código los rotula `// permissive in legacy tests`.)*

### 3.4 **¿Es correcto el rechazo? SÍ — y los números lo dicen**

| Medición de las 122 | Valor |
|---|---|
| `net_profit_usd` | 0,0717 … **1,5459** |
| `total_cost_usd` | 0,6491 … 0,6891 |
| **umbral exigido `3,0 × cost`** | **1,9473 … 2,0673** |
| `net/cost` | 0,1040 … 2,2864 |
| `target_delta_usd` | −48,45 … −49,93 |

**El gate exige `net ≥ 3× cost`, o sea `net ≥ ~$1,95`. El mejor neto de toda la población es `$1,5459`. ⇒ El rechazo es CORRECTO según el criterio declarado: ninguna de las 122 alcanza el suelo de viabilidad.** **No es un gate de más.**

**⇒ VEREDICTO DEL GATE: veredicto de mercado, correcto, con números.** Es la misma dicotomía de t123, resuelta partiendo la población: aquí **el rechazo es correcto**.

---

## 4. LAS TRES CAPAS — CADA UNA CIERRA EL SISTEMA

| Capa | Lo que exige | Lo que hay | Brecha |
|---|---|---|---|
| **1 — Config (DECISIVA)** | `net ≥ 50` con `capital = 1000` ⇒ **ROI ≥ 5,0000 %** | mejor ROI **0,560999 %** | **8,91×** |
| **2 — Gas floor (`:1224`)** | `net ≥ 3× cost` ≈ **$1,95** | mejor net **$1,5459** | **1,26×** |
| **3 — `meets_target`** | `net ≥ 50` | máximo geométrico **$5,61** | **8,91×** |

**⇒ Las capas 1 y 3 son la MISMA pared vista dos veces.** La capa 2 es una pared **distinta y más baja**. **Quitar la capa 2 no abre nada**: las 122 seguirían sin pasar `meets_target` — su `target_delta` está entre **−48,45 y −49,93**.

**⇒ La capa que hay que mover es la 1, y no es código: es la fila `trading_config` de chain 1.**

---

## 5. EL TAMAÑO: ¿OPTIMIZA, O ESTÁ PEGADO AL CAP?

Del `SizeOptimizer` de las 122 rentables, por buckets de `amount_in_usd`:

| bucket | filas | rango |
|---|---|---|
| 1 | 1 | 79,68 |
| 2 | 2 | 148,68 – 148,70 |
| **10** | **83** | **999,99 – 1000,00** |
| **11** | **36** | **1000,00** |

**⇒ 119 de 122 (97,5 %) están en o pegadas al cap de $1000, y 36 están en el cap EXACTO.** Sólo 3 filas son menores.

**Lectura:** esto **no es la firma de un óptimo encontrado, es la firma de un tamaño empujado hasta un techo duro**. El óptimo interior sería un valor disperso; un cap produce concentración exacta en el borde.

**Y entre las que SÍ llegan a simular** (`simulated_at > '2026-10-08T14:16:07Z'`, exit=0): **879 filas, `amount_in_usd` de 0,01 a 2534,66, con 55 valores distintos de `amount_in_wei`.** ⇒ **El optimizador SÍ varía el tamaño** — el pin al cap es específico de la población rentable, no del optimizador.

**⇒ El cuello NO es el `SizeOptimizer`: es el `capital_usd = 1000` que le pone el techo, y el spread bruto del detector que hace que el óptimo esté en el borde.** `SizeOptimizer` vive en `backend/searcher-rs/src/size_optimizer.rs` (blob `a1411d8200d3baad5c912f2dcba036cc63e032b7`); su rama de Kelly con el gas floor está en `:1206-1240`.

---

## 6. EL DETECTOR: DISTRIBUCIÓN COMPLETA

`psql -tAc "SELECT coalesce(rejection_reason,'<NULL>'), count(*) FROM opportunities GROUP BY 1 ORDER BY 2 DESC NULLS LAST LIMIT 25"`, exit=0, lectura `14:43:35Z`:

| `rejection_reason` | count |
|---|---|
| **`spread_negative_round_trip`** | **4 892 047** |
| `v3_pool_not_catalogued` | 1 031 922 |
| **`non_positive_profit`** | **970 907** |
| `spread_zero_equilibrium` | 787 040 |
| `v3_quote_unavailable` | 715 824 |
| `single_pool_no_spread` | 334 403 |
| `v3_pair_no_pools` | 299 121 |
| `no_tradable_size` | 105 045 |
| `StrategyDisabled:triangular_arb` | 84 398 |
| `v3_multileg_budget_exhausted` | 33 955 |
| `v3_pool_revert` | 15 485 |
| **`negative_net_profit`** | 8 222 |
| `TokenNotAllowed:0xeef9f339…` | 5 242 |
| `TokenNotAllowed:0x236eb848…` | 4 920 |
| `TokenNotAllowed:0x45804880…` | 4 345 |
| `TokenNotAllowed:0x1abaea1f…` | 4 126 |
| `TokenNotAllowed:0xf8173a39…` | 1 185 |
| `no_price_oracle` | 707 |
| **`gas_floor_breach:own_capital`** | **122** |
| `anomalous_math` | 115 |
| `reserves_cache_miss` | 25 |
| `high_gas_volatility` | 25 |
| `missing_reserves_pool_b` | 25 |
| `TokenNotAllowed:0xbbbbca6a…` | 11 |
| `TokenNotAllowed:0x4575f413…` | 8 |

**`status`: `rejected` en 9 300 841** (de 9 323 311 oportunidades a `14:49:04Z`, exit=0).

**`spread_negative_round_trip` = 4 892 047 = 52,5 % del total** ⇒ más de la mitad de todo lo detectado muere porque **el spread es negativo al cerrar el round-trip**. El detector propone ciclos que, cerrados, **dan menos de lo que costaron**.

**Y las que SÍ pasan el spread: máximo `net = +1,5459`, máximo `gross = +2,2221`, `meets_target = TRUE` en 0 de 9 308 426.** El techo de la mejor ruta es **+$1,55**, contra un objetivo de **$50**.

**⚠️ Calidad de dato, declarada:** entre las ROI hay valores absurdos — `min ROI = −196 972 209 456 200 400 %` y `net` de hasta `−999 949 711 198,83` en 63 filas. **Son filas patológicas**; no se usan para nada de este informe, pero **existen y se declaran**.

---

## 7. LO QUE YA ESTABA CERRADO Y NO SE RE-DERIVÓ

- **El veredicto de mercado NO es cero**: vive en `opportunities.rejection_reason` (millones), **no** en `simulations.fail_reason`, donde t167 lo buscó. **Columnas distintas, tablas distintas: error de categoría.** Se verificó que sigue ahí esta corrida y **no se volvió a buscar en `fail_reason`**.
- **El fallo de transferencia es por RUTA, no por par** (t168): `route_metadata.token_addresses = [WETH, 0x3f382dbd…, WETH]` = **round-trip cíclico**; **el `token_in == token_out` universal NO es un defecto, es la firma de un round-trip**.
- **ALLOWANCE confirmado** (t168), y — coherente con todo esto — **el arreglo vale hoy $0**: ninguna ruta que llegue a la transferencia es rentable.

---

## 8. NO COMPUTADO, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| **El "óptimo" que declara el `SizeOptimizer`** | **NO COMPUTADO.** Se midió el **resultado** (tamaños pegados al cap, §5), no el valor interno de su óptimo. El optimizer no persiste el óptimo que calcula; sólo el tamaño resultante. |
| **Tasa de fallo de fondeo de la población del fork** | La identidad heredada (`cache_hit + seeded_fresh = anvil`) **murió con Δ no constante (4, 3, 4)** en t168. Sin población delimitada no hay tasa. Conteos crudos en §9. |
| Por qué el detector no encuentra spreads > 0,561 % | **NO COMPUTADO aquí.** Requiere auditar el detector, que es otra capa; esta tarea mide el techo, no su causa. |
| `kelly_gas_safety_multiplier` **en la BD** | **NO EXISTE columna** (`ILIKE '%kelly%'` -> 0 filas) y `strategy_configs = {}`. El valor **efectivo** es el default de serde `3.0` (§3.3). Se declara porque «no está en la tabla» no es «no está en efecto». |
| Capacidad | **NO COMPUTADA.** Ventanas propias, comparadas sólo contra sí mismas. |

**Regla aplicada:** un «no medido» **no se escribe como 0**; una etiqueta renombrada **no** se lee como un cero; y **un cero necesita su control positivo**.

---

## 9. CONTROLES

| Control | Resultado |
|---|---|
| Canal SQL `SELECT 1` | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`** | `SELECT esto_no_existe` -> **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` -> **509** |
| **Conteo numerado** | todo con `count(*)`; `ILIKE '%kelly%'` -> 0 filas con el control de que la tabla tiene 32 columnas |
| **`NULLS LAST` explícito** | en todas las distribuciones: `ORDER BY 2 DESC NULLS LAST` — los NULL **no** se leen como cero |
| Externo `:9090` (defecto #9) | **`http_code=000`, exit=7** — reproducido por sexta vez |
| `raw_trace` (defecto #8) | `710|0|918` post-restart ⇒ **0 sobre 918 positivas**: **SEXTA** tarea consecutiva. Campo real: `revert_risk_pct` = **710** |

**Ninguna evidencia usa `195.201.235.70:9090` como fuente ni `/metrics`.** Prometheus por **loopback vía ssh**, exit=0: `verify_mismatch=6`, `cache_hit=685`, `seeded_fresh=6` @ts `1791471073.885`.

---

## 10. DOS AJUSTES PROPIOS, CAZADOS Y DECLARADOS

1. **`config_aware.rs:1268` NO es el productor del target** — está dentro de un `#[cfg(test)]`. Lo presenté como «smoking gun» y **me corrijo**: el productor es la fila de `trading_config` (§2.2). El blob del archivo queda citado para que el error sea auditable.
2. **`opportunities` NO tiene columnas planas `gross_profit_usd`/`net_profit_usd`** — viven en el JSONB `economics`. Mi primer `SELECT max(gross_profit_usd)` dio **`ERROR: column "gross_profit_usd" does not exist`, exit=1**, y el `max` del techo salió vacío. Corregido con `(economics->>'net_profit_usd')::numeric`; **el `exit=1` va reportado, no ocultado.**

---

## 11. QUÉ SE PUEDE HACER CON ESTO (y qué no)

**El sistema no gana nada porque su propia configuración lo cierra.** Tres movimientos posibles, **los tres son decisiones del operador, ninguno es un fix de código**:

1. **Subir `capital_usd` de chain 1 de `1000` a ≈ `8913`** (+791 %), manteniendo el target en 50. *Ojo: hay que verificar que el capital real lo permita y que el tamaño no mueva el precio — no se computó el impacto de mercado a ese tamaño.*
2. **Bajar el target a ≈ `5,6`** (el máximo geométrico), manteniendo $1000.
3. **Que el detector encuentre rutas con ≥ 5 % de spread** — nunca lo ha hecho en 9,3 M de intentos; su mejor es **0,561 %**.

**Y una nota que puede valer más que las tres:** mover la capa 2 (el gas floor de 3×) **no cambia nada**, porque su umbral (~$1,95) ya está **por debajo** del máximo geométrico de la capa 1 (~$5,61) — pero muy por encima del techo real (+$1,55). **Ese es el único de los tres números que NO es de configuración: es del mercado.**

---

## 12. TRAZABILIDAD

- Leído: `GET /api/status`, `docker inspect` (`StartedAt`), **PostgreSQL** por `docker exec … psql` (SELECT-only), Prometheus **loopback vía ssh**, y **el código de `origin/main`** en el clon aislado (`git rev-parse`, `git show`).
- **Blobs citados** (todos en `origin/main` = `77b42b3dccc001455fda3e8d4437d973c9e98c4b`): `shared-rs/src/trading_config.rs` = `950122a2087a770c8345a4c69763f593c91520ed` · `searcher-rs/src/size_optimizer.rs` = `a1411d8200d3baad5c912f2dcba036cc63e032b7` · `prioritization-spine/src/config_aware.rs` = `06429758f04f5a9544f9b078f6a5c585172d002f` · `searcher-rs/src/economics.rs` = `173e9227d308867bd561fba9c44a06e7d395a85c`.
- **CERO** cambios a motor, umbrales, escrituras o reinicios. **CERO** mainnet, firmas o broadcast. **No se tocó el fork.** Paper.
- **NO** se re-disparó el benchmark.
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t173 · attempt `177f48b9-4ce0-4984-aecb-708c55a5b125`
