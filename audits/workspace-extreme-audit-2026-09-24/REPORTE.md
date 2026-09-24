# AUDITORÍA EXTREMA DE WORKSPACE — ArbitrageX v2
**Fecha:** 2026-09-24 · **Método:** estática + aritmética + fronteras (sin ejecutar código del proyecto, sin compilar, sin RPC/DB/Redis)
**HEAD auditado:** `96aec70e92081459f8fa89178b92e37cdb34f320` · **Ref del operador:** `2245eeb325ab7a3d0e40f09cfeca357369975d1a` (existe localmente; diff A=2/D=22/M=60)

---

## 1. INVENTARIO (sección 2a)

### 1.1 Conteo total
| Métrica | Valor |
|---|---|
| Archivos tracked (git ls-files) | **7 863** |
| Archivos untracked no-ignorados | 109 (audits/ 72, tools/ 24, docs/ 4, raíz 9) |
| **Universo auditado (tracked+untracked)** | **7 972** |
| Hasheados SHA-256 | 7 965 (+4 con nombres UTF-8 que el pase inicial no resolvió por encoding = 7 969) |
| Gitlinks (submódulos no materializados) | 3: `contracts/lib/forge-std`, `openzeppelin-contracts`, `openzeppelin-contracts-upgradeable` |
| **Esperado por el operador** | 7 873 ±5 → **delta real: −10 vs esperado, −20 vs ref** |

**Reconciliación del delta:** el ref `2245eeb` tiene 7 883 archivos; HEAD tiene 7 863 (22 borrados, 2 añadidos desde el ref). El "7873" del prompt no coincide con ninguno de los dos (off-by-10 respecto a HEAD, fuera de la banda ±5). BorradOS desde el ref (los 22, relevantes): `backend/searcher-rs/src/beta_priors.rs`, `price_bus_global.rs`, `workers/binance_ws.rs`, `backend/shared-rs/src/price_bus.rs`, `migrations/122/123_*.sql`, 5 tests de frontend store, `scripts/cleanup-branches.sh`. Añadidos: `backend/searcher-rs/src/workers/binance_stream_worker.rs`, `docs/env/binance-ws-feed.md`.

### 1.2 Conteo por extensión (tracked)
.md 3 926 · .json 1 225 · .tsx 627 · .ts 516 · .rs 427 · .rhai 271 · .sql 115 · .yml 85 · .sh 83 · .py 81 · .txt 72 · .png 63 · .sol 53 · sin-ext 34 · .html 29 · .mmd 29 · .csv 26 · .xlsx 24 · .toml 23 · .log 14 · .js 14 · .patch 13 · resto ≤9 cada una.
**Nota:** NO EXISTE ningún archivo `.hex` en el árbol del proyecto (la pregunta 2f sobre Dockerfile COPY de fixtures .hex queda respondida: no aplican).

### 1.3 Cartuchos .rhai
- `cartridges/strategies/` = **264 .rhai + 264 .json** (sidecars).
- IDs `mev_NN_NNN`: **264 únicos, 0 duplicados, 0 gaps** por categoría: 01=36, 02=17, 03=31, 04=31, 05=14, 06=30, 07=30, 08=25, 09=20, 10=18, 11=12 (Σ=264; cat 11 termina en 012 ✓).
- 7 raíz: backrun, dex_arb, funding_rate_arbitrage, liquidation, mean_reversion_arbitrage, omega_strategy_pack, triangular_arb (+2 JSON sidecars) = **271 .rhai totales** ✓.
- **Hash de árbol verificado:** `git rev-parse HEAD:backend/searcher-rs/cartridges/strategies` = `f0ca2fcb8d5ff54af983a17e0593569be096ea15` — **coincide byte-a-byte con el hash de referencia del operador** (también coincide con el árbol dentro del ZIP adjunto AUDITORIA_RHAI_ESTRUCTURA_Y_ECONOMIA.zip).

### 1.4 Artefactos de la auditoría
- `manifest.sha256` (7 965 entradas hash+size+ruta) — `audits/workspace-extreme-audit-2026-09-24/`
- `git-blob-index.txt` (7 863 blob SHA del índice)
- Hallazgo de inventario: 18 artefactos de fuzz-cache de Foundry commiteados bajo `arbitragex-v2-main/` (ruta de ZIP extraído commiteada por error) — ver BAJO INV-01.

---

## 2. HALLAZGOS

> RHAI-01..15 = los 14+1 hallazgos previos del operador (AUDITORIA_ES.md), **re-verificados uno a uno contra los bytes**. RHAI-16+ y CORE-xx = nuevos de esta auditoría.

### [ALTO] RHAI-01 — dex_arb rama V2: resta paralela Q_alt(x)−Q_source(x) en vez de round-trip encadenado
**Archivo:** `backend/searcher-rs/cartridges/dex_arb.rhai:371-391`
**Predicado:** el código afirma que `|Q_alt(x) − Q_source(x)| > 0` implica arbitraje; falso: ambas quotes son salidas A→B con el MISMO input x y no se encadenan.
**Evidencia:**
```rhai
let estimated_out_source = calculate_amount_out_v2(amount_in_str, source_reserves.r0, source_reserves.r1);
let estimated_out_alt    = calculate_amount_out_v2(amount_in_str, reserves.r0, reserves.r1);
if alt_price > source_price { profit_raw = estimated_out_alt - estimated_out_source; }
else                        { profit_raw = estimated_out_source - estimated_out_alt; }
```
**Contraejemplo (re-derivado independientemente + verificado contra el JSON del operador):** pools CPMM fee 30 bps, reservas P1=(1 000 000 A, 1 000 000 B), P2=(1 000 000 A, 1 001 000 B), entrada 100 A:
- Q_P1(100)=99.690060 B; Q_P2(100)=99.789750 B → diferencia paralela **+0.099690 B** (positiva).
- Round-trip real P1→P2: 99.690060 B → 99.281840 A → **−0.718160 A** (pérdida).
- Round-trip real P2→P1: 99.789750 B → 99.480483 A → **−0.519517 A** (pérdida).
Diferencia paralela positiva con ambos ciclos perdedores: la rama V2 marca "oportunidad" donde el ciclo completo pierde.
**Impacto:** falsos positivos económicos en la rama V2 del cartucho maestro (mitigado aguas abajo por SizeOptimizer/gates, pero el `estimated_profit` emitido es un número sin validez).
**Reparación:** cotizar ida y vuelta encadenando el output del primer hop como input del retorno; conservar el resultado con signo.
**Estado: CONFIRMADO** (igual que el reporte previo; la rama V3 `v3_roundtrip_for_alt` L626-677 sí encadena correctamente — el comentario L170 del propio archivo reconoce el bug de la rama V2).

### [ALTO] RHAI-02 — dex_arb V2: gate de gas compara token_out humano contra gas en moneda nativa
**Archivo:** `backend/searcher-rs/cartridges/dex_arb.rhai:404-409`
**Predicado:** `gas_cost_token` (ETH/MATIC humano) se compara/resta contra `best_profit` (token_out humano) — unidades incompatibles salvo token_out=nativo.
**Evidencia:**
```rhai
let gas_cost_token = estimated_gas * base_fee / 1000000000.0; // gwei to token
let min_profit_threshold = gas_cost_token * 1.2;
if best_profit <= min_profit_threshold { ... below_gas_threshold }
net_profit: best_profit - gas_cost_token
```
**Contraejemplo:** token_out=USDC (6 dec), base_fee=30 gwei, gas≈405 000 → threshold = 405000×30/1e9×1.2 = 0.01458. Cualquier profit >0.015 USDC "cubre gas" — el gate real debería exigir ≈$40 (30 gwei×405k×$3000/1e9×1.2). Para token_out=PEPE el gate se vuelve inalcanzable. El propio archivo corrige esto en la rama V3 (L202-232: `native_gas_symbol` + `native_usd`) pero la rama V2 quedó sin el fix.
**Impacto:** falsos positivos (gate de gas inoperante) o falsos negativos según el token.
**Reparación:** valorar gas en USD con el precio del token nativo (como la rama V3) y comparar contra profit valorado en USD.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-03 — dex_arb V2: `block_number` declarada DENTRO del for y usada DESPUÉS del loop (scope)
**Archivo:** `backend/searcher-rs/cartridges/dex_arb.rhai:350` (declaración) vs `429` y `469` (usos)
**Predicado:** Rhai limita las variables al bloque; `block_number` se declara en el cuerpo del `for pool_addr in alt_pools` (L350) y se usa en `calculate_confidence(..., block_number)` (L429) y en el mapa final `block_number: block_number` (L469) fuera de ese bloque.
**Evidencia:**
```rhai
for pool_addr in alt_pools {
    let block_number = get_block_number();   // L350 — scope del loop
    ...
}
...
let confidence = calculate_confidence(..., block_number);  // L429 — FUERA de scope
```
**Contraejemplo:** cualquier evaluación V2 que pase `insufficient_pools` y alcance L429 lanza `ErrorVariableNotFound` en runtime → `CartridgeError::RuntimeError` → `cartridge.shadow_eval_error`/`active_eval_error`. El camino positivo V2 **nunca retorna un mapa**.
**Impacto:** la rama positiva V2 del cartucho maestro está estructuralmente muerta (además de RHAI-01/02 y CORE-02 que la mueren antes).
**Reparación:** declarar `block_number` una vez en el ámbito de la función antes del loop.
**Estado: CONFIRMADO** (estático; el runner usa `Scope::new()` vacío, sin globals).

### [ALTO] RHAI-04 — dex_arb V3: emite is_opportunity=true etiquetado single_tick_upper_bound (cota, no confirmación)
**Archivo:** `backend/searcher-rs/cartridges/dex_arb.rhai:264-280`
**Predicado:** una cota superior single-tick no es un quote ejecutable; marcar `is_opportunity:true` con `v3_sizing_method:"single_tick_upper_bound"` + `confidence:0.3` promete más de lo verificado.
**Evidencia:**
```rhai
if best_net_usd > 0.0 {
    return #{ is_opportunity: true, estimated_profit: best_net_usd, ...,
        confidence: 0.3, single_tick_unconfirmed: true,
        v3_sizing_method: "single_tick_upper_bound", reason: "v3_arb_viable", ... };
```
**Impacto:** candidatos V3 cuya pierna V3 puede estar sobreestimada (single-tick sobreestima por diseño — ver amm_math.rs:117-127); confían en QuoterV2/simulación aguas abajo, pero la etiqueta no viaja como gate.
**Reparación:** propagar `quote_method` hasta la card y exigir confirmación QuoterV2/REVM antes de autorizar ejecución.
**Estado: CONFIRMADO** (gate `ARBX_V3_ARB_MODE` default OFF limita exposición).

