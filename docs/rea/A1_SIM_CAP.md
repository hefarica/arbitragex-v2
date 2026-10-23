# A1 SIM-CAP — encontrar UNA ruta conocida-buena que pase sim

Fecha de medición: **2026-10-23 13:37–17:52 UTC** · Autor: verificador · Repo: `hefarica/arbitragex-v2`
Base: `origin/main` = `6bde300e8b14434894e23885bd5b7fd927dc4c06` (los 5 P0 + docs mergeados y desplegados; deploy de código `af331ce7` completó **2026-10-22T13:59:50Z**).
Insumos: `docs/rea/FASE1_GATE.md` (t11), `docs/rea/FASE1_VEREDICTO.md` (t12, mi medición previa), `docs/rea/FASE1_BASE.md` (t1).

> **Alcance de esta unidad:** diagnosticar y **demostrar** por qué **ninguna** simulación pasa, y localizar el/los bloqueos con `ruta:linea`. No se tocó código de producción, ni config, ni rutas protegidas, y **no se escribió en la DB** (psql de solo lectura + Redis de solo lectura + logs). El objetivo ">0 passed=true" **no se alcanzó** y este documento explica por qué, con evidencia medida, en lugar de fabricarlo.

---

## 0. Resumen ejecutivo (veredicto)

