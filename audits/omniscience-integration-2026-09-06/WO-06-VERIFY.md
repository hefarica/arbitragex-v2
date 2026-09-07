# WO-06 — VERIFY adversarial del diseño (peer-review post-hoc del WO-06-DESIGN.md)

- **WO:** WO-06 · kind: **verify** · Fecha: 2026-09-06 (noche).
- **Agente:** math-validator — Gang Omniscience (rubric `ecc:rust-reviewer` para anclas de código).
- **Objeto:** `WO-06-DESIGN.md` (565 líneas, 44 KB, aterrizó 19:26 sin peer-review) — filtro anti-flood estructural ANTES de la emisión (roadmap P1-2).
- **Veredicto:** **PASS — VERIFICADO, sin defecto CRITICAL en la matemática ni en las anclas. NO BLOCK.** Se requieren 3 correcciones al implementar (M1–M3) y se registran 6 observaciones menores. Detalle abajo.
- **Método:** lectura de código contra **HEAD de la branch `a6-cbprom-01`** (la referencia que el propio diseño declara en §9) + `git show HEAD:` para neutralizar el drift del working tree (ver O6). **0 cargo** (charter READ-ONLY). **0/5 requests HTTP al dominio** — la verificación es analítica + código; no hay artefacto desplegado que observar. Cero toques al VPS, cero git, cero terminus (§32/§33/§34.3).

---

## 1. Matemática del predicado F1 — VERIFICADA (charter ítem 1)

### 1.1 La trampa ruta-level vs leg-level está correctamente evitada

- **El test canónico existe y dice lo que el diseño cita:** `route_intent.rs:334-342` (`multi_leg_intent_valid`) construye legs **[A→B, B→C, C→A]** y exige que el intent sea válido. Para ese intent, `intent.legs.iter().any(|l| l.token_in == l.token_out)` es **false** en las tres legs (A≠B, B≠C, C≠A) → **el ciclo cerrado legítimo de Holonomic Loop Resolution PASA**. Verificado contra el código real, no contra la cita.
- **Un filtro ingenuo por `pair_symbol == "X/X"` habría matado todo el triangular:** `cartridge_boot.rs:1140` construye `token_in = format!("{:#x}", first.token_in)` / `token_out = last.token_out` y `:1161` arma `pair_symbol = "{in}/{out}"` — un ciclo [WETH→USDC, USDC→PEPE, PEPE→WETH] produce `pair_symbol = "0xWETH/0xWETH"`. El diseño identifica esta trampa explícitamente (§1) y su test T4 la fija. Correcto.
- **Caso borde embebido (charter):** [A→B, **B→B**, B→A] → `any()` → `true` → **rechazo del intent completo, NO poda de la leg** — exactamente la semántica que el charter exige, y declarada así en §1 con la justificación de dominancia (la variante podada [A→B, B→A] paga menos fees). Test T6 la fija.

### 1.2 Tipo de dato: la comparación es estructuralmente case-insensitive

`RouteIntentLeg.token_in`/`token_out` son `ethers::types::Address` (`route_intent.rs:110-112`), no Strings. `==` sobre `H160` es comparación de 20 bytes — la representación hex (checksum vs lower) no existe a ese nivel, y `route_decoder.rs:258-260` asigna `w[0]`/`w[1]` directamente del path decodificado. La nota del diseño §1 ("comparación de direcciones, case-insensitive") es correcta en efecto. En el path legacy, donde SÍ hay Strings, `patterns.rs:44-49` produce hex **lowercase** (`hex::encode`), y el diseño usa `eq_ignore_ascii_case` (`swap_is_self_pair`) — cubierto por ambas vías. **No existe vía de bypass por case-mix.**

### 1.3 Búsqueda de contraejemplos (charter: "mate rutas legítimas o deje pasar no-ops")