### [MEDIO] RHAI-05 — encode_arb_calldata produce texto, no ABI; build_payload no se invoca en el camino activo
**Archivo:** `backend/searcher-rs/cartridges/dex_arb.rhai:799-811`; consumidor ausente en `cartridge_boot.rs`
**Predicado:** `"0x" + "execute_arb(" + ...` no es calldata ABI; además ningún caller activo invoca `runner.build_payload` (verificado: cartridge_boot nunca lo llama).
**Impacto:** superficie contractual muerta; riesgo de confusión si alguien cablea build_payload tal cual.
**Reparación:** delegar el ensamblado al encoder Rust (swap_encoder/execute_arbitrage_encoder) con los campos del plan validado.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-06 — omega_strategy_pack: costes ausentes → 0.0; liquidez/frescura → 0.50 via get_num
**Archivo:** `backend/searcher-rs/cartridges/omega_strategy_pack.rhai:66-98, 148-154`
**Predicado:** `get_num(d,"gas_cost_usd",0.0)` trata "campo ausente" igual que "coste cero"; `liquidity_score/freshness_score` ausentes puntúan 0.50.
**Evidencia:**
```rhai
let net = cost_adjusted_profit(gross, get_num(d, "gas_cost_usd", 0.0),
    get_num(d, "dex_fees_usd", 0.0), get_num(d, "slippage_cost_usd", 0.0), get_num(d, "risk_penalty_usd", 0.0));
let liq = get_num(pool_data, "liquidity_score", 0.50);
```
**Contraejemplo:** spread $2 con TODOS los costes ausentes → net=$2 > min $0.50 → `accept()` con confianza 0.35+0.125+0.10+0 = 0.575; el mismo spread con gas real $8 sería rechazado. Ausencia ≈ coste cero.
**Impacto:** aceptaciones del pack con costes no computados (viola R8 "None ≠ 0").
**Reparación:** campo aplicable ausente ⇒ rechazo por completitud con reason; 0 solo si fue computado.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-07 — omega_strategy_pack: reject() borra el net ya calculado
**Archivo:** `backend/searcher-rs/cartridges/omega_strategy_pack.rhai:41-49` + callers (p.ej. 155-157)
**Predicado:** `reject(reason)` siempre emite `estimated_profit:0.0` aunque el caller ya computó `net` (p.ej. `spatial_net_profit_below_threshold` descarta el net que explica el rechazo).
**Impacto:** telemetría/dataset de calibration pierde la magnitud de los rechazados.
**Reparación:** reject tipado que conserve net/costes calculados.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-08 — triangular_arb: profit en unidades mínimas (wei) comparado/restado contra gas en ETH
**Archivo:** `backend/searcher-rs/cartridges/triangular_arb.rhai:170-213` (find_best_triangle) y `95-109` (gate)
**Predicado:** `profit = out_a − amount_in` en wei; `gas_cost = 450000 × base_fee / 1e9` en ETH; el gate `profit <= gas_cost*1.3` y `net = profit − gas_cost` cruzan escalas 1e18.
**Evidencia:**
```rhai
let out_a = calculate_amount_out(out_c.to_string(), res_ca.r0, res_ca.r1);
let profit = out_a - amount_in;              // WEI
...
let gas_cost = estimate_triangle_gas(chain_id) * base_fee / 1000000000.0;  // ETH
if best_result.profit <= gas_cost * 1.3 { ... }   // wei vs ETH
```
**Contraejemplo:** dec=18, profit real 0.5 token = 5e17 wei; gas@30gwei = 0.0135. Gate: 5e17 > 0.01755 → pasa SIEMPRE que profit>~0.02 wei → cualquier micro-ganancia de 1 wei supera "13.5 mETH de gas". Para dec=6 (USDC): profit 50 (=$0.50) > 0.01755 → también pasa. El gate de gas es inoperante en la práctica y `net = 5e17 − 0.0135` no descuenta nada. Los parámetros `dec_a/dec_c` se reciben y NO se usan.
**Impacto:** falsos positivos sistemáticos del triángulo raíz; net sin gas.
**Reparación:** normalizar profit a token humano (usar los decimales ya recibidos), valorar gas en USD nativo (como dex_arb V3), comparar en USD.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-09 — liquidation.rhai: build_payload convierte max_liquidatable con 18 decimales fijos
**Archivo:** `backend/searcher-rs/cartridges/liquidation.rhai:146`
**Predicado:** `to_wei(opportunity.max_liquidatable, 18)` ignora `debt_meta.decimals` usado en la evaluación.
**Evidencia:**
```rhai
flash_loan: #{ protocol: "auto", token: opportunity.debt_token,
              amount: to_wei(opportunity.max_liquidatable, 18) },
```
**Contraejemplo:** debt_token USDC (6 dec), max_liquidatable=1000 → to_wei produce 1000×10^18 en vez de 1000×10^6 → flash loan **10^12× mayor** → revert o error de fondos si algún executor consumiera el payload.
**Impacto:** payload no ejecutable/peligroso (hoy build_payload no se invoca en el camino activo — exposición latente).
**Reparación:** usar los decimales verificados del debt_token (ya disponibles en debt_meta).
**Estado: CONFIRMADO.**

### [ALTO] RHAI-10 — runner.rs parse_eval_result: unwrap_or(0.0) funde ausente/cero/moneda
**Archivo:** `backend/searcher-rs/src/cartridge/runner.rs:583-597` (parse_eval_result)
**Predicado:** `estimated_profit` y `confidence` se leen con `as_float().unwrap_or(0.0)`; un f64 plano sin moneda ni bruto/neto.
**Evidencia:**
```rust
let estimated_profit = map.get("estimated_profit").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
let confidence = map.get("confidence").and_then(|v| v.as_float().ok()).unwrap_or(0.0).clamp(0.0, 1.0);
```
**Nota de verificación:** se sweep-eó el tipado de los 264 generados: TODAS las formas `profit_q/bu_q`, `profit_tok/base_unit` son f64 (math_pow→float), y ningún literal INT en estimated_profit → el silenciamiento por tipo NO ocurre hoy en los generados (los 7 raíz tampoco emiten INT). El riesgo queda como contrato frágil (un cartucho futuro que emita INT o string se silencia a 0.0 sin reason).
**Impacto:** contrato productor↔consumidor sin moneda/tipo; degradación silenciosa posible.
**Reparación:** parser discriminado (valor, unidad, bruto/neto, moneda, causa de ausencia) sin unwrap_or genérico; conservar metadata (hoy sí se conserva).
**Estado: CONFIRMADO (con matiz de tipado verificado).**

### [ALTO] RHAI-11 — get_token_price_usd recibe DIRECCIÓN 0x en los 264 generados; el hash Redis está indexado por SÍMBOLO canónico
**Archivo:** productor: `backend/searcher-rs/src/workers/price_worker.rs:1194-1201` + doc L282 ("Canonical uppercase symbol used as the Redis hash field key"); binding: `host_bindings.rs:426-441` (HGET field literal); consumidor: `mev_01_001_dex_dex_arbitrage.rhai:138` (y los 264: `get_token_price_usd(token_in0)` con token_in0 = dirección).
**Predicado:** si el campo del hash es símbolo UPPERCASE y el caller pasa "0x…", el HGET nunca matchea.
**Evidencia:**
```rhai
let px = get_token_price_usd(token_in0);        // token_in0 = leg.token_in = "0x…"
if px != () && px > 0.0 { profit_usd = (profit_tok / base_unit) * px; }
```
```rust
pipe.hset(&key, sym, format!("{}", price)).ignore();   // sym = símbolo canónico
```
**Contraejemplo:** WETH a $3 000 en `arbx:token_prices:1` bajo campo "WETH". `HGET arbx:token_prices:1 0xc02aaa39…` → nil → `px == ()` → `profit_usd = 0.0` → el resultado NO lleva `profit_usd_hint` → cartridge_boot (L1193-1197) fija `expected_profit_usd = None` → el gate `has_computed_economics` del emitter (opportunity_emitter.rs:272-288) reclasifica **toda aceptación de los 264 como rechazo `no_computable_economics`**.
**Impacto:** **la flota completa de 264 cartuchos no puede emitir NINGUNA fila aceptada** en el camino ACTIVE (frontera productor/consumidor rota end-to-end). Secundario: dex_arb raíz sí pasa `meta_in.symbol` (correcto), pero si el símbolo on-chain no es UPPERCASE el HGET también missa (case-sensitive).
**Reparación:** resolver identidad chain+address→símbolo canónico compartido con price_worker (contrato de clave único writer↔reader).
**Estado: CONFIRMADO y ESCALADO** (el reporte previo lo marcaba "falta verificar incidencia": aquí queda verificada la consecuencia estructural en el emitter).

### [ALTO] RHAI-12 — cartridge_boot reconstruye RoutePlan/Candidate desde intent.legs, ignorando la ruta/elegidos del Rhai
**Archivo:** `backend/searcher-rs/src/cartridge_boot.rs:1276-1358`
**Predicado:** `target_pool`/`alt_pool`/`direction`/`optimal_amount_in` del resultado Rhai no se usan; el plan viaja con `intent.legs` + `intent.amount_in`.
**Evidencia:** el loop de legs construye `RouteLeg` exclusivamente desde `intent.legs` (L1315-1353); del `eval_result` solo se leen `profit_usd_hint` y flags. `route_fingerprint = cartridge:tx:intra_tx_index` (L1281-1284) no incluye la ruta elegida.
**Contraejemplo:** dex_arb V3 elige alt_pool P2 dirección B; el SizeOptimizer recibe el plan con las legs del intent observado (P1/P2 según el mempool) y puede optimizar una ruta distinta a la aprobada por el script.
**Impacto:** el candidato simulado/emitido puede no ser la propuesta del cartucho (identidad de plan no verificada).
**Reparación:** thread de `route`/`amount_in`/plan_hash del resultado Rhai al plan; re-sizing = revisión nueva + re-cotización de todos los hops.
**Estado: CONFIRMADO** (sin `plan_hash` en ningún lado del backend — grep: 0 hits).

### [MEDIO] RHAI-13 — mev_01_009/010/011 (y familia CLOB/RFQ): stubs always-false external_feed_unavailable
**Archivo:** `backend/searcher-rs/cartridges/strategies/mev_01_009_amm_clob_arbitrage.rhai:40-51` (representativo)
**Predicado:** cobertura (264 archivos) ≠ criterio económico implementado.
**Impacto:** 174 NEEDS_ROUTE_DATA + familia external-feed jamás detectan (coherente con strategy_dispatch_status; no es bug sino estado honesto — reporte como transparencia).
**Reparación:** productor específico por familia antes de prometer ejecución.
**Estado: CONFIRMADO.**