1. **`passed=true` es hoy inalcanzable, y no por el mercado: por el propio criterio de pass.** `SELECT count(*) FROM simulations WHERE gas_estimate_wei IS NOT NULL` = **0** sobre **516,460** filas: **ningún `eth_call` de la tubería terminó con salida decodificable**. La rama que decide `passed` (`sim_engine.rs:188`) **nunca se ejecutó**; no existe ni una fila `slippage_too_high` ni `output_undecodable` (§6).
2. **El bloqueo nuevo y dominante en la población 2-pata es un desajuste de contrato introducido por la propia Fase 1**: TWO-LEG-01 (#1015, merge `b5809f08`) empezó a persistir el plan cerrado `[A, X, A]` con `token_in=A`, `token_out=X` (semántica de **par**), y el constructor de probes de `sim-ctl` clasifica la ruta como **abierta** cuando `token_in != token_out` y exige `path[-1] == token_out` ⇒ `A ≠ X` ⇒ **`route_path_not_representable:dex_arb` ANTES de tocar el fork**. Medido: **143 de 143** filas 2-pata de W1 mueren ahí (**100 %**), y la familia **no existía** antes del deploy (0 filas con `path>=3` en la ventana previa; primera fila 2026-10-22T14:03:01Z, 3 min después del deploy de 13:59:50Z).
3. **Los otros dos bloqueos vivos son capacidad/infra, no mercado**: `build_error` (656 en W1) es una **etiqueta engañosa** — el catálogo **sí** tiene `universal-router` (`shared-rs/src/chains.rs:428-441`) pero el encoder no tiene brazo para ese `RouterKind` y reusa el error `UnknownRouter` (`tx_builder.rs:57`, `:170-176`); y `rpc_error` (1,125 en W1) + `sim_signer_funding_slot_unresolved` (350) son el fork/RPC público y el sondeo de slot de balance (§5, §7).
4. **Advertencia de métrica (crítica para A1):** aunque se arregle lo anterior, el criterio de `passed` actual **no es honesto para pares cross-decimal**: `token_decimals()` está **hardcodeado a (18,18)** (`sim_engine.rs:318-321`), así que la rama R8 "cross-decimals ⇒ `None`" (`sim_engine.rs:290-296`) es **código muerto**. Con el heurístico resultante, un par `USDC(6)→WETH(18)` da `actual_out ≥ amount_in` en unidades crudas ⇒ **slippage 0 % ⇒ PASS falso**; y `WETH→USDC` da ≈99.7 % ⇒ **FAIL falso**. **Ningún `passed=true` debe aceptarse como evidencia hasta arreglar F2**: sería contaminar exactamente la métrica que esta unidad existe para producir.
5. **Corrección a t12 (errata honesta):** el estado "6 pools V3 WETH/USDC sin reservas muestreadas" era **un artefacto de mirar la tabla equivocada**. `pool_reserves` es **V2-only por diseño** (`pool_sync_worker.rs:1130-1148`); V3 vive en Redis `arbx:v3_slot0:<chain>:<addr>` y **5 de los 6** pools V3 WETH/USDC están muestreados y frescos; el sexto (`0x7bea3986…`, tier 1 %) **no** está porque `pools.is_active = false` (§3). `pool_ticks` (tabla PG para ticks) sigue **vacía** (0 filas).

---

## 1. Ventanas, fuentes y transporte

| Ventana | Definición | Filas |
|---|---|---|
| **W1** (post-deploy) | `detected_at >= '2026-10-22T13:59:50Z'` → 2026-10-23T17:52Z (**27.9 h**) | **2,674** opportunities · 2,667 sims |
| **W0** (2 h pre-deploy) | `>= '2026-10-22T11:59:50Z' AND < '2026-10-22T13:59:50Z'` | **62** |
| **2H** (últimas 2 h) | `simulated_at >= now() - interval '2 hours'` | **124** sims |
| All-time | toda la tabla `simulations` (retención 2026-10-11 → hoy) | **516,460** |

Transporte (reproducible; el `sql_query` del harness **no ejecuta** SQL, ver `FASE1_GATE.md` §5):

```bash
# SQL (solo lectura)
ssh -i ~/.ssh/arbx_hetzner root@195.201.235.70 \
  "docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -q -f -" < q.sql
# Redis (solo lectura)
ssh … "docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern 'arbx:v3_slot0:*'"
```

Cada cifra de este documento lleva el id de su query (Q1…Q15) y el SQL literal está en el **Apéndice §9**.

---

## 2. (1) Funnel por razón — y el desglose real de `candidate_incomplete`

### 2.1 Familias, por ventana (Q9, Q11, Q14)

| Familia | All-time | **W1** | 2H (live) | Span |
|---|---|---|---|---|
| `candidate_incomplete:amount_in_wei_zero` | **393,485** | **0** | **0** | 2026-10-11 04:27 → **2026-10-19 12:32** |
| `reverted: TransferHelper: TRANSFER_FROM_FAILED` | 63,601 | 377 | 8 | 2026-10-11 → 2026-10-23 16:52 |
| `rpc_error: (code: 3, execution reverted, data 0x)` | 27,865 | **1,125** | 63 | 2026-10-11 → 2026-10-23 16:49 |
| `sim_signer_funding_slot_unresolved` | 18,096 | **350** | 11 | 2026-10-11 → 2026-10-23 16:48 |
| `rpc_error: (-32603, EVM error InvalidFEOpcode)` | 3,294 | (incluido arriba) | — | — |
| `sim_timeout` | 3,139 | 16 | 3 | 2026-10-11 → 2026-10-23 16:10 |
| `build_error: router not in catalog … dex=universal-router` | 2,565 | **656** | 34 | 2026-10-19 12:33 → 2026-10-23 17:48 |
| `build_error: router not in catalog … dex=unknown` | 2,311 | 0 | 0 | 2026-10-11 → 2026-10-22 19:31 |
| `funding_balanceof_timeout` | 1,324 | 1 | 0 | 2026-10-11 → 2026-10-22 18:32 |
| `reverted: UniswapV2Router: INSUFFICIENT_OUTPUT_AMOUNT` | 568 | (en reverted) | — | — |
| **`route_path_not_representable:dex_arb`** | **143** | **143** | 5 | **2026-10-22 14:03:01 → 2026-10-23 17:49** |
| `funding_setstorage_rpc` / `anvil_setStorageAt` (RPC público 429/403/408/500) | 20 / 5 | 0 | 0 | 2026-10-12 → 2026-10-21 |

*(Los conteos de W1/2H crecen entre corridas porque la ventana es móvil: `FAM_W1` se midió a las 13:44Z y `RPNR_SPAN`/`PR1_W1` a las 17:52Z. La diferencia de 1-2 filas entre tablas de este documento es ese desfase, no un cambio de método.)*

**Hallazgo 1 — el "76.2 % `candidate_incomplete`" del veredicto anterior es histórico, no vivo.** Las 393,485 filas son **100 %** la subfamilia `amount_in_wei_zero` (`FR` en Q8: la única variante con conteo) y su rango termina el **2026-10-19 12:32:31Z**, cuatro días antes del deploy de la fase. En W1 hay **0**. Productor: `backend/sim-ctl/src/consumer.rs:1108` (`candidate_incomplete:amount_in_non_positive_{amount_in}`) y `:199` (`…amount_in_wei_zero`) — la fila llegó al consumidor con `amount_in_wei = 0` (Q8 `CI_LINK`: `amt_zero = 393,485`, `amt_null = 0`). Ese agujero **ya está cerrado**; no es el bloqueo actual.

**Hallazgo 2 — la familia que SÍ es nueva es `route_path_not_representable` (143, todas en W1, todas 2-pata).** Se abre en §4.

---

## 3. (2) Sampling de los pools V3 y de los "564 nunca muestreados"

### 3.1 Lo medido (Q5 de t12, Q10, Q15 + Redis)

| Hecho | Valor | Fuente |
|---|---|---|
| Pools activos en `pools` (chain 1) | 2,000 | Q6 (t12) |
| Pools con snapshot en `pool_reserves` en 1 h | 1,436 | Q6 (t12) |
| Pools V3 activos | **564** | Q15 `V3_TOTAL` |
| Pools V3 con filas en `pool_ticks` (PG) | **0** | Q10 `TICKS_SPAN` = 0 filas, `V3_POOLS…with_ticks = 0` |
| Claves Redis `arbx:pool_reserves:*` | **1,436** | probe Redis |
| Claves Redis `arbx:v3_slot0:*` | **537** | probe Redis |
| WETH/USDC V3 con `arbx:v3_slot0` | **5 de 6** | probe Redis por dirección |
| WETH/USDC V3 sin `v3_slot0` | `0x7bea39867e4169dbe237d55c8242a8f2fcdcc387` (tier 10000) | probe Redis `EXISTS = 0` |

Payload crudo (fresco; `ts`=1792777756 ⇒ 2026-10-23T16:29Z):

```
arbx:v3_slot0:1:0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8
{"sqrt_price_x96":"1615856618754944197496699925885774","liquidity":"4857954836198238338","ts":1792777756}
arbx:pool_reserves:1:0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc
{"r0":"9916764446249","r1":"4124785499712236730288","token0_addr":"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48","blk":26256924,"ts":1792777756}
```

### 3.2 Causa nombrada (código)

- **`pool_reserves` es V2-only por diseño**: `load_pools` filtra `d.protocol_type = 'UNISWAP_V2'` (`backend/searcher-rs/src/workers/pool_sync_worker.rs:1130-1148`; el comentario `:1131-1133` lo declara: *"V3 pools don't have getReserves()"*). El escritor de la tabla es `pool_sync_worker.rs:846`. ⇒ **mirar `pool_reserves` para juzgar V3 es un error de instrumento**, no un worker caído.
- **V3 se muestrea en otro store**: `load_v3_pools` (`pool_sync_worker.rs:1174-1186`) selecciona `is_active = TRUE` y `protocol_type='UNISWAP_V3'` y el poll de `slot0` escribe `arbx:v3_slot0:*` (537 claves vivas).
- **El pool que falta está inactivo**: `0x7bea3986…` (WETH/USDC 1 %) tiene `pools.is_active = false` (Q15 `POOL7B`), y el `WHERE … is_active = TRUE` lo excluye. **Causa nombrada, no "worker caído".**
- `pool_ticks` (ticks por pool, PG) **nunca se escribió** (0 filas): la liquidez por tick no está persistida; el consumo de V3 es vía `slot0+liquidity` de Redis.

### 3.3 Errata sobre `FASE1_VEREDICTO.md` §3

La frase *"los 6 pools V3 WETH/USDC no tienen snapshots de reservas, nunca"* es **correcta para `pool_reserves` y engañosa como diagnóstico**: 5 de 6 tienen `slot0` fresco en Redis. La consecuencia para t12 **no cambia** (la dislocación V2↔V3 sigue **no medida**, porque para eso hace falta el precio V3 en el instante del snapshot V2, y el par V2↔V2 medido es el único con ambas series), pero la causa declarada cambia y queda corregida acá.

---

## 4. (3) Ruta elegida y traza hasta el `fail_reason` exacto

**Ruta elegida** (Q11 `PICK`, la 2-pata más reciente con ambas patas en la familia V2 y token allowlisted):

```
opportunity_id : f55914d3-826e-4d97-800f-eef333a8ab88
detected_at    : 2026-10-23 13:47:25.379584+00
detector       : patterns · strategy_kind: dex_arb · pair: c02aaa…/1f9840…  (WETH/UNI)
dex_a / dex_b  : uniswap-v2 / uniswap-v2         (pools 0xd3d2e269…, 0xdafd6663…)
token_in       : 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2   (WETH)
token_out      : 0x1f9840a85d5af5bf1d1762f925bdaddc4201f984   (UNI)  ← contra-token del par
amount_in_wei  : 5000000000000000
route_metadata : {"token_addresses":["0xc02aaa…(WETH)","0x1f9840…(UNI)","0xc02aaa…(WETH)"],
                  "pool_addresses":[…,"…"], "dex_adapters":["uniswap-v2","uniswap-v2"],
                  "leg_amounts_in":["10000000000000000000","3324152049790452129816"],
                  "leg_amounts_out":["3324152049790452129816","2415508319371197274005"],
                  "leg_zero_for_one":[false,true], "fee_tier_bps":[30,30]}
sim.fail_reason: route_path_not_representable:dex_arb   (simulated_at 2026-10-23 13:47:27.361504+00)
sim.simulator  : not_implemented
```

**Traza del path de sim (ruta:linea)**

1. `sim_engine::simulate_with_route` construye el probe con el path: `sim_engine.rs:62` → `build_probe_with_path(opp, signer_from, route_path)`.
2. `tx_builder::build_probe_with_path` clasifica por **igualdad de token**, no por cierre del path (`backend/sim-ctl/src/tx_builder.rs:132-146`):
   ```rust
   let path_tokens = if token_in == token_out {        // :132  rama CERRADA
       if path.len() < 3 || path[0] != token_in || path[path.len()-1] != token_out { … CyclicRouteMissingPath }
       Some(path.to_vec())
   } else if path.is_empty() {                          // :139  rama 1-pata sin path
       None
   } else {                                            // :141  rama ABIERTA con path
       if path.len() < 2 || path[0] != token_in || path[path.len()-1] != token_out {
           return Err(BuildError::PathNotRepresentable(…));   // :143
       }
   ```
   Con `token_in = WETH`, `token_out = UNI`, `path = [WETH, UNI, WETH]`: entra por la rama **abierta** y `path[2] = WETH ≠ token_out = UNI` ⇒ `PathNotRepresentable`.
3. `sim_engine.rs:104-110` lo convierte en el motivo tipado `route_path_not_representable:{kind}` y lo publica con `not_implemented` (**nunca llega al fork**: no hay `eth_call`, no hay funding, no hay gas).
4. El clasificador de gaps lo mantiene como **gap de capacidad**, no como rechazo de mercado (`backend/sim-ctl/src/persistence.rs:155-158`).

**Por qué es exactamente la población 2-pata y por qué empezó con el deploy (Q12, Q13, Q15)**

| Evidencia | Valor |
|---|---|
| Filas W0 (pre-deploy) con `path >= 3` | **0 de 62** (`META_PRE`) — todas 1-pata (`path` len 2, `path[-1] == token_out`) |
| Filas W1 con `path >= 3` | **143** (`META_W1`) → **143 `route_path_not_representable`, 0 simuladas, 0 passed** (`PR1_W1`) |
| Filas con `token_in == token_out` en W0 y en W1 | **0 / 62** y **0 / 2,674** (`CLOSED_PRE`, `CLOSED_W1`) ⇒ la rama "cerrada" de `tx_builder.rs:132` es **inalcanzable** con lo que el emisor escribe hoy |
| Primera fila `route_path_not_representable` | **2026-10-22T14:03:01.852Z** (`RPNR_SPAN`) |
| Deploy de código completado | **2026-10-22T13:59:50Z** (`FASE1_GATE.md` §2) |

⇒ **Causa raíz:** el emisor (TWO-LEG-01, merge `b5809f08`) escribió `token_in`/`token_out` con semántica de **par** (`leg 0 buys token_in→token_out … leg 1 sells token_out→token_in`, cuerpo del commit) y el plan cerrado en `route_metadata.token_addresses = [A, X, A]`; el constructor de sims asume que una ruta cerrada se reconoce por `token_in == token_out`. **Los dos contratos no coinciden, y ningún test de integración los cruza** — el PR de TWO-LEG-01 validó la persistencia (`route_metadata`, `dex_b`) pero no que la fila resultante fuera construible por `sim-ctl`.

---

## 5. (4) Re-ejecución en paper/read-only: **NO ejecutada** (con razón y con sustituto literal)

- **No hay knob de dry-run**: `git grep -n -E "dry_run|DRY_RUN|no_persist|SIM_PERSIST" backend/sim-ctl/src` → **0 hits**. El consumidor escribe **siempre** el veredicto en `simulations` (`sim-ctl/src/persistence.rs`), y la tarea prohíbe escrituras en DB. Re-publicar el evento en el stream también reintroduciría un evento en producción (no es *read-only*).
- **Sustituto 1 — la salida literal del propio pipeline** (últimas 6 filas, 2026-10-23 17:48–17:51Z, Q14 `LAST`), es decir el mismo veredicto que produciría mi re-ejecución:

```
c0e61639-…|sim_signer_funding_slot_unresolved|f|2026-10-23 17:51:50.973797+00|anvil
cf96a13e-…|rpc_error: (code: 3, message: execution reverted, data: Some(String("0x")))|f|2026-10-23 17:50:38.840724+00|anvil
d06387eb-…|route_path_not_representable:dex_arb|f|2026-10-23 17:49:16.927401+00|not_implemented
0b2176df-…|rpc_error: (code: 3, message: execution reverted, data: Some(String("0x")))|f|2026-10-23 17:49:03.149248+00|anvil
5d30c352-…|reverted: TransferHelper: TRANSFER_FROM_FAILED, data: …|f|2026-10-23 17:48:54.155667+00|anvil
af6f895a-…|build_error: router not in catalog for chain=1 dex=universal-router|f|2026-10-23 17:48:23.569959+00|anvil
```

- **Sustituto 2 — la rama, ejecutada localmente**: `cargo test -p sim-ctl --lib tx_builder` (ver §8, `commandsRun`). El test `:659` ya asserta el caso `PathNotRepresentable`, y el test de catálogo `:724-731` prueba que `find_router_by_name` resuelve nombres PascalCase/kebab.

---

## 6. El criterio de `passed` no puede dispararse hoy (y por qué un "pass" sería falso)

| Hecho | Valor | Query/código |
|---|---|---|
| `simulations.passed = true` | **0** | Q14 `PASSED_N` |
| `simulations.gas_estimate_wei IS NOT NULL` | **0** (de 516,460) | Q14 `GAS_PRESENT` |
| Filas con `slippage_pct` no nulo | **0** | Q14 `SLIP_DIST` |
| Filas `slippage_too_high` / `output_undecodable` | **0** | Q14 `SLIP` (vacío) |

`gas_estimate_wei` sólo se escribe **después** de un `eth_call` exitoso (`sim_engine.rs:158-164`, `:190-193`). Con 0 filas: **la tubería nunca completó una llamada**, por lo que la decisión `passed = slippage <= max_slippage_for_pass_pct` (`sim_engine.rs:188`; default 5.0 % en `main.rs:610-615`) **jamás se evaluó**. No es que el slippage sea alto: **no hay slippage**.

Y aunque llegara: `compute_slippage` (`sim_engine.rs:283-313`) intenta protegerse con `dec_in != dec_out ⇒ None`, pero `token_decimals()` **devuelve siempre `(18, 18)`** (`sim_engine.rs:318-321`, con comentario "Until then: (18,18) = same-decimals legacy"). Consecuencias medidas por inspección del código (no ejecutadas, porque no hay `eth_call` que las produzca):

- par **cross-decimal** con `token_in` de 6 dec y `token_out` de 18 dec (p. ej. USDC→WETH): `actual ≥ amt_in` en unidades crudas ⇒ `diff = 0` ⇒ `slippage = 0 %` ⇒ **`passed = true` con cualquier salida** (pass falso);
- par inverso WETH→USDC: `actual ≪ amt_in` ⇒ `slippage ≈ 99.7 %` ⇒ **fail falso**.

⇒ **F2 es prerrequisito de A1**: aceptar un `passed=true` producido por ese heurístico destruiría la métrica que A1 debe destrabar.

---

## 7. Lista de bloqueos con `ruta:linea` y efecto medido

| # | Bloqueo | `ruta:linea` | Efecto medido | Fix mínimo |
|---|---|---|---|---|
| **F1** | El constructor clasifica la ruta por `token_in == token_out`; el plan 2-pata emite `token_out` = contra-token con `path=[A,X,A]` | `sim-ctl/src/tx_builder.rs:132-146` (condición `:142-143`); productor del error `sim_engine.rs:104-110`; escritor del plan: `searcher-rs` TWO-LEG-01 (`b5809f08`) | **143/143 (100 %)** de la población 2-pata de W1 muere antes del fork | Detectar cierre por **path** (`path.len()>=3 && path[0]==path[-1] && path[0]==token_in`) y encodear `encode_v2_path`/`encode_v3_exact_input`; o escribir `token_out` = token de cierre. **Cualquiera de las dos** desbloquea las 143 y toda ruta 2-pata futura. Añadir test de contrato que cruce el `route_metadata` real de PR1 con el builder. |
| **F2** | Criterio de pass no honesto en cross-decimal | `sim-ctl/src/sim_engine.rs:283-313` (`compute_slippage`), `:318-321` (`token_decimals` hardcode `(18,18)`) | bloquea la validez de cualquier `passed`; 0 mediciones producidas | Hilar `route_metadata.decimals` (hoy vacío: `{"map":{}}` en toda fila) o cotización S6; mientras tanto, **no aceptar `passed`** en pares de decimales distintos. |
| **F3** | `build_error: router not in catalog … dex=universal-router` — **etiqueta falsa** | `sim-ctl/src/tx_builder.rs:57` (texto de `UnknownRouter`) reusado en el brazo catch-all `_ =>` de `tx_builder.rs:185-190`; el catálogo **sí** tiene UniversalRouter: `shared-rs/src/chains.rs:428-441` (`find_router_by_name` lo resuelve, `tx_builder.rs:231-252`) | **656** sims en W1 (2,565 all-time), 34 en las últimas 2 h | Brazo `RouterKind::UniversalRouter` con encoding `execute(bytes,bytes[],uint256)` **o**, como mínimo, un `BuildError::RouterKindNotEncodable` con nombre propio para no mentir sobre el catálogo. |
| **F4** | Funding del signer en el fork | `sim-ctl/src/signer_funding.rs:248-350` (sondeo de `CANDIDATE_SLOTS`; familia en `:334`/`:350`) | **350** en W1 (18,096 all-time); los `funding_setstorage_rpc` reales son **429/403/408 de RPC público** (Q8: `drpc.org` "upgrade to paid plan") | RPC propio/con breaker para `anvil_setStorageAt`, o `anvil_setBalance/setStorageAt` por un nodo que no rate-limite; cachear slot por token (ya existe `slot_cache`). |
| **F5** | `rpc_error: execution reverted, data 0x` + `InvalidFEOpcode` | `sim-ctl/src/sim_engine.rs:173-177` (`extract_revert_reason` sobre `ProviderError`) | **1,125** en W1 (43 % del funnel), 63 en 2 h — el bloqueo más grande | Instrumentar el fork (`anvil` upstream RPC + estado): el `data 0x` sin razón apunta a estado/estado-fork faltante o a un nodo que responde por otro; requiere diagnóstico propio (no lo cerré acá). |

---

## 8. Qué NO se computó (fail-honest)

1. **Ningún `passed=true`**: no existe y no se fabricó. La ruta a un pass real es F1→F2→F3/F4/F5 y después la caza.
2. **No re-ejecuté la sim**: sin knob read-only (§5). La evidencia es la salida del propio pipeline (6 filas literales) + la rama ejecutada en tests.
3. **`rpc_error` no está desglosado por causa raíz**: 1,125 filas con `(code: 3, execution reverted, data: 0x)`; no separé "estado de fork faltante" de "nodo equivocado" — requiere instrumentar el fork, fuera del alcance de esta unidad.
4. **Los 27 pools V3 sin `slot0`** (564 − 537) no fueron enumerados uno por uno; sí verifiqué el caso WETH/USDC y su causa (`is_active=false`). Los otros 26 quedan declarados como no computados.
5. **`pool_ticks` vacía**: no hay ticks persistidos; el consumo V3 es `slot0+liquidity` de Redis. No medí si el encoder V3 usa ticks (el brazo V3 usa `encode_v3_exact_input*` con `DEFAULT_UNIV3_FEE = 3000` hardcodeado en `tx_builder.rs:80-83`, lo que es un riesgo adicional para pools de tier ≠ 3000: **FOLLOWUP F3b**).
6. **Restricciones respetadas**: no toqué `configs/app.toml`, `live_exec_policy.rs`, `bundle_builder.rs`; `paper_mode` intacto; sin cambios de pisos; **cero escrituras** en PG/Redis (solo lecturas); sin difusión on-chain.

---

## 9. Apéndice — SQL literal y salidas (Q8…Q15)

Todas las queries: `psql -q -f -` (tuples_only, `|`), **solo lectura**, por el transporte de §1. `now()` de la primera corrida: `2026-10-23 13:37:34.233069+00`; última corrida: `2026-10-23 17:52Z`.

### Q8 — familias exactas y desglose de `candidate_incomplete`

```sql
SELECT 'FR', fail_reason, count(*) FROM simulations GROUP BY 2 ORDER BY 3 DESC LIMIT 25;
SELECT 'CI_LINK', count(*), count(*) FILTER (WHERE o.id IS NULL) AS no_opp,
  count(*) FILTER (WHERE o.amount_in_wei IS NULL) AS amt_null,
  count(*) FILTER (WHERE o.amount_in_wei = 0) AS amt_zero,
  count(*) FILTER (WHERE o.route_metadata = '{}'::jsonb) AS rm_empty,
  count(*) FILTER (WHERE o.route_metadata->'token_addresses' IS NULL) AS no_token_addresses,
  count(*) FILTER (WHERE o.chain_id <> 1) AS other_chain
FROM simulations s LEFT JOIN opportunities o ON o.id = s.opportunity_id
WHERE s.fail_reason LIKE 'candidate_incomplete%';
```
```text
FR|candidate_incomplete:amount_in_wei_zero|393485
FR|reverted: TransferHelper: TRANSFER_FROM_FAILED, data: …|63601
FR|rpc_error: (code: 3, message: execution reverted, data: Some(String("0x")))|27865
FR|sim_signer_funding_slot_unresolved|18096
FR|rpc_error: (code: -32603, message: EVM error InvalidFEOpcode, data: None)|3294
FR|sim_timeout|3139
FR|build_error: router not in catalog for chain=1 dex=universal-router|2562
FR|build_error: router not in catalog for chain=1 dex=unknown|2311
FR|funding_balanceof_timeout|1324
FR|reverted: UniswapV2Router: INSUFFICIENT_OUTPUT_AMOUNT, data: …|568
FR|route_path_not_representable:dex_arb|142
FR|cyclic_route_missing_route_metadata:dex_arb|5
FR|fork_acquire_failed|2
CI_LINK|393485|0|0|393485|0|0|0
```

### Q9 — familias por ventana y spans

```sql
SELECT 'FAM_W1', coalesce(split_part(fail_reason,':',1),'(null)'), count(*)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id
WHERE o.detected_at >= '2026-10-22T13:59:50Z' GROUP BY 2 ORDER BY 3 DESC;
SELECT 'SPAN', coalesce(split_part(fail_reason,':',1),'(null)'), count(*), min(simulated_at), max(simulated_at)
FROM simulations GROUP BY 2 ORDER BY 3 DESC LIMIT 20;
SELECT 'CI_HOUR', to_char(date_trunc('hour', simulated_at),'MM-DD HH24'), count(*)
FROM simulations WHERE fail_reason LIKE 'candidate_incomplete%' GROUP BY 2 ORDER BY 2;
```
```text
FAM_W1|rpc_error|1125        FAM_W1|build_error|656        FAM_W1|reverted|377
FAM_W1|sim_signer_funding_slot_unresolved|350   FAM_W1|route_path_not_representable|142
FAM_W1|sim_timeout|16        FAM_W1|funding_balanceof_timeout|1
SPAN|candidate_incomplete|393485|2026-10-11 04:27:41.258107+00|2026-10-19 12:32:31.83051+00
SPAN|reverted|64169|2026-10-11 04:27:42.715772+00|2026-10-23 16:52:37.631648+00
SPAN|rpc_error|31200|2026-10-11 04:32:04.025972+00|2026-10-23 16:49:41.45232+00
SPAN|sim_signer_funding_slot_unresolved|18096|2026-10-11 04:32:39.093033+00|2026-10-23 16:48:32.776934+00
SPAN|build_error|4873|2026-10-11 04:40:12.937764+00|2026-10-23 16:52:37.646352+00
SPAN|route_path_not_representable|142|2026-10-22 14:03:01.852959+00|2026-10-23 16:50:33.838109+00
CI_HOUR|10-11 04|2603 … CI_HOUR|10-19 12|…  (histograma completo: ráfaga 10-11 → 10-19, cero después)
```

### Q10 — sampling V3 en PG (`pool_ticks`)

```sql
SELECT 'TICKS_SPAN', count(*), count(DISTINCT pool_id), min(timestamp), max(timestamp) FROM pool_ticks;
SELECT 'V3_POOLS', count(*) AS v3_pools,
  count(*) FILTER (WHERE EXISTS (SELECT 1 FROM pool_ticks t WHERE t.pool_id = p.id)) AS with_ticks
FROM pools p JOIN factories f ON f.id = p.factory_id JOIN dexes d ON d.id = f.dex_id
WHERE p.chain_id = 1 AND p.is_active AND d.protocol_type = 'UNISWAP_V3';
```
```text
TICKS_SPAN|0|0||
V3_TICKS|0x1445f32d…|100|0|   (×6: los 6 pools V3 WETH/USDC con ticks=0)
V3_POOLS|564|0
```

### Q11 — spans de `build_error` + candidatos que sobreviven la allowlist

```sql
SELECT 'BE2', fail_reason, count(*), min(simulated_at), max(simulated_at)
FROM simulations WHERE fail_reason LIKE 'build_error%' GROUP BY 2 ORDER BY 3 DESC;
SELECT 'CAND', o.dex_a, o.dex_b, s.fail_reason, count(*), max(s.simulated_at)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id
WHERE o.detected_at >= '2026-10-22T13:59:50Z'
  AND (o.rejection_reason IS NULL OR o.rejection_reason NOT LIKE 'TokenNotAllowed%')
GROUP BY 2,3,4 ORDER BY 5 DESC LIMIT 15;
```
```text
BE2|build_error: … dex=universal-router|2565|2026-10-19 12:33:15.904411+00|2026-10-23 17:48:23.569959+00
BE2|build_error: … dex=unknown|2311|2026-10-11 04:40:12.937764+00|2026-10-22 19:31:59.822258+00
BE_W1|build_error: … dex=universal-router|659
CAND|uniswap-v3|uniswap-v3|route_path_not_representable:dex_arb|28|2026-10-23 16:06:39.604104+00
CAND|uniswap-v2|uniswap-v3|route_path_not_representable:dex_arb|11|2026-10-23 14:56:24.541159+00
CAND|uniswap-v3|uniswap-v2|route_path_not_representable:dex_arb|10|2026-10-23 17:49:16.927401+00
CAND|uniswap-v2|uniswap-v2|route_path_not_representable:dex_arb|4|2026-10-23 13:47:27.361504+00
CAND|universal-router||build_error: … dex=universal-router|2|2026-10-23 16:05:09.465247+00
PICK|f55914d3-…|2026-10-23 13:47:25.379584+00|c02aaa…/1f9840…|uniswap-v2|uniswap-v2|WETH|UNI|5000000000000000|-0.00984871|NegativeNetProfit|{…,"token_addresses":["WETH","UNI","WETH"],…}|route_path_not_representable:dex_arb
```

### Q12 — semántica cerrada/abierta y span de `route_path_not_representable`

```sql
SELECT 'CLOSED_PRE', count(*) FILTER (WHERE token_in = token_out), count(*)
FROM opportunities WHERE detected_at >= '2026-10-22T11:59:50Z' AND detected_at < '2026-10-22T13:59:50Z';
SELECT 'CLOSED_W1', count(*) FILTER (WHERE token_in = token_out), count(*)
FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';
SELECT 'RPNR_SPAN', count(*), min(simulated_at), max(simulated_at)
FROM simulations WHERE fail_reason LIKE 'route_path_not_representable%';
SELECT 'RPNR_TOKEN', count(*) FILTER (WHERE o.token_in = o.token_out), count(*) FILTER (WHERE o.token_in <> o.token_out), count(*)
FROM simulations s JOIN opportunities o ON o.id = s.opportunity_id
WHERE s.fail_reason LIKE 'route_path_not_representable%';
```
```text
CLOSED_PRE|0|62
CLOSED_W1|0|2674
RPNR_SPAN|143|2026-10-22 14:03:01.852959+00|2026-10-23 17:49:16.927401+00
RPNR_TOKEN|0|143|143
```

### Q13 — metadata con path, pre vs post deploy

```sql
SELECT 'META_PRE', count(*), count(*) FILTER (WHERE route_metadata->'token_addresses' IS NOT NULL),
  count(*) FILTER (WHERE jsonb_array_length(route_metadata->'token_addresses') >= 2) AS path_ge2,
  count(*) FILTER (WHERE jsonb_array_length(route_metadata->'token_addresses') >= 3) AS path_ge3
FROM opportunities WHERE detected_at >= '2026-10-22T11:59:50Z' AND detected_at < '2026-10-22T13:59:50Z';
SELECT 'META_W1', … (misma forma, W1);
```
```text
META_PRE|62|62|62|0
META_W1|2674|2674|2674|143
PRE2H_SIM|build_error|22   PRE2H_SIM|rpc_error|21   PRE2H_SIM|sim_signer_funding_slot_unresolved|10
PRE2H_SIM|reverted|8      PRE2H_SIM|sim_timeout|1
```

### Q14 — ¿alguna vez se ejecutó un probe?

```sql
SELECT 'PASSED_N', count(*) FILTER (WHERE passed) FROM simulations;
SELECT 'SLIP', fail_reason, count(*) FROM simulations WHERE fail_reason IN ('slippage_too_high','output_undecodable') GROUP BY 2;
SELECT 'GAS_PRESENT', count(*), count(*) FILTER (WHERE passed) FROM simulations WHERE gas_estimate_wei IS NOT NULL;
SELECT 'SLIP_DIST', count(*), round(min(slippage_pct)::numeric,4), round(max(slippage_pct)::numeric,4)
FROM simulations WHERE slippage_pct IS NOT NULL;
SELECT 'LAST', opportunity_id, fail_reason, passed, simulated_at, simulator FROM simulations ORDER BY simulated_at DESC LIMIT 6;
```
```text
PASSED_N|0
SLIP|            (0 filas)
GAS_PRESENT|0|0
SLIP_DIST|0||
LAST|c0e61639-…|sim_signer_funding_slot_unresolved|f|2026-10-23 17:51:50.973797+00|anvil
LAST|cf96a13e-…|rpc_error: (code: 3, message: execution reverted, data: Some(String("0x")))|f|2026-10-23 17:50:38.840724+00|anvil
LAST|d06387eb-…|route_path_not_representable:dex_arb|f|2026-10-23 17:49:16.927401+00|not_implemented
LAST|0b2176df-…|rpc_error: (code: 3, …)|f|2026-10-23 17:49:03.149248+00|anvil
LAST|5d30c352-…|reverted: TransferHelper: TRANSFER_FROM_FAILED, …|f|2026-10-23 17:48:54.155667+00|anvil
LAST|af6f895a-…|build_error: router not in catalog for chain=1 dex=universal-router|f|2026-10-23 17:48:23.569959+00|anvil
```

### Q15 — por qué falta el pool del 1 % y población 2-pata de W1

```sql
SELECT 'POOL7B', p.address, p.is_active, p.fee_tier, d.name, d.protocol_type, f.address, p.created_at
FROM pools p JOIN factories f ON f.id = p.factory_id JOIN dexes d ON d.id = f.dex_id
WHERE p.address = '0x7bea39867e4169dbe237d55c8242a8f2fcdcc387';
SELECT 'PR1_W1', count(*) AS rows_2leg,
  count(*) FILTER (WHERE s.fail_reason LIKE 'route_path_not_representable%') AS rpnr,
  count(*) FILTER (WHERE s.fail_reason IS NULL) AS no_sim,
  count(*) FILTER (WHERE s.passed) AS passed
FROM opportunities o LEFT JOIN simulations s ON s.opportunity_id = o.id
WHERE o.detected_at >= '2026-10-22T13:59:50Z' AND jsonb_array_length(o.route_metadata->'token_addresses') >= 3;
```
```text
POOL7B|0x7bea39867e4169dbe237d55c8242a8f2fcdcc387|f|10000|UniswapV3|UNISWAP_V3|0x1f98431c8ad98523631ae4a59f267346ea31f984|2026-10-09 13:20:28.774681+00
POOLCMP|0x88e6a0c2…|500|UniswapV3|UNISWAP_V3
POOLCMP|0x8ad599c3…|3000|UniswapV3|UNISWAP_V3
RPNR_PLAN|143|7|143|143
V3_TOTAL|564
PR1_W1|143|143|0|0
```

### Redis (solo lectura, mismo día)

```bash
docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern "arbx:pool_reserves*" | wc -l   # 1436
docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern "arbx:v3_slot0:*"   | wc -l       # 537
docker exec arbitragex-v2-redis-1 redis-cli EXISTS arbx:v3_slot0:1:0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8   # 1
docker exec arbitragex-v2-redis-1 redis-cli EXISTS arbx:v3_slot0:1:0x7bea39867e4169dbe237d55c8242a8f2fcdcc387   # 0
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:v3_slot0:1:0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8
# {"sqrt_price_x96":"1615856618754944197496699925885774","liquidity":"4857954836198238338","ts":1792777756}
```