**¿Mata rutas legítimas? Ninguna encontrada.** Para que una leg X→X sea legítima se requeriría una Variedad de Liquidez con token0 == token1: V2 lo impide (CREATE2 de tokens idénticos revierte), V3 exige token0 < token1 (distintos por construcción), Curve/Balancer exigen coins distintos. Los "wrap/unwrap" (ETH↔WETH) usan addresses distintos y pasan. Un swap misdecodificado (zap/deposit con path [X,X]) no es un intent arb — suprimirlo es correcto. Los ciclos descubiertos por el **grafo** no pueden contener legs self-pair (las aristas conectan tokens distintos: `route_intent_dispatcher.rs:80`, `workers/route_scanner_worker.rs:365`), por lo que F1 tampoco toca el path de route-discovery.

**¿Deja pasar no-ops?** Una clase residual, correctamente fuera del alcance declarado: el round-trip **mismo par, mismas pools** ([A→B, B→A] por la MISMA pool) tiene Topological Yield negativo pero es **indemostrable a nivel de geometría** (las legs no son identidad; se necesita identidad de pool, y `pool_hint` es `Option`/R8). El diseño lo pasa (T5) y deja que el sizing/spine lo mate aguas abajo — decisión correcta: pasar el no-op improbable a un gate existente es el lado seguro del trade-off.

**Prueba adicional que el diseño no explicita (refuerza I3):** en el path legacy el predicado es sobre **endpoints del candidato** (`token_in` vs `token_out`), no sobre legs — un tx de usuario con path [A,B,A] tiene token_in == token_out == A y TAMBIÉN se suprime. Esto es seguro porque un candidato dex_arb del par (A,A) es estructuralmente inejecutable (no existe pool del par A/A — misma imposibilidad de §1.3 arriba), y la evidencia de producción lo confirma: ese shape es exactamente el flood 100%-rejected de 15 días. Cero pérdida demostrable también en I3.

### 1.4 Kernels F2/F3

- `FloodGate::admit_rejected` (`WO-06-DESIGN.md` §3.1): semántica de ventana correcta (`duration_since < window` → suppress; expirado → admit, consistente con T7); sweep con `retain` que conserva solo entradas frescas; map acotado (4096). Mutex simple a ≤25 rejects/s: sin riesgo.
- `pool_liquidity_usd`: fórmula correcta (`r/10^d · p` por lado, suma solo si AMBOS lados computables — `None` fail-honest si falta un precio). **Pero el spec del test T9 tiene un error aritmético — ver M1.**

---

## 2. Anclas file:line — TODAS verificadas contra HEAD (charter ítem 4)

Verificadas línea a línea en esta sesión (branch `a6-cbprom-01`):