### [MEDIO] RHAI-14 — funding_rate: proyección por ciclo con tamaño estático, gas configurado (50 USD), confianza fija 0.85
**Archivo:** `backend/searcher-rs/cartridges/funding_rate_arbitrage.rhai:122-186`
**Predicado:** `net = diff×size − gas_config` es forecast, no ganancia; `get_config()` local = `init_strategy()` (defaults hardcodeados, schema config decorativo).
**Impacto:** señal proyectada puede etiquetarse como profit.
**Reparación:** separar forecast de ciclo atómico; costes/fills desde productores reales.
**Estado: CONFIRMADO** (además: CORE/BAJO — config_schema "editable" sin wiring).

### [MEDIO] RHAI-15 — mean_reversion: slippage_cost usa precio unitario, no tamaño; beneficio depende de reversión a EMA
**Archivo:** `backend/searcher-rs/cartridges/mean_reversion_arbitrage.rhai:126-153`
**Predicado:** `slippage_cost = current_price × bps/10000` tiene unidades de precio (USD/token), se resta a `profit_usd` (USD) — falta multiplicar por tamaño.
**Contraejemplo:** precio $3 000, slippage 30 bps → "coste" $9.00 restado a un P&L en USD sobre posición de $10 000: el coste real a tamaño ≈ $3 000×0.003×(10000/3000)=$30; el código resta $9 (o $9 000 si el precio fuera $3M) — dimensión incorrecta.
**Impacto:** coste infradimensionado para precios <$1 000, sobredimensionado encima.
**Reparación:** slippage a tamaño con curva/orden real; etiquetar señal estadística ≠ arbitraje atómico.
**Estado: CONFIRMADO.**

### [ALTO] RHAI-16 (NUEVO) — triangular_arb: reserves sin orientar por token0 (r0 SIEMPRE es reserve_in)
**Archivo:** `backend/searcher-rs/cartridges/triangular_arb.rhai:184, 191, 198`
**Predicado:** `calculate_amount_out(x, res.r0, res.r1)` asume r0=reserve del token entrante en TODOS los hops; los pools V2 ordenan token0 por dirección, no por dirección del ciclo.
**Contraejemplo:** triángulo A→B→C→A donde en el pool BC resulta token0=C (C<B por dirección): el hop B→C usa r_in=reserva de C, r_out=reserva de B → quote invertido (~1/precio²) — el 50% de las orientaciones de cada pool queda mal cotizado.
**Impacto:** quotes de triángulo incorrectos para la mitad de las orientaciones posibles (los 264 generados SÍ orientan por `token0_addr`; el raíz no).
**Reparación:** orientar cada hop por `token0_addr` vs token_in del leg (patrón de mev_01_001 L84-87).
**Estado: NUEVO, verificado estático.**

### [ALTO] RHAI-17 (NUEVO) — liquidation.rhai: gross_profit = max_liquidatable(debt) × bonus — bonus en collateral calculado sobre cantidad de debt
**Archivo:** `backend/searcher-rs/cartridges/liquidation.rhai:78-93`
**Predicado:** el bonus de liquidación se recibe en COLLATERAL por el valor repagado; el código multiplica cantidad-debt × bonus y la mezcla con gas nativo y swap_cost en debt.
**Contraejemplo:** debt=1 WBTC ($60 000), collateral=USDT, bonus 5% → código: gross=0.05 "WBTC-units" (≈$3 000 si 1WBTC=$60k, accidentalmente); con debt=60 000 USDC y collateral=WBTC: gross=3 000 "USDC-units"=$3 000 pero el bonus real son 3 000/60 000=0.05 WBTC — mismo valor USD solo si USDC≈$1; con debt=PEPE el número es absurdo. `swap_cost` además se aplica sobre debt (lo que se vende es el collateral).
**Impacto:** P&L de liquidación dimensionalmente incorrecto para pares arbitrarios (el generado mev_08_014 sí lo hace bien: seized_usd/coll_px).
**Reparación:** seguir el patrón de mev_08_014 (seized_collateral con precios USD por token).
**Estado: NUEVO, verificado estático.**

### [MEDIO] RHAI-18 (NUEVO) — estimated_profit con unidades inconsistentes ENTRE cartuchos (USD vs token)
**Archivo:** `dex_arb.rhai:268` (USD, rama V3 viable) vs `mev_01_001:156` (token humano) vs `triangular_arb:117` (wei) vs `types.rs:52-53` ("base token").
**Predicado:** el tipo `CartridgeEvalResult.estimated_profit` documenta "base token" pero cada familia emite su unidad; `rd_outcome_v1/v2` (cartridge_boot.rs:568/641) lo serializan sin unidad.
**Impacto:** dataset `arbx:route_discovery:outcomes` mezcla unidades en una misma columna (labels S4/calibración contaminadas).
**Reparación:** campo `currency`/`unit` obligatorio o unificar a USD en la frontera del runner.
**Estado: NUEVO.**

### [MEDIO] RHAI-19 (NUEVO) — dex_arb: heurísticas de gas/tiempo hardcodeadas
**Archivo:** `dex_arb.rhai:735-745` (185k/150k ×2.2, sin escalar con N-hop, sin priority fee) y `760-781` (block_time por chain_id literal).
**Impacto:** subestimación de gas en rutas con flashloan (185k×2.2=407k vs ~450-500k real con premium), ~10-20% de gas (tip) nunca modelado; viola arbx-no-hardcode (chain IDs literales).
**Reparación:** gas medido/parametrizado por ruta; block time desde el scanner.
**Estado: NUEVO.**

### [BAJO] RHAI-20 (NUEVO) — to_wei f64→u128 trunca y pierde precisión >2^53
**Archivo:** `host_bindings.rs:783-787` — `(amount * 10f64.powi(dec)) as u128`.
**Contraejemplo:** para amount≥1e18 el espaciado f64 es ≥128 wei → el resultado puede diferir ±(128+1) wei del exacto (truncación `as`).
**Impacto:** menor (1 uso: liquidation build_payload, ya roto por RHAI-09).
**Reparación:** parse string→U256 como v2_amount_out_str.
**Estado: NUEVO.**

### [MEDIO] RHAI-21 (NUEVO) — config_schema decorativo: get_config()==init_strategy() (defaults hardcodeados)
**Archivo:** `funding_rate_arbitrage.rhai:265-267` y `mean_reversion_arbitrage.rhai:259-261`; costes gas 50/25 USD literales.
**Impacto:** la UI declara editable (`"editable": true`) lo que ningún wiring puede editar; costes fijos reemplazan dato requerido (anti-R8).
**Reparación:** binding host de config o quitar `editable`.
**Estado: NUEVO.**

### [BAJO] RHAI-22 (NUEVO) — hooks de lifecycle (on_activate/on_deactivate/on_new_block) jamás invocados
**Archivo:** `contract.rs:15-30` los documenta; `runner.rs`/`subscriber.rs` nunca los llaman (verificado por grep).
**Impacto:** superficie contractual muerta (contrato los declara "Optional").
**Reparación:** invocar en load/pause/new-block o eliminar del contrato.
**Estado: NUEVO.**

### [MEDIO] RHAI-23 (NUEVO) — mev_08_014: close_factor ausente defaultea a 1.0 (full close) — fail-open
**Archivo:** `mev_08_014_full_liquidation_arbitrage.rhai:150-151,166`
**Predicado:** Aave default close factor ≈0.5; el default local 1.0 asume poder liquidar TODO cuando el feed no trae el campo.
**Impacto:** sobre-tamaño de q (debt repaid) en posiciones con close factor real <1.
**Reparación:** default conservador (0.5) o rechazo `close_factor_unavailable`.
**Estado: NUEVO.**

### [ALTO] CORE-01 (NUEVO) — cartridge_boot: gas del §IV evidence divide milli-gwei entre 1e9 (error 1e6×)
**Archivo:** `backend/searcher-rs/src/cartridge_boot.rs:1244-1248`; convención del atómico: `block_scanner.rs:63-80` + `runner.rs:415` + test `block_scanner.rs:803-808` ("1.5 gwei = 1_500_000_000 wei → 1500 milli-gwei").
**Predicado:** el atómico guarda gwei×1000; `publish_declared_combo_evidence` espera `gas_price_gwei`; el divisor correcto es 1e3, el código usa **1e9**.
**Evidencia:**
```rust
let base_fee_gwei = (runner_ev.host_base_fee_handle().load(...) as f64) / 1e9;
```
**Contraejemplo:** base fee 30 gwei → atómico=30 000 → correcto 30 000/1e3=30.0 gwei; el código produce **3e-5** — 1e6× más chico.
**Impacto:** todos los operadores dependientes de gas del §IV (ver tabla math-engine) computan con gas ~0; evidence_vector distorsionado (alimenta scoring Bayesiano vía `arbx:math_evidence` → fold §IV del emitter).
**Reparación:** dividir por 1e3 (o reutilizar get_base_fee) + test de unidad del pasaje.
**Estado: NUEVO, verificado contra 3 fuentes del convenio.

### [ALTO] CORE-02 (NUEVO) — get_pool_index (binding) lowercasea los símbolos; pool_sync bootstrap escribe claves UPPERCASE; pool_discovery escribe lowercase en orden por dirección
**Archivo:** reader: `host_bindings.rs:302-330` (`to_lowercase` + sort); writer bootstrap: `pool_sync_worker.rs:1254-1277` (ordena por símbolo VERBATIM de PG — uppercase) vía `reserves.rs:85-92` (sin lowercase; test L464-473 fija `arbx:pool_index:1:USDC:WETH`); writer discovery: `pool_discovery.rs:749-777` (lowercase, orden token0/token1).
**Predicado:** tres convenciones de clave sobre el mismo índice.
**Contraejemplo:** pool_sync escribe `arbx:pool_index:1:USDC:WETH`; el cartucho llama `get_pool_index("USDC","WETH")` → clave `arbx:pool_index:1:usdc:weth` → **miss**. Solo matchean los pools descubiertos on-chain por pool_discovery Y cuyo orden de símbolos coincida con el orden por dirección.
**Impacto:** `insufficient_pools`/`incomplete_triangle` sistemáticos — el descubrimiento cross-pool de dex_arb V2 y triangular_arb (raíz) queda estructuralmente hambreado aunque el índice bootstrap esté poblado (explica el patrón "never-detected" de los runs HERMES).
**Reparación:** una única función canonizadora de clave (case+orden) compartida por writers y binding.
**Estado: NUEVO.**

### [MEDIO] CORE-03 (NUEVO) — simulate_swap: binding muerto + fallback V2 que consulta pool_index con DIRECCIONES contra índice por símbolos
**Archivo:** `host_bindings.rs:511-632` (binding registrado) — **0 usos en los 271 .rhai** (verificado por grep literal); fallback V2 L925-935 construye `arbx:pool_index:{chain}:{0xlo}:{0xhi}` (direcciones) contra índice escrito por símbolos.
**Impacto:** ~120 líneas de quoter RPC + rate-limiter nunca invocadas; si alguien lo cablea, su modo V2 jamás resuelve pools.
**Reparación:** eliminar o alinear la clave; registrar el gap en el contract test de bindings.
**Estado: NUEVO (binding no usado por ningún .rhai — ítem 2d del prompt).**

### [MEDIO] CORE-04 (NUEVO) — strategy_dispatch_status solo lo consume route_discovery_worker; el camino de emisión de cartuchos no lo consulta
**Archivo:** tabla `strategy_dispatch_status.rs:98+` (verificada 264 = 79+174+8+3 ✓); consumer único: `route_discovery_worker.rs:411,530`; el path `cartridge_boot::active_evaluate_and_emit` → emitter solo aplica `signal_tier` (Execution_Class).
**Predicado:** un MEV con dispatch NO_COMPATIBLE_ROUTE/NEEDS_ROUTE_DATA (p.ej. MEV-02-011) pero Execution_Class permisiva (DETERMINISTIC_*) puede formar candidato por el camino cartridge sin que la doctrina dispatch ("NEEDS_ROUTE_DATA nunca fabrica ruta") se aplique.
**Impacto:** bypass doctrinal del workbook en el segundo productor de candidatos.
**Reparación:** aplicar `disposition(mev_id)` también en cartridge_boot antes de formar candidato.
**Estado: NUEVO.**

### [MEDIO] CORE-05 (NUEVO) — size_optimizer: resolve_token_in_symbol mapea 5 direcciones mainnet→símbolo SIN gate de chain; fallback "WETH"
**Archivo:** `size_optimizer.rs:1805-1826`; caller `optimize_with_reason:382` sin filtro de chain_id.
**Contraejemplo:** candidato en Base (8453) con token_in USDC de Base (dirección ≠ mainnet) → no matchea → fallback pair_symbol o "WETH" → cap_usd y token_price_usd se resuelven contra el símbolo EQUIVOCADO (precio WETH $3 000 aplicado a un token de $0.01) → cap_wei inflado 300 000×.
**Impacto:** sizing con precio/cap equivocados en chains no-mainnet (mitigado por UnknownTokenPrice cuando el símbolo no existe en config).
**Reparación:** resolver símbolo por (chain_id, address) desde token_identity (ya existe el módulo); eliminar literales.
**Estado: NUEVO.**

### [BAJO] CORE-06 — amm_math::v2_amount_out: `10_000u32 − fee_bps` puede subflow si un caller pasa fee>10 000
**Archivo:** `amm_math.rs:81` — los dos callers actuales guardan el rango (host_binding L396; simulate_swap pasa 30 literal), riesgo latente.
**Reparación:** guard en el kernel.

### [BAJO] CORE-07 — comentarios stale "31-operator registry" vs OPERATOR_COUNT=32
**Archivo:** `host_bindings.rs:447`, `runner.rs:539` ("IDs 1-31") vs `operators/mod.rs:57` (`OPERATOR_COUNT=32`) y 32 archivos op_XX.
**Reparación:** actualizar comentarios.

### [BAJO] INV-01 — 18 artefactos de fuzz-cache commiteados bajo `arbitragex-v2-main/`
**Archivo:** `arbitragex-v2-main/contracts/cache/fuzz/failures/...` (18 tracked; ruta de ZIP extraído).
**Impacto:** basura de build en el repo; confusión de árboles.
**Reparación:** gitignore `contracts/cache/` y remover.

---

## 3. VERIFICACIÓN DE LOS 15 HALLAZGOS PREVIOS (resumen)
| ID | Estado | Nota |
|---|---|---|
| RHAI-01 | ✅ CONFIRMADO | re-derivado con contraejemplo propio + verificado el del operador |
| RHAI-02 | ✅ CONFIRMADO | rama V3 ya corregida en código; V2 no |
| RHAI-03 | ✅ CONFIRMADO | scope Rhai block-local; runner usa Scope::new() |
| RHAI-04 | ✅ CONFIRMADO | |
| RHAI-05 | ✅ CONFIRMADO | build_payload sin caller activo |
| RHAI-06 | ✅ CONFIRMADO | |
| RHAI-07 | ✅ CONFIRMADO | |
| RHAI-08 | ✅ CONFIRMADO | dec_a/dec_c recibidos y no usados |
| RHAI-09 | ✅ CONFIRMADO | |
| RHAI-10 | ✅ CONFIRMADO (matiz) | tipado f64 verificado seguro HOY en los 264; contrato frágil |
| RHAI-11 | ✅ CONFIRMADO y ESCALADO | consecuencia estructural: 0 aceptaciones posibles de los 264 |
| RHAI-12 | ✅ CONFIRMADO | sin plan_hash en todo el backend |
| RHAI-13 | ✅ CONFIRMADO | |
| RHAI-14 | ✅ CONFIRMADO | +config decorativa (RHAI-21) |
| RHAI-15 | ✅ CONFIRMADO | unidades de slippage |

## 4. TABLA DE FRONTERAS (verificadas en esta espina central)
| Frontera | Estado | Detalle |
|---|---|---|
| runner.rs parser ←→ .rhai emit | **FAIL (frágil)** | RHAI-10: unwrap_or(0.0); tipado HOY seguro en 264 |
| cartridge_boot RoutePlan ←→ opportunity map | **FAIL** | RHAI-12: plan desde intent.legs, no desde el resultado |
| SizeOptimizer ←→ plan_hash/amount_in Rhai | **FAIL** | no existe plan_hash (0 hits); amount_in del intent |
| emitter INSERT PG ←→ migrations | **OK** | 22 columnas existen (003+049+099+102+121); CHECK strategy_kind dropeado en 103 |
| publisher XADD ←→ api-server XREADGROUP | **OK** | misma clave arbx:opps:detected |
| get_token_price_usd ←→ price_worker hash | **FAIL** | RHAI-11: dirección vs símbolo uppercase |
| get_pool_index ←→ pool_sync/pool_discovery | **FAIL** | CORE-02: case/orden de clave |
| get_base_fee ←→ block_scanner milli-gwei | **OK (binding)** / **FAIL (§IV evidence)** | CORE-01: /1e9 en cartridge_boot |
| get_v3_slot0 ←→ pool_sync v3_slot0 | **OK** | clave por dirección lowercase ambos lados |
| strategy_dispatch_status (79/174/8/3) | **OK tabla / FAIL consumo** | CORE-04: cartridge path no consulta |
| strategy_hop_mask ←→ quotebase JSON | **OK** | 264 filas JSON; máscaras coherentes (01-015→h2, 01-016→h3) |
| detector_policy ←→ consumer | **OK (corregido)** | consumidor real: route_discovery_worker (no scanner); 60 entradas |
| signal_tier ←→ emitter | **OK** | bloquea OBSERVE_ONLY/SIGNAL con reason |
| math_evidence key writer ←→ emitter reader | **OK (clave)** / **FAIL (payload)** | STRAT-IDENT-01 fixó la clave pero MATH-02: forma Objeto vs Array esperada por fold/calibración |
| §IV fold ←→ stage2_calibration | **FAIL** | MATH-02: loop completo muerto |
| Redis opps stream maxlen | OK | MAXLEN ~10000 ambos lados |
| manifest cartridge_loader ←→ FS | OK | 271 .rhai cargados recursivamente |
| encoder Rust ←→ contratos (AE/FLE) | **OK** | 8 firmas/selectores 1:1; 0 FAIL (Anexo A) |
| sim_multistep SimulationOutcome ←→ consumers | **OK (forma)** / **FAIL (semántica gross)** | SIM-02: retained_spread gross sin gas |
| paper_stack slots ←→ sim_prefund | **OK** | fórmula ERC-7201 OZ v5 verificada exacta (Anexo A) |
| selector-api engine ←→ blacklist | **OK (tokens)** / **FAIL (robustez)** | SEL-01: poison messages; whitelist muerta SEL-02 |
| api-server WS ←→ frontend | (subagente FRONT — pendiente fusión) | usePricesStream verificado fail-honest por espina |

## 5. CONTRAEJEMPLOS ARITMÉTICOS EJECUTADOS (derivación manual entera)
| # | Módulo | Entradas | Salida código | Salida correcta | Veredicto |
|---|---|---|---|---|---|
| 1 | dex_arb V2 (RHAI-01) | reservas (1e6,1e6)/(1e6,1.001e6), fee 30bps, x=100 | diff paralela +0.099690 B → "oportunidad" | round-trip −0.718160/−0.519517 A | **FAIL** |
| 2 | dex_arb V2 gas (RHAI-02) | base_fee=30 gwei, gas=405k, token USDC | threshold 0.01458 | ≈$40 | **FAIL** |
| 3 | triangular (RHAI-08) | profit=5e17 wei, gas=0.0135 ETH | gate pasa (5e17>0.0176) | gate debe exigir ≈$17.55 | **FAIL** |
| 4 | liquidation to_wei (RHAI-09) | max_liq=1000, USDC 6dec | 1000×10^18 | 1000×10^6 | **FAIL (1e12×)** |
| 5 | §IV gas (CORE-01) | 30 gwei (atómico 30000) | 3e-5 | 30.0 | **FAIL (1e6×)** |
| 6 | pool_index case (CORE-02) | HGET …:usdc:weth vs SET …:USDC:WETH | nil | lista de pools | **FAIL** |
| 7 | price addr (RHAI-11) | HGET prices 0xabc… | nil → sin USD → rechazo no_computable_economics | $3 000 | **FAIL** |
| 8 | v2_amount_out (kernel) | x=99.690060, rin=1.001e6, rout=1e6, fee 30 | 99.281840 A | 99.281840 A | OK |
| 9 | v3_spot_price | sqrt=2^96, dec 18/18 | 1.0 | 1.0 | OK |
| 10 | v3_amount_out_single_tick z4o | fórmulas canónicas | — | — | OK (tests del kernel) |
| 11 | mev_01_001 cpmm_out | γ=0.997, r=1000/1005 | coincide Uniswap | — | OK |
| 12 | mev_08_014 seized_collateral | q×debt_px×(1+bonus)/coll_px | dimensionalmente OK | — | OK (default cf=1.0 fail-open → RHAI-23) |
| 13 | mean_reversion slippage (RHAI-15) | px=$3000, 30bps | $9 | $9×(tamaño/px) | **FAIL** |
| 14 | funding/mean_rev config | get_config defaults | gas 50/25 USD fijos | dato requerido | **FAIL (R8)** |