| Ancla del diseño | Realidad en HEAD | Estado |
|---|---|---|
| `route_decoder.rs:249-266` (`windows(2)`, `token_in: w[0], token_out: w[1]`, sin check self-pair) | Exacto | ✅ |
| `cartridge_boot.rs:959-964` (info! enter), `:966-978` semáforo, `:984` list_cartridges | Exacto | ✅ |
| `cartridge_boot.rs:1038-1042` reserves first-leg, `:1054-1060` identity, `:1065-1073` price_snapshot | Exacto | ✅ |
| `cartridge_boot.rs:1085` loop `for … in pertinent`, `:1096-1104` `emit_shadow_outcome` | Exacto | ✅ |
| `cartridge_boot.rs:1153-1183` Opportunity, `:1161` pair_symbol, `:1233-1236` route_fingerprint | Exacto | ✅ |
| `cartridge_boot.rs:1399-1477` SizeOptimizer; `:1649-1657` spine TokenNotAllowed; `:1514-1523` summary | Exacto | ✅ |
| `cartridge_boot.rs` emit_rejected callsites | **8 sitios, no 7** — ver O1 | ⚠️ cobertura OK |
| `opportunity_emitter.rs:337` docstring "always persisted (no dedup check)", `:349-423` emit_rejected, `:356-372` counters, `:374-393` dry_run, `:401` score_and_publish, `:409` PG, `:413` XADD | Exacto | ✅ |
| `scanner.rs:1609` build_dex_arb_candidate, `:1617` trading_config, `:1630-1642` enriquecimiento Redis | Exacto | ✅ |
| `scanner.rs:2004-2038` no-config, `:2185-2235` TokenNotAllowed, `:2236-2278` StrategyDisabled, `:2284-2320` gate-blocked | Exacto | ✅ |
| `scanner.rs:2179-2183` doctrina de persistir rechazos de política ("RULE 00 transparency") | Exacto (`:2178-2183`) | ✅ |
| `scanner.rs:2581-2586` "Rejected rows … skip this check" (OppDedup solo filas aceptadas) | Exacto | ✅ |
| `scanner.rs:2656-2658` invariante None≠0 (`compute_gross_usd_for_spread` doc en HEAD :2656-2658) | Exacto en HEAD | ✅ (drift WT, ver O6) |
| `scanner.rs:2846-2847` `same_token_in_out` (comentario del encoder de simulación) | `:2847` en HEAD (`:2870` en working tree) | ✅ en HEAD |
| `orchestrator.rs:311-314` DECODED_INTENTS_TOTAL, `:317` resolve, `:383-425` spawn de cartuchos | Exacto | ✅ |
| `canonical_knobs.rs:54/152/251-253/381` knob liquidez | Exacto | ✅ |
| `metrics.rs:151-162` REJECTED_CONFIG_TOTAL, `publisher.rs:9-28` XADD MAXLEN ~10_000, `patterns.rs:29-50`, `reserves.rs:28-39`, `dedup.rs:96-127`, `chain_supervisor.rs:290-322` | Exacto | ✅ |

**Orden de inyección exigido por el charter — verificado:** I2 se inyecta en ~`:979` (tras el semáforo), ANTES de `list_cartridges()` `:984`, ANTES de TODAS las lecturas Redis (`:1054` identity, `:1065` prices) y ANTES del loop de cartuchos `:1085` ✅. I3 en ~`:1610`, ANTES de `trading_config` `:1617` y del enriquecimiento `:1630-1642` ✅. I1 tras el contador `:311-314`, ANTES de `resolve` `:317` y del spawn `:383-425` (cubre modo Active **y** Shadow del spawn) ✅.

**Bypass claims verificados:** `Orchestrator::spawn_cartridge_eval` (`orchestrator.rs:250-279`, Active-gated) llama directo a `active_evaluate_and_emit` (`:266`) y es invocado por `workers/route_scanner_worker.rs:550` y `route_discovery/route_discovery_worker.rs:1531` sin pasar por `on_route_intent` → **I2 es requerido y suficiente**. El path shadow de los workers (`shadow_evaluate_intent`, que también emite RDO outcomes vía `cartridge_boot.rs:899`) solo recibe ciclos de grafo — que no pueden ser self-pair (§1.3) → sin gap.

---

## 3. RULE 00 — VERIFICADA (charter ítem 2)

- El código propuesto de producción (`flood_gate.rs` §3.1 + diffs I1/I2/I3/I4/I4b/I5) contiene **cero addresses y cero símbolos de token**: los predicados son geometría de legs (Address ==), (chain, reason-class, par) + ventana, y un knob numérico. Los 5 addresses (AGLD/XEN/PEPE/WETH/USDC) viven SOLO en el `mod fixtures` bajo `#[cfg(test)]` (§4), y §7.3 añade el grep de gate del PR (`0x3235|0x0645|0x6982|agld|xen|pepe` → hits solo en cfg(test)).
- Los 3 addresses del flood coinciden con los contratos canónicos mainnet conocidos (AGLD `0x3235…9a20`, XEN `0x0645…6fb8`, PEPE `0x6982…1933`; también WETH/USDC del fixture). El diseño además difiere la re-verificación contra PG al PR (§4 SQL) — higiene correcta.
- El criterio es dinámico: la rotación AGLD→XEN→PEPE demostrada en la evidencia no afecta ninguno de los tres predicados.

---

## 4. R8/R9/R7 — suficiente EN MECANISMO, con 2 gaps declarables (charter ítem 3)

**Lo que sostiene al diseño:**
- El counter `arbx_flood_suppressed_total` usa el mismo `REGISTRY` que se sirve por **`/metrics`** (`metrics.rs:25-26` "the same Prometheus registry served by the `/metrics` HTTP endpoint"; `main.rs:24` "Serve `/health` + `/metrics`") → el "métrico = ledger" es auditable en vivo, no solo in-process.
- F2 persiste **1 fila muestra por ventana** — la evidencia PG sobrevive al restart.
- El argumento contra la doctrina `scanner.rs:2179-2183` es matemáticamente sólido: `TokenNotAllowed` exige acción del operador (iterar allowlist), pero un self-pair sería rechazado por **cualquier** allowlist (no hay acción posible que lo vuelva tradeable) → la fila es costo puro. Engage directo con la doctrina, no una elisión.

**Los 2 gaps (M2, M3 abajo):** el diseño NO declara que la supresión en I4 ocurre ANTES de `score_and_publish` (`:401`) y del counter `OPPORTUNITIES_TOTAL{outcome="rejected_*"}` (`:418-420`), y el "summary por ventana a info!" declarado en §1 no tiene diff en §3.

---

## 5. Knob `min_pool_liquidity_usd` (charter ítem 5) — cableado, con alcance matizado

- Grep propio confirma el diagnóstico: `min_pool_liquidity` aparece SOLO en `canonical_knobs.rs` (`:27/54/152/251-253/381/497/763` — declaración, default, env, validación, serialización, test). **Cero call-sites de enforcement.** El diseño lo cablea (F3/I5/I6) — respuesta al charter: sí lo cablea.
- **Matiz de alcance que el diseño sub-expresa (M3):** `reserves_source` depende de `pool_hint` (`cartridge_boot.rs:1038`), y el decoder de mempool lo deja `None` **siempre** por R8 (`route_decoder.rs:261` "never fabricate a pool address from calldata"). `pool_hint: Some` existe solo en el path de grafos (`route_intent_dispatcher.rs:80`, `workers/route_scanner_worker.rs:365`) y block_scanner (`:394`). Consecuencia: **F3 es passthrough `tier_unknown` para ~todo el flood de mempool** (F1 hace ese trabajo) y tiene dientes reales en el path de discovery — que es donde vive el eco RDO de 30M filas/día. Coherente con D-2, pero la redacción de §2 ("supresión pre-matriz + métrico") sugiere más palanca anti-flood-mempool de la que habrá.

---

## 6. Invariante XLEN / regresión (charter ítem 6)

- **F1:** no-op salvo legs X→X (§1.3). **F2:** primera ocurrencia por ventana se emite idéntica; `emit_accepted` y la fila no-config (`scanner.rs:2004-2038`, `rejection_reason = None`) quedan fuera del gate — protección correcta del invariante. **F3:** solo rechaza con cifra computada < knob; incomputable pasa (T9).
- **`EmitOutcome::Suppressed` es aditivo — verificado, no solo asumido:** los 15 callsites de `emit_rejected` (8 en cartridge_boot incl. `:1368` que el diseño no lista — ver O1; 7 en orchestrator `:1134/:1167/:1254/:1281/:1309/:1347/:1390`) usan `.await?` sin match de variantes. El único `match` sobre `EmitOutcome` (`orchestrator.rs:1416-1441`) es sobre `emit_accepted` y tiene brazo `_ =>` catch-all ("any future variants") → compila sin tocar callers.
- **F3 es filtro de POLÍTICA, no no-op matemático** (ver O5): puede suprimir una emisión que habría sido aceptada sobre un pool bajo el piso — eso es el propósito declarado del knob del operador, pero el framing "cero pérdida de oportunidades es demostrable" (§5) es demostrable solo para F1/F2; para F3 es verdadero **por definición** (bajo-piso = fuera de política), no por teorema. El runbook §5.1 debe exigir que el par de control esté también sobre el piso.
- Rollback limpio declarado (§7.8: gate in-memory, sin schema ni streams nuevos). El invariante §33.3 queda protegido para tráfico legítimo según la definición de §5.