## 6. DEAD CODE / BINDINGS
- **simulate_swap**: registrado, 0 usos en .rhai (CORE-03).
- **on_activate/on_deactivate/on_new_block**: contrato los declara, nadie los invoca (RHAI-22).
- **build_payload** (runner): sin callers activos (RHAI-05).
- Adapters `thermodynamics/{dex_potential,liquidation_potential,triangular_curvature}.rs`: TODO(scaffold) sin wiring.
- `round_trip_executor.rs:318` — "TODO Phase 5: implement" (skeleton).
- `jit_v3_worker.rs:161-166` — TODO(BE-3.3): estrategia JIT íntegramente stub.
- `to_float` binding: usado 12× vía método `.to_float()` (OK, no muerto).
- unwrap()/expect() no-test en searcher-rs: 135, clasificados M11-allow (Lazy métricas `.expect("metric")`, `NonZeroUsize::new(1).unwrap()`, Mutex expects con documentación) — riesgo de pánico bajo y declarado.

## 7. ARCHIVOS FALTANTES / IDs MEV
- Conteo: ver §1.1 (delta explicado; 7 missing-on-disk eran 4 nombres UTF-8 + 3 submódulos gitlink).
- IDs MEV: 264 únicos sin gaps ni duplicados (§1.3). Árbol = hash de referencia del operador.

## 8. SECRETS EN ARCHIVOS NO-SANCTIONED
(pendiente de fusión con subagente CI/SEC — sección 2e/2f)

---

# ANEXO A — CONTRATOS + ENCODERS (subauditoría WEB3, verificación independiente)

**Inventario .sol (excl. lib/out/cache):** src/: ArbitrageExecutor, FlashLoanExecutor, AllowanceManager, AdminTimelock · src/core/: DeterministicFactory, WalletTopology · src/adapters/: 6 · src/dexes/: 4 · src/flashloans/: 4 · src/interfaces/: 3 · script/: 5 Deploy* · test/: 16 .t.sol. NO existe contracts/executor/.

### [MEDIO] WEB3-01 — MakerDssAdapter.dsrExit sin control de acceso drena posición DSR compartida
**Archivo:** `contracts/src/adapters/MakerDssAdapter.sol:90-103`
**Predicado:** cualquiera puede exit(wad) de la posición del adapter y enviar el DAI a `recipient` arbitrario, sin contabilidad por usuario.
**Contraejemplo:** A hace dsrJoin(1000e18); B llama dsrExit(…, 1000e18+yield, B) → B roba principal+rendimiento.
**Impacto:** pérdida de fondos SI se desplegara (hoy no está en ningún Deploy*.s.sol; marcado "NOT yet invoked from the hot path" L43-46). Escalaría a CRÍTICO en despliegue real.
**Reparación:** mapping por usuario o forzar recipient==msg.sender.

### [MEDIO] WEB3-02 — AaveV3CrossChainAdapter.withdraw envía a `to` arbitrario fondos custodiados
**Archivo:** `contracts/src/adapters/AaveV3CrossChainAdapter.sol:49-69`
**Predicado:** pool.withdraw quema aTokens del adapter y paga al `to` del llamador; supply() acepta onBehalfOf libre (L55).
**Contraejemplo:** A: supply(5 WETH, adapter); B: withdraw(5 WETH, B) → B se lleva el WETH de A.
**Impacto:** latente (no desplegado).
**Reparación:** forzar to==msg.sender o contabilidad.

### [MEDIO] WEB3-03 — AdminTimelock.initialize acepta minDelay=0; DeployTestnet sin gate de chainid
**Archivo:** `contracts/src/AdminTimelock.sol:77-83`; `DeployTestnet.s.sol:53-63,137`
**Predicado:** OZ TimelockControllerUpgradeable setea `$._minDelay=minDelay` sin check (lib L156); DeployTestnet corre contra cualquier RPC sin require(chainid) ni CONFIRM_*, delay 60s, deployer EOA único admin.
**Contraejemplo:** `forge script DeployTestnet.s.sol --rpc-url $MAINNET_RPC` despliega mainnet con timelock 60s y EOA admin.
**Impacto:** bypass operacional del estándar 24h (DeployMainnet SÍ exige CONFIRM_MAINNET_DEPLOY + chainid==1 + multisig: DeployMainnet.s.sol:58-84,179).
**Reparación:** require(minDelay>0) + require(block.chainid!=1) en DeployTestnet.

### [MEDIO] WEB3-04 — callbacks flash sin nonReentrant (FLE)
**Archivo:** `contracts/src/FlashLoanExecutor.sol:287-296, 312-355`
**Predicado:** sin ReentrancyGuard; llamada externa a ArbitrageExecutor con fondos+allowance activos.
**Mitigación:** auth 3 capas (msg.sender==aavePool / balancerVault+provider, L291-292/322-329), initiator==this, reentrar a requestFlashLoan exige EXECUTOR_ROLE, y el objetivo AE es nonReentrant (AE.sol:260,313). Sin vector explotable encontrado.
**Reparación:** ReentrancyGuardUpgradeable como defensa en profundidad.

### [BAJO] WEB3-05 — deploy-m5.sh:36 clave Anvil#0 hardcodeada (test pública, fail-closed salvo Sepolia; allowlist gitleaks).
### [BAJO] WEB3-06 — post-deploy-sepolia.sh:53,56-107 — Balancer Vault MAINNET en Sepolia + sends admin de EOA incompatible con DeploySepolia (ruta Balancer rota en Sepolia).
### [BAJO] WEB3-07 — swap_encoder.rs:117-144 — fee V3 u32 sin validar <2^24 (fail-closed revert; doc correcta).
### [BAJO] WEB3-08 — DeterministicFactory.deploy permissionless (squatting de salt namespace; intencional, no desplegado).
### [BAJO] WEB3-09 — FLE.sol:349-352 — forceApprove+safeTransfer redundante al repagar Balancer.

**Verificaciones negativas clave:** repayment flash ≥ amount+premium con fail-closed `FL_RepaymentShortfall` (FLE:349-352/376-378) — CORRECTO; sin downcasts uint256→uint128 en src/; sin mint/burn; grantRole mainnet = multisig+timelock 24h con renuncia del deployer (DeployMainnet:226-258); setRouterApproval onlyRole(ADMIN)+whitelist por selector (AE:473-481,518-521); sin claves reales en tests (keccak("arbx.test…")); mainnet = multisig, testnet = EOA única (reporte).

**Tabla encoder↔contrato (0 FAIL):** executeArbitrage 0x76d81cdf ✓ 7 args 1:1 · executeArbitrageFlashFunded 0xdde0bf51 ✓ · requestFlashLoan 0x5107d61e ✓ (provider en storage slot 2) · swapExactTokensForTokens 0x38ed1739 ✓ · exactInputSingle 0x414bf389 ✓ (uint24→WEB3-07) · exactInput 0xc04b8d59 ✓ · approve/balanceOf/transfer canónicos ✓. Selectores custom re-verificados contra keccak en tests in-repo; unidades wei/segundos/bps(≤50) consistentes; broadcast usa calldata sim-validado VERBATIM (bundle_builder.rs:278,315-345).

**paper_stack:** NO existe literal (0 matches). Equivalente: scripts paper-shadow + sim-core/sim_prefund.rs.
**Slot AccessControl: SIN MISMATCH** — `access_control_role_member_slot` (sim_prefund.rs:286-299) = keccak256(role‖ERC7201_base) → keccak256(pad32(account)‖role_slot), verificado contra OZ v5.1.0 pinneada (base 0x02dd…6800 re-derivada en test; cast cross-check 0x1d2cf3c1…906a; EXECUTOR_ROLE == AE.sol:127/FLE.sol:79). Nota: correcto SOLO para targets OZ v5 (repo usa 5.1.0 ✓); contra v4 caería en storage muerto → fail-closed.

---

# ANEXO B — SIM-CTL + RELAYS-CLIENT (subauditoría SIM/RELAY)

**retained_spread = fle_post − fle_pre (tarea obligatoria del prompt):** fle_pre/post = balanceOf(FLE, token_in) REVM antes/después del dispatch envuelto requestFlashLoan→callback→executeArbitrageFlashFunded (sim_multistep.rs:407-433; sequence_runner.rs:366-426). Ambos en wei token_in → **resta dimensionalmente consistente**; premium de flash CUBIERTO (el delta = profit−premium, sim_multistep.rs:754-759); amount_in neteado atómicamente; **gas NO descontado** (prices-free por diseño, gate neto-USD delegado a round_trip_executor).

### [ALTO] SIM-02 — retained_spread es GROSS: SIM_SUCCESS posible con trade perdedor neto de gas
**Archivo:** `backend/sim-core/src/sim_multistep.rs:775-846` (gate 800-802); `round_trip_executor.rs:70-82`; `sim-ctl/consumer.rs:649-664`
**Predicado:** passed=true solo exige retained_spread>0; el gas jamás se descuenta en el sim.
**Contraejemplo:** USDC in 1 000 000; premium Aave 0.05%=500; profit bruto ciclo=1.20 → retained=0.70 USDC=700 000 wei>0 → passed=true. Gas 450 000×25 gwei=0.01125 ETH≈$37.13 → **neto real −$36.43 con SIM_SUCCESS**.
**Impacto:** mitigado en terminus LIVE (three-gas rule BigDecimal); todo consumidor NO-relays del outcome ve ganancia bruta → estadística inflada.
**Reparación:** mantener gate gross pero etiquetar el campo como gross en nombre/persistencia; exigir net_usd_viable en todo consumidor que decida economía.

### [ALTO] SIM-01 — carriers cíclicos (#567) NUNCA pueden pasar el sim: SameTokenInOut contradice el propósito
**Archivo:** `sim-ctl/canonical_plan_consumer.rs:14-17,99-168,219-264`; `sim-core/sim_multistep.rs:482-484`
**Predicado:** resimulate inyecta plan.ctx (token_in==token_out en toda ruta cíclica) → validate_context rechaza.
**Contraejemplo:** carrier WETH→USDC→DAI→WETH → passed=false "same_token_in_out" → la clase `strategy_cyclic_route_not_simulatable_in_s4` que #567 venía a arreglar sigue sin SIM_PASS (solo cambia el reason tag).
**Reparación:** validar hops cíclicos en vez de identidad de extremos.

### [ALTO] SIM-05 — gate de slippage path legacy anvil dimensionalmente inválido entre decimales
**Archivo:** `sim-ctl/sim_engine.rs:243-258,153`
**Predicado:** slippage = (amt_in_tokenIN − actual_out_tokenOUT)/amt_in mezcla tokens.
**Contraejemplo:** (a) WETH(18)→USDC(6): amt_in=1e18, actual=3e9 (perfecto @ $3000) → pct≈99.7% FAIL; (b) USDC→WETH: actual>amt_in → pct=0% PASS con slippage real arbitrario.
**Reparación:** normalizar por DecimalsMap; conversión checked (as_u128 trunca >u128::MAX).