---

## 7. Hallazgos (orden de severidad)

### MEDIUM — corregir al implementar

**M1 — Error aritmético en el spec del test T9 (§4).** `pool_liquidity_usd(100e18, 18, Some(2000.0), 200e6, 6, Some(1.0))` = 100 × 2000 + 200 × 1 = **Some(200_200.0)**, no `Some(2200.0)`; y 200.200 > 150.000 → NO demuestra `liquidity_below_floor` (la segunda afirmación de T9 también falla). Corrección: o el fixture es `1e18` (1 WETH → 2000 + 200 = 2200 < 150.000 ✓) o el esperado es `Some(200_200.0)` con el test invertido. El kernel es correcto; el spec del test es el que está mal. Riesgo si se copia verbatim: test rojo de arranque, o peor, un implementador "arregla" el kernel para satisfacer el número.

**M2 — Efectos observables no declarados de la posición de I4.** El gate se inyecta en ~`:395`, lo que además de PG/XADD salta: (a) `score_and_publish` (`opportunity_emitter.rs:401`) — el feed de clase negativa del Gate C / `bayesian_priors` (docstring `:395-399`: "prior calibration needs rejected observations") pierde los duplicados suprimidos; (b) el counter `OPPORTUNITIES_TOTAL{outcome="rejected_*"}` (`:418-420`) deja de contarlos → **todo dashboard/alerta/paridad que lea esa familia ve −97%** sin que el diseño lo declare (§5 declara los deltas de PG/XADD, no de este métrico ni del stream `arbx:scoring:scored`; el panel reject-breakdown y los paneles A.6/A.9 leen volumen de rechazo de PG). El volumen exacto sigue recuperable como `OPPORTUNITIES_TOTAL(rejected) + arbx_flood_suppressed_total{rejected_window_dedup}` — pero esa composición debe declararse o el delta parecerá pérdida.

**M3 — Traza durable de F1 y summary sin diff.** (a) `arbx_flood_suppressed_total` es counter Prometheus de vida-del-proceso: se expone en `/metrics` pero **resetea en restart** y el diseño no declara quién lo scrapea/persiste; F1 produce **cero filas PG, para siempre** — la única traza es el métrico y `debug!` por ítem (que bajo flood rota fuera de la ventana de 50 MB en segundos — la lección R9/LOGFLOOD-01 que el propio diseño cita). (b) El "summary agregado por ventana a `info!`" que §1 declara como parte del mecanismo R9 **no tiene diff en §3** (I1/I2/I3 solo tienen `debug!` + counter). Correcciones: implementar el summary periódico declarado, y/o sumar `flood_suppressed` a `ScannerCounters` (counters.rs) para que aterrice en el heartbeat (read+reset 60s), que es el canal durable existente.

### MINOR / observaciones

**O1 — Enumeración incompleta de callsites.** cartridge_boot tiene **8** sitios `emit_rejected` (`:1368, :1463, :1588, :1603, :1637, :1656, :1664, :1674`); el diseño lista 7 y omite `:1368` (rechazo `unmapped_strategy_label`, pre-SizeOptimizer). Cobertura intacta — I4 gatea DENTRO del emisor — pero la enumeración del §2/I4 debe corregirse.

**O2 — Claim §6 de R9 impreciso en el punto de inyección.** "I2 retorna antes del `info!` de entrada (`:959-964`)" es falso tal como está: I2 va tras el semáforo (`:967-978`), que es DESPUÉS del `info!` `:959`. La línea enter sigue disparando por spawn directo de los workers. El ruido muere en la práctica vía I1 (entrada del orchestrator) para el share mempool; corregir el claim o mover el check F1 al tope de la función (antes de `:959`).

**O3 — `F1 I2` y el info! `active_eval_actives`/`pertinent`:** ídem O2 — con el check en `:979`, los `info!` de `:999`/`:1017` sí quedan antes del return de F1… no: están después de `:984`, o sea después del check → también mueren. Solo `:959-964` queda vivo. (Precisión para el PR.)

**O4 — Nits de contexto de diffs.** (a) §3.2: `lib.rs` real tiene `pub mod fe_normalization;` en `:50` (sin `.rs`) y `pub mod financing;` en `:53` — no adyacentes (2 líneas de comentario entre); el contexto del diff no calza literal. (b) §2 I2: los paths reales son `src/workers/route_scanner_worker.rs` y `src/route_discovery/route_discovery_worker.rs`, no los planos citados. (c) §3.1 cita "patrón `counters()`" como precedente del global de proceso — `counters.rs:318+` marca esa API como legacy deprecada ("New code MUST use chain_counters"); el FloodGate no la usa (OnceLock propio, chain_id dentro de la key — sin mezcla de cadenas), pero el precedente citado es la forma deprecada.

**O5 — Framing de §5 para F3.** "Cero pérdida de oportunidades es demostrable" agrupa F3 con F1/F2; F3 es política de operador (knob), no teorema. El runbook (§5.1) debe definir el par de control como "sobre el piso" explícitamente.

**O6 — Drift de anclas vs working tree (informativo).** `scanner.rs` del working tree lleva +89/−4 líneas sin commit (bloque WO-02 hot-path, `:2623-2641` y otros hunks desde `:2384`): `same_token_in_out` está en `:2847` en HEAD pero `:2870` en el working tree. El diseño ancló a HEAD y lo declaró (§9) — correcto; el PR de WO-06 deberá re-anclar si WO-02 mergea primero. Los hunks ≥:2384 no afectan las anclas I3/§0.7 (todas < :2384).

---

## 8. Cumplimiento del verify (reglas duras)

- **RULE 00:** verificación sin datos fabricados; los 2 ítems no computables se declaran (no corrí cargo — charter 0 cargo; no hay métrica del dominio que verificar — 0/5 HTTP).
- **§32/§33:** read-only total. Sin VPS, sin executor/wallets/broadcast, sin mutaciones.
- **§34.3:** el diseño no toca `relays-client`/`live_exec_policy`/default-deny (confirmado por lectura: ningún diff de §3 toca el terminus); este verify tampoco.
- **NO-GIT:** cero commit/push/PR. Este archivo es el único artefacto escrito (está en mi claim).
- **Lexicon:** Holonomic Loop Resolution / Variedad de Liquidez / Topological Yield / Decoherencia de Estado aplicados.

## 9. Veredicto final

**PASS — el diseño es matemáticamente correcto en su núcleo (F1 leg-level con tipo `Address`, F2 ventana, F3 fail-honest), las anclas calzan contra HEAD, RULE 00 está satisfecha, el knob queda cableado, y el invariante XLEN queda protegido para tráfico legítimo.** NO BLOCK. Condiciones para el PR (bloqueantes de merge, no de diseño): M1 (fijar T9), M2 (declarar los deltas de `OPPORTUNITIES_TOTAL`/scoring o mover el gate después del counter y declarar el scoring), M3 (implementar el summary/window declarado + considerar heartbeat). Menores O1–O6 para precisión del diff.

*Firmado: math-validator, Gang Omniscience, 2026-09-06.*