### [MEDIO] SIM-03 — signer_funding: probing escribe SENTINEL en slots ajenos y "restaura" CERO, no el valor original (signer_funding.rs:41,130-167; CANDIDATE_SLOTS [0,2,3,9]) — corrompe estado del fork dentro del snapshot. Reparación: eth_getStorageAt previo + restaurar valor real.
### [MEDIO] SIM-04 — truncado por índice de BYTE en String → panic si UTF-8 multibyte cae en el límite 200 (sim_engine.rs:269; revm_backend.rs:228-232,244-248) — availability del consumer. Reparación: chars().take(N).
### [MEDIO] SIM-08 — degradación silenciosa a zero-address/0 en parseo de payload (revm_backend.rs:79-81) — latente. Reparación: reject tipado.
### [MEDIO] SIM-10 — anvil_setStorageAt sin timeout (signer_funding.rs:95-104,138-147) — hang del funding path. Reparación: timeout 8s como balanceOf.
### [BAJO] SIM-06 — require_trace_hash/require_positive_net_profit config MUERTA (se setea en 10 call sites, jamás se lee; guards incondicionales).
### [BAJO] SIM-07 — intermediate_amount_out SIEMPRE None en path paper (label "intermediate_token_out_balance" que nadie escribe).
### [BAJO] SIM-09 — revert_risk_pct fabricado (0.5/50.0/100.0) persistido.
### [BAJO] SIM-11 — SequenceContext paper pinya SpecId::OSAKA fijo (sequence_runner.rs:193) vs hardfork real del bloque.
### [MEDIO] RELAY-03 — checklist 12 pasos solo `if let Some(pg_pool)` (submit_engine.rs:148-150,227): sin DB, live cae antes; belt-and-suspenders depende de esa línea. Reparación: require pg fail-fast al arranque.
### [BAJO] RELAY-04 — 3 × .expect(reqwest builder) M11-allow (relay_flashbots/titan/bloxroute).
### [BAJO] RELAY-05 — coinbase_diff malformado → 0 silencioso; EWMA f64 wei (submit_engine.rs:686-699).

**Higiene verificada (relays/sim):** default-deny impecable (enabled solo "true" exacto; chains malformado → allowlist VACÍA → deny-all; boot + per-firma); plan_validation exige ABI canónico re-encode, extremos/paths/routers/quotes/slippage ≤50bps, frescura 30s, fee flash re-computado on-chain; **net>0 + three-gas (admitted=min(net,claimed)>0 && ≥3×gas) + cap 2% capital en BigDecimal ANTES de broadcast** (execution_admission.rs:152-211); U256 checked en settlement con bail explícito antes de u64; HTTP con timeouts 5-20s en TODOS los relays/oracle/reconcile; cero TODO/FIXME; cero unwrap no-test en sim-ctl; sin Vec sin cota ni HashMap-order dependence (trace_hash en orden de llamada).

**Tabla SimulationOutcome productor↔consumidor:** passed ← guards multistep/verified → execution_admission.ensure!/validated_plan ✓ · simulated_profit_token_in (GROSS token_in wei) → binding.retained_profit_wei → usd(retained)−gas ✓ · intermediate_amount_out (None en paper — SIM-07) · gas_used_total/gas_price_wei → max_fee tx + gas USD ✓ · fail_reason (tags tipados) → clasificación PEL ✓ · wrapped_calldata → broadcast VERBATIM ✓ · evidence SOLO verified_simulation (paper nunca adjunta) ✓.

---

# ANEXO C — MATH-ENGINE + SELECTOR-API (subauditoría MATH/SEL)

**Premisa del prompt corregida:** NO existe sigmoid en operators/mod.rs ni op_11_bayes.rs. El único sigmoid vivo está en `math-engine/src/strategies/canonical_strategy.rs:54-93` — forma **estable** `1/(1+e^−z)`: z=±750 → límites correctos 0.0/1.0 sin NaN (la forma e^z/(1+e^z) daría inf/inf). **CORRECTA, sin contraejemplo.** op_11_bayes posterior Beta-Binomial mean=(α0+w)/(α0+w+β0+l) ✓ (test 8w/2l→0.75).

### [ALTO] MATH-01 — gas_price_gwei al MarketState = milligwei/1e9 (1e6× bajo) y 0.0 en la vía regime
**Archivo:** `cartridge_boot.rs:1244-1248` (writer); `block_scanner.rs:63-78` (unidad); `orchestrator.rs:459` (0.0 explícito "not carried in RouteIntent yet")
**= CORE-01 de esta auditoría; confirmado INDEPENDIENTEMENTE por 3 pases (espina central + docs + math).**
**Consumidores reales de gas_price_gwei:** op_15_golden_section.rs:102, op_21_newton.rs:105, op_26_flash_loan.rs:102 (op_32_multi_objective :621 NO registrado — dormante).
**Contraejemplo:** basefee 20 gwei → atómico 20 000 → pasado 2e-5. op_15 p_ref=2000: gas correcto 20×21000×1e-9×2000=0.84; computado 8.4e-7 (factor 1e6). Un pool sin edge (bruto +0.001, gas real 0.84) publica optimal_yield +0.001 en vez de −0.839.
**Impacto:** evidencia §IV / /api/math/evidence sobrestiman net-yield por TODO el gas real; calibración futura aprendería de labels sesgados. Vía observe-only hoy.

### [ALTO] MATH-02 — loop §IV completo (fold calibrado + Stage-2b) estructuralmente muerto por mismatch de forma JSON
**Archivo:** writers: `math_evidence.rs:115-152,331-348` (publican OBJETOS {primary_operators:[{op,scalar}]…}) vs readers: `priors_cache.rs:187-192` y `recon/stage2_calibration.rs:257-268` (exigen `Json::Array`); clave engine además divergente (`orchestrator.rs:448` usa `{:?}` RouterKind vs `opportunity_emitter.rs:585-588` strategy_kind).
**Predicado:** ∀ oportunidad, fold.posterior_log_odds=None ∧ calibration_applied=false aunque snapshot y calibración existan.
**Contraejemplo:** snapshot {primary_operators:[{op:15,scalar:0.42}]}, calibración log_lr[14]=+1.2 → esperado prior+0.504; obtenido None (Objeto no matchea Array). Stage-2b: 10 000 rows → op_n[k]=0 ∀k → log_lr≡0 flat.
**Impacto:** el gate Bayesiano calibrado que el diseño asume activo NO existe (misma clase que STRAT-IDENT-01 pero en el payload). Silent feature failure.
**Reparación:** publicar array plano (build_evidence_vector ya existe, sin call-site de producción) o enseñar el objeto a ambos readers; unificar clave engine. Test E2E fold≠None.

### [ALTO] MATH-03 — op_15/op_21: gross_yield = out(token1) − x(token0) sin conversión de precio; 21000 gas = transferencia no swap
**Archivo:** `op_15_golden_section.rs:104-111` (op_21_newton.rs:5-7 hereda)
**Contraejemplo:** r0=10 WETH, r1=30 000 USDC (pool a precio justo), γ=0.997, gas≈0: margen en 0 = 2991−1 > 0 ⇒ "edge"; x*=10 → f(10)≈**14 972 unidades de yield fantasma**. Modelo correcto (costo=x·p≈3000): margen 2991−3000<0 ⇒ x*=0. Error ~10³–10⁶ según el par.
**Impacto:** scalars de yield/break-even absurdos como evidencia para cualquier par no-1:1 (WETH/*).
**Reparación:** medir en numerario (out − x·p_ref); gas_units de swap (150-500k), no 21k.

### [MEDIO] MATH-04 — price_matrix construida con ratios r1/r0 SIN normalizar decimales y mezclando pares de la ruta (math_evidence.rs:60-67). Contraejemplo: WETH/USDC r0=5e15, r1=1e13 → "precio" 0.002 vs real 2000; op_27 spread=(2000−1)/1=199 900% en ruta triangular.
### [MEDIO] MATH-05 — op_26 gas fail-open: gas_units/token0_per_eth default 0.0 ⇒ gas_cost=0; publish_declared_combo_evidence pasa SIEMPRE features vacías (:201). Contraejemplo: y_net +$8 con gas real $12 → publicado +$8.
### [MEDIO] MATH-06 — op_11_bayes: features NaN/negativas atraviesan el guard (`wins+losses<1.0` con NaN=false ⇒ pasa) → scalar Some(NaN)/mean −2.0.
### [MEDIO] MATH-07 — scoring.rs:23-31: NaN atraviesa `net_expected <= 0.0` → final_score NaN (orden indefinido); score ∝ 1/frescura_ms crudo (opp net=$1/fresh=1ms puntúa 5× sobre net=$10/fresh=50ms).
### [BAJO] MATH-08 — vector evidencia 31 slots (op_32 excluido) y None→0.0 conflata "no computado" con "cero" (R8).
### [BAJO] MATH-09 — canonical_strategy: features no finitas propagan NaN a viability/yield (sigmoid NaN-safe pero su entrada no).

### [MEDIO] SEL-01 — blacklist ARROJA con dirección malformada → mensaje poison sin XACK para siempre (blacklist.ts:19-23; engine.ts:60 sin try/catch; consumer.ts:184-189 no ack). Contraejemplo: token_in="ETH" nativo → loop de error + lag permanente.
### [BAJO] SEL-02 — whitelist definida (blacklist.ts:39-45) SIN call-site: gate default-ALLOW; no existe blacklist de estrategias (StrategyDisabled vive en Rust).
### [BAJO] SEL-03 — gates de simulación (simulation_failed, revert_risk_too_high) estructuralmente muertos: consumer pasa sim:null (consumer.ts:211-213). Tests cubren decide() con sim → falsa confianza.
### [BAJO] SEL-04 — shim /score con umbrales propios (safety<50 vs cfg 70; gas cap 200 hardcode) diverge del consumer.

**Bayes selector-api verificado a mano:** prior 0.5, r 0.8, Beta_toxic(5,5), Beta_safe(2,8) → posterior 0.9846 ✓ (logSumExp estable).
**Kelly/sigmoid/primitivos compartidos CORRECTOS** — los defectos están en el cableado (unidades, defaults, formas de payload), no en la matemática nuclear.
**Tabla de 32 operadores:** uses_gas = op_15/op_21/op_26 (op_32_multi_objective dormante) — resto no consume gas. Fórmulas verificadas OK salvo MATH-03/04/05/06 señalados.

---

# ANEXO D — DOCS vs CÓDIGO (subauditoría DOC)

### [MEDIO] DOC-01 — **= CORE-01/MATH-01** (gas /1e9): confirmado por 2º pase independiente con citas cruzadas (runner.rs:415, block_scanner.rs:63-78, host_bindings.rs:644, runner.rs:699, scanner.rs:1080/1101 correcto — cartridge_boot único divergente).
### [MEDIO] DOC-02 — README dice "31 operadores" (README.md:7,19,21,25,43,50-52) y CLAUDE §34.1 "264×31=8.184" vs OPERATOR_COUNT=32 (desde 2026-09-11; con 32 serían 8.448). La checklist interna ya lo cerró el 09-19 pero README/CLAUDE siguen stale.
### [MEDIO] DOC-03 — `docs/EXECUTION_MODES_DOCTRINE.md` citado como fuente de verdad (CLAUDE.md:368) NO EXISTE (0 resultados en el árbol; gap charteado en WO-02d-DESIGN.md:42 y sin reparar).
### [MEDIO] DOC-04 — README omite relays-client (el ÚNICO firmador §34.3), selector-api, prioritization-spine, shared-rs. "24 services" CONFIRMADO contra compose.prod.yml (contados 1 a 1).
### [MEDIO] DOC-05 — variables §34-críticas sin doc de env: ARBX_LIVE_EXEC_* (0 grep en .env.example), ARBX_ORCHESTRATOR_MODE, ARBX_V3_ARB_MODE, ARBX_ROUTE_DISCOVERY_OUTCOMES. Positivo: docs/env/binance-ws-feed.md 100% confirmado (estándar a replicar).
### [MEDIO] DOC-06 — P0 de schema-drift SIGUE ABIERTO: drift_observations SIN escritor (0 INSERT repo-wide) mientras RegistryCoherenceStrip.tsx:61 muestra "COHERENT — 0 observaciones" sobre tabla siempre vacía → veredicto fabricado por ausencia (R8).
### [BAJO] DOC-07 — docker-compose.yml raíz = legacy 3 servicios vs canon compose.prod.yml (24).
### [BAJO] DOC-08 — deriva de líneas en citas de audits recientes (sustancia intacta).
### [BAJO] DOC-09 — tabla de dominios del README sin op_32.

**§34 vs live_exec_policy.rs — todos los claims CONFIRMADOS** (default Sepolia :3; mainnet soportado test :71-76 exacto; default-deny; malformado nunca parcial; MainnetRefused nunca existió; flags v1/v2/shadow/off).
**Verificación de claims de audits previos (5+5+5):** 13 CONFIRMED, 2 STALE (propagación de toggles LANDED post-audit; F1 batching del quoter LANDED — docs predicen fixes ya aplicados).
**Conteos verificados:** dispatch 79/174/8/3 ✓ (con desglose por categoría); strategy_mapping cubre ids 1..31 — **op_32 sin cartucho asignado** (hueco persistente); "264 cartridges" ✓.

---

---

# ANEXO E — API-SERVER ↔ FRONTEND (subauditoría FRONT)

**Correcciones de premisas del prompt (explícitas):** frontend/src/* NO existe (usa lib/hooks, components, features). OpportunityCard.tsx NO existe (real: OpportunityTradeCard.tsx + OpportunitySummaryGrid.tsx). enrichOpportunity NO existe (real: mapToOmniOpportunity, types.ts:343-474). estimated_profit_usd NO existe como nombre wire (real: expected_profit_usd/net_expected_profit_usd). gas_cost_usd NO existe en el wire (real: simulated_cost_breakdown.gas_usd). POST /opportunities NO existe (feed = GET /api/v1/opportunities/live + WS new_opportunity).

### [ALTO] FRONT-01 — amount_in_wei viaja como JSON number sin comillas por el WS → pérdida >2^53
**Archivo:** `database/migrations/025_opportunities_websocket_trigger.sql:6` + `websocket.ts:497-513` + `frontend/lib/store/types.ts:388`
**Predicado:** `row_to_json(NEW)` serializa NUMERIC(78,0) como número JSON; JSON.parse cliente → f64; el mapper hace String() del float ya redondeado. REST en cambio castea `::text` (exacto, opportunities-live.ts:270).
**Contraejemplo:** wei 1 234 567 890 123 456 789 → JS number 1 234 567 890 123 456 800 → **−11 wei silenciosos** en la frontera WS.
**Impacto:** hoy display; cualquier uso económico futuro del wei del WS corrupto.
**Reparación:** `amount_in_wei::text` en el payload NOTIFY (trigger forward-only) o castear en broadcastOpportunity; test round-trip >2^53.

### [MEDIO] FRONT-02 — differential REST vs WS: cards LIVE degradadas (sin token_info/leg_symbols/simulated_*)
**Archivo:** `opportunities-live.ts:565-647` vs fila WS cruda; `useOmniOpportunities.ts:208-213`; sim TS solo en REST (:828-898).
**Contraejemplo:** misma oportunidad — REST: Gas $2.31/Net $8.21/ⓘ WETH; WS card: "—" hasta refresh manual.
**Impacto:** degradación permanente de la vista LIVE (no miente — R8 "—").
**Reparación:** enriquecer en broadcast o re-idratar por REST tras upsert WS; documentar el diferencial como contrato.

### [MEDIO] FRONT-03 — eventos hot opportunity:detected/validated emitidos a NADIE (dead wire)
**Archivo:** `websocket.ts:956-957,1002` (emisión + XACK); único cliente (websocket-client.ts) sin importadores vivos; el feed escucha solo new_opportunity (socket-lifecycle.ts:81).
**Impacto:** trabajo/ancho de banda en vacío; status/gas_used/net_profit_wei jamás llegan a UI.
**Reparación:** cablear useHotOpportunities a un panel o desmontar el streamer.

### [MEDIO] FRONT-04 — room público de oportunidades transmite alpha MEV sin auth; comentario C4 del consumidor FALSO
**Archivo:** `websocket.ts:378-394` ("Public: allow connection without token") vs `socket-lifecycle.ts:27-29` ("rejects every handshake without admin token").
**Impacto:** suscripción anónima al feed de detecciones (decisión documentada WS-POLL-1 pero contradice objetivo A1); comentario falso induce errores futuros.
**Reparación:** decidir postura; corregir el comentario C4.

### [MEDIO] FRONT-05 — risk_score renderizado con DOS unidades (fracción vs porcentaje)
**Archivo:** `OpportunitySummaryGrid.tsx:124` (0.11) vs `format.ts:80-83` (11.0%).

### [MEDIO] FRONT-06 — campos sin productor: confidence_score_bps y gas_used — siempre null
**Archivo:** `types.ts:455-457` + `schemas.ts:110-130` (documenta "backend does not yet emit"); 0 emisores REST/WS (grep).
**Impacto:** "— unscored" permanente + trampa Number(wei-string) si se cablea el hot stream.
**Reparación:** emitir (scored_opportunities existe) o eliminar del ViewModel.

### [BAJO] FRONT-07 — candidates.ts:41 z.number() estricto (residual block-number-string en endpoint distinto).
### [BAJO] FRONT-08 — GateBanner.tsx fetch 3s descartado a console.log (componente muerto, sin timeout).
### [BAJO] FRONT-09 — useOpportunitiesStream legacy sin consumidor vivo.

**Incidente block-number-string (screenshot raíz): FIJADO EN AMBOS LADOS** — causa: BIGINT int8 → string de node-postgres vs tipo number → Zod rechazaba TODO el payload → OpportunityTicker "feed unavailable". Fix productor: normalizeBlockNumber (opportunities-live.ts:455-459, aplicado a block_number :614 y pipeline_latency_ms :623). Fix consumidor: BlockNumberWireSchema z.preprocess (schemas.ts:11-18; null queda null) + tests (schemas.test.ts:91-130).

**TS-safety sin hallazgos críticos:** eval/Function 0; secretos 0 (admin cookie httpOnly; guardas R2); fetch con timeout+retries+Zod; TODOS los WS hooks limpian en unmount; setState-in-render 0.

**Fórmula de gas real (productor, computeSimulatedNet.ts:201-207):** `gasUsd = gasUnits × gwei × 1e9 / 1e18 × base_token_price_usd` — consumida por el card como `cb?.gas_usd`; NO existe cálculo de gas client-side (el total del card = suma de filas conocidas, OpportunityTradeCard.tsx:205-208).

---

# ANEXO F — CI/CD + BUILD + SUPPLY CHAIN + SECRETS (subauditoría CI/SEC)

## 8. SECRETS EN ARCHIVOS NO-SANCTIONED (sección obligatoria 2e)

### [CRÍTICO] SEC-01 — DSN PostgreSQL real (alta entropía) commiteado
**Archivo:** `backend/api-server/inject-trigger.mjs:3` (tracked; commit d7f9c6d3 "feat: Zero Latency WebSockets…")
**Predicado:** existe el literal `postgres://arbx_rw:RRwDFjgqH61DwSRYiNIkEFt@localhost:5432/arbitragex_v2` — password random de 24 chars, NO placeholder (contrastar `REPLACE_ME` en .env.example:31-47).
**Evidencia (verificada por lectura directa del archivo en esta espina):**
```js
const url = "postgres://arbx_rw:RRwDFjgqH61uDwSRYiNIkEFt@localhost:5432/arbitragex_v2";
```
**Impacto:** violación directa de configs/secrets.policy.md §1 (T1: password arbx_rw = "Remote admin takeover") y §2 "Never commit". Write en DB para cualquiera con reach al host. La db `arbitragex_v2` es de la generación anterior del VPS — probabilidad alta de password RW real.
**Reparación:** (1) rotar ARBX_RW_PASSWORD ya sea o no activo; (2) borrar archivo o parametrizar `process.env.DATABASE_URL`; (3) auditar git history (gitleaks full-history es blocking pero este DSN evita la firma generic-api-key — considerar regla custom postgres-DSN).

### [MEDIO] SEC-02 — default DEPLOYER_PRIVATE_KEY = clave pública Anvil#0 (scripts/deploy-m5.sh:36 + 3 docs) — no es secreto real, pero como default de deploy en red pública = fondos drenables por cualquiera. Reparación: `${DEPLOYER_PRIVATE_KEY:?required}`.
### [BAJO] SEC-03 — claves anvil 0x…01/02 en docs de auditoría (falsos positivos; .gitleaks.toml:13-15 allowlista 40-hex anclado, 64-hex sigue detectándose).

**Sweep completo (universo git ls-files, excl. .env.example/compose):** 0x{64} = 63 hits TODOS falsos-positivos clasificados (topic0 Uniswap Sync/Transfer, slots EIP-1967, calldata fuzz, roles keccak, salt ASCII, límites secp256k1 en docs) — excepto SEC-01/02. sk_live=0 · AKIA=0 · ghp_=0 · postgres:// con credenciales: 1 real (SEC-01); resto placeholders/CI. **Makefile raíz, cliff.toml y deny.toml NO EXISTEN** (supply chain cubierto por cargo-audit + gitleaks + npm-audit-gate).

## Hallazgos CI/CD

### [ALTO] CI-01 — `cargo check --workspace` corre en la RAÍZ del repo (workspace vive en backend/)
**Archivo:** `.github/workflows/ops-live-testnet.yml:50-51` — sin working-directory; no existe Cargo.toml raíz (verificado). Los workflows sanos usan `defaults.run.working-directory: backend` (rust.yml:25-27) o `cd backend` (codeql.yml:37).
**Impacto:** el job `validate` (y por cascade e2e-live-testnet + deploy de ese workflow_dispatch) NUNCA pasa — gate de live-testnet muerto.
**Reparación:** `working-directory: backend` + `--locked`.

### [ALTO] CI-02 — docker-compose raíz referencia Dockerfile inexistente
**Archivo:** `docker-compose.yml:5-7` (context ./backend/semiotic-bridge — glob Dockerfile = 0); `docker/compose.hotpath-test.yml:164` (tests/e2e/fixtures/Dockerfile = 0).
**Impacto:** `docker compose up` raíz falla en build — stack legado/huérfano (relaciona DOC-07).
**Reparación:** eliminar o alinear con docker/compose.*.

### [ALTO] CI-03 — infra/docker/*: 3 Dockerfiles rotos (scaffolding fósil)
**Archivo:** `infra/docker/Dockerfile.{searcher,backend,frontend}` — copian `backend/src` (no existe, workspace 13 crates), esperan binarios Rust `api-server`/`selector-api` (son servicios NODE), rust:1.78 < MSRV 1.85, sin --locked. 0 referencias.
**Reparación:** `git rm infra/docker/`.

### [ALTO] CI-04 — searcher-rs/Dockerfile.edge (imagen paper-shadow) no compilable
**Archivo:** `backend/searcher-rs/Dockerfile.edge:6,18-27,36,57` — rust 1.78 < MSRV; copia 2 de 13 members; `--features paper-shadow` sin `-p searcher-rs` en manifest virtual; sin --locked. La feature SÍ existe (Cargo.toml:81, cfg-gates reales en sed_bridge.rs:36-268). 0 referencias en compose.
**Impacto:** la imagen paper-shadow del SOP-EDGE-001 es inutilizable.
**Reparación:** re-basar en Dockerfile (context ../backend, rust:1.91, `-p searcher-rs --features paper-shadow --locked`) o eliminar.

### [MEDIO] CI-05 — 30 refs de actions sin pin SHA (@v4/v5/v6 en ~18 workflows; los core están SHA-pinneados — inconsistente). Reparación: pin por SHA.
### [MEDIO] CI-06 — allowlist RUSTSEC de security.yml desalineada del Cargo.lock actual (justifica sqlx 0.7.4/rmcp 0.3.2 que YA NO están — lock tiene sqlx 0.8.1 único y rmcp 3.1.4). El gate bloquea advisories remediados y la doc miente. Reparación: re-verificar cada --ignore contra el lock.
### [MEDIO] CI-07 — feature `ml` de math-engine declarada y rota (candle-core 0.4 no compila; 0 usos cfg; ci.yml:56-59 documenta por qué NO --all-features). Todas las demás features verificadas EN USO (88 hits cfg). Reparación: eliminar o cfg real.
### [MEDIO] CI-08 — token-enricher Dockerfile `COPY . ./` sin capa lockfile-antes-de-fuente (único Rust vivo sin el patrón; --locked sí). Comentario :10-11 stale (1.82/1.75 vs real 1.91/1.85).

**Verificado-OK (no-finding):** tests con Redis/DB tienen guards (#[ignore] o skip ruidoso; integration-tests.yml y v3-fee-units provisionan servicios reales) · **0 archivos .hex y 0 COPY .hex** (pregunta 2f respondida) · compose secrets solo ${VAR} (${VAR:?} en prod) · .env.example 100% placeholders · toolchain 1.91.0 pineado consistente (CI + 7 Dockerfiles) · **generadores scripts/xls/*.py DETERMINISTAS** (asserts de ascendencia 264, sorted() en toda emisión de sets, newline fijo; no-determinismo solo intencional: nonces cripto) · alpha-map-parity.py determinista (universo git ls-files, todo sorted).

**Duplicados Cargo.lock (838 pkgs, 99 con >1 versión):** hashbrown ×5 · itertools/ark-*/base64/glam ×4 · **syn ×3 (serde ×1 limpio)** · rand/getrandom/reqwest/windows-sys ×3 · rustls 0.21.12+0.23.45 (0.21 vía ethers 2.0.14 — el gran driver de deuda) · ring 0.16.20 vulnerable (RUSTSEC-2025-0009/0010 ignorado justificado) · openssl AUSENTE (stack rustls ✓) · idna 1.1.0 actual.

**Cobertura env-vars:** 91 vars leídas en searcher-rs/relays-client/sim-ctl; **74 faltan en .env.example** — críticas: ARBX_LIVE_EXEC_ENABLED/_CHAINS (el switch de broadcast mainnet §34.3 SIN doc), SIM_SIGNER_ADDRESS (RULE 02 boot-var), TOPOLOGY_ADMIN_TOKEN, TITAN_*/BLOXROUTE_AUTH_HEADER (.env.example documenta BLOXROUTE_AUTH — NOMBRE DISTINTO), FLASHBOTS_STAGING_*, SIM_BACKEND/REVM_RPC_URL/ARBITRAGE_EXECUTOR, ARBX_ORCHESTRATOR_MODE.

---

# RESUMEN FINAL DE SEVERIDADES

| Bloque | CRÍT | ALTO | MEDIO | BAJO |
|---|---|---|---|---|
| RHAI (15 previos verificados + 8 nuevos) | 0 | 13 | 7 | 3 |
| CORE (espina Rust searcher) | 0 | 2 | 3 | 3 |
| MATH/SEL | 0 | 2 | 5 | 5 |
| SIM/RELAY | 0 | 3 | 5 | 7 |
| WEB3 (contratos+encoders) | 0 | 0 | 4 | 5 |
| DOC | 0 | 0 | 5 | 3 |
| FRONT | 0 | 1 | 5 | 3 |
| CI/SEC | 1 | 4 | 5 | 2 |
| **TOTAL** | **1** | **25** | **39** | **31** |

**Total: 96 hallazgos** (1 CRÍTICO + 25 ALTO + 39 MEDIO + 31 BAJO).
† CORE-01 = MATH-01 = DOC-01 (gas /1e9) triple-confirmado por pases independientes — contado una vez.
Nota de conteo: subagente CI reportó 7 852 tracked; el conteo autoritativo de esta espina (doble-verificado: ls-files + blob-index) es **7 863**.

## Top-10 accionables (mayor impacto primero)
1. **SEC-01**: rotar password arbx_rw + borrar/parametrizar inject-trigger.mjs + auditar history.
2. **RHAI-11**: resolver identidad address↔symbol en get_token_price_usd — sin esto los 264 cartuchos no pueden emitir NINGUNA aceptación (no_computable_economics).
3. **CORE-02**: unificar convención de clave pool_index (case+orden) writers↔binding — descubre cross-DEX.
4. **CORE-01/MATH-01/DOC-01**: /1e9→/1e3 en cartridge_boot:1246 + propagar gas real en orchestrator:459.
5. **MATH-02**: alinear forma del payload §IV (Objeto vs Array) — revive fold+calibración.
6. **RHAI-01/02/03**: rama V2 de dex_arb (resta paralela + unidades + scope) — reescribir como round-trip encadenado.
7. **RHAI-08/16**: triangular raíz (wei vs ETH + orientación token0) — normalizar y orientar.
8. **RHAI-12**: thread de route/amount/plan_hash del resultado Rhai al RoutePlan.
9. **CI-01..04**: reparar workflows/Dockerfiles rotos (gate live-testnet muerto; imágenes no compilables).
10. **DOC-06**: productor de drift_observations o degradar a NO COMPUTADO (veredicto fabricado en pantalla).

**Correcciones a las premisas del prompt (verificación negativa):**
1. NO existe sigmoid en operators/mod.rs — el único vivo (canonical_strategy.rs:93) es la forma estable y CORRECTA.
2. NO existen OpportunityCard.tsx ni enrichOpportunity — análogos verificados fail-honest (mapToOmniOpportunity).
3. NO existe POST /opportunities — feed por Redis stream + GET live + WS new_opportunity.
4. NO existen fixtures .hex en el repo (0 archivos, 0 COPY).
5. NO existe "paper_stack" literal — sim_prefund verificado CORRECTO (OZ v5 ERC-7201, cast cross-check).
6. La fórmula keccak(abi.encode(role,account)) sospechada NO es la usada (sería incorrecta para OZ v5); la real es la canónica anidada.
7. NO existe Makefile raíz, cliff.toml ni deny.toml.
8. NO existe docs/EXECUTION_MODES_DOCTRINE.md pese a citarse como fuente de verdad (DOC-03).

## 9. PREGUNTAS ABIERTAS QUE REQUIEREN RED/DB/RPC
1. ¿`arbx:token_prices:<chain>` contiene en vivo los símbolos canónicos UPPERCASE exactos que asume price_worker? (RHAI-11 verificado estático; volcado en vivo confirmaría incidencia.)
2. ¿XLEN arbx:route_discovery:outcomes crece con cartridges ACTIVE? (BUG-003 marcado cerrado.)
3. ¿Los pools de pool_discovery (lowercase) coexisten con duplicados UPPERCASE del bootstrap? (CORE-02: SCAN de claves lo cerraría.)
4. ¿La rama V3 con ARBX_V3_ARB_MODE=on produjo v3_arb_viable en logs? (validez práctica conf 0.3.)
5. Slots sim_prefund contra un grantRole REAL on-chain en fork (matemática verificada; falta ejecución en fork vivo).
6. ¿drift_observations sigue vacía en PG vivo? (DOC-06.)
7. ¿El password de SEC-01 sigue vivo en algún PG desplegado? (rotación recomendada incondicional.)

---
**FIN DEL REPORTE** — Auditoría extrema ArbitrageX v2 · 2026-09-24 · Método: estática/aritmética/fronteras, sin ejecución de código del proyecto, sin compilación, sin conexiones externas. Artefactos: manifest.sha256 (7 965), git-blob-index.txt (7 863), universe-count.txt.
