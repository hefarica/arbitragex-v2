# MATH-VERIFY — FEE-TIER-AWARE-QUOTING (WO-06)
> Verificador independiente: agent-math (doctrina §12 — el autor nunca certifica su pieza).
> Objeto: diff SIN commit en `fix/sim-fund01b-slot-padding-20260917` (`git diff` + archivo nuevo
> `v3_fee_catalog.rs`). Especificación: `06-FEE-TIER-DESIGN.md`. Reporte builder: `07-FEE-TIER-BUILD.md`.
> Fecha: 2026-09-17. Modo: read-only sobre código/git/VPS. Sancho checkpoint post-flight: TIMEOUT
> (fail-open, run_236853607f2949158fdf86ad980730ab) — la verificación no depende de él.

## VEREDICTO FINAL: **PASS-CON-SALVEDADES**

El corazón matemático (vector T1/T7, resolución de tiers, suite) es correcto y quedó verificado
por FUENTES INDEPENDIENTES al builder (cast + 4byte + openchain + eth_call VIVO a mainnet).
Las 2 preguntas de ataque de Sancho se responden NEGATIVAS (no hay bug de orden de gates ni
γ V2 en hops V3). Las salvedades son hallazgos latentes PRE-EXISTENTES fuera del diff y
criterios de producción no verificables en local.

---

## 1. Vector T1/T7 — RE-VERIFICACIÓN INDEPENDIENTE: **CORRECTO**

### 1.1 Selector (tres fuentes independientes + keccak local)

| Fuente | Resultado |
|---|---|
| `cast sig "quoteExactInputSingle((address,address,uint256,uint24,uint160))"` (keccak local) | **0xc6a5026a** |
| 4byte.directory API (`?hex_signature=0xc6a5026a`) | única entrada: la firma TUPLA |
| openchain signature-database lookup | única entrada: la firma TUPLA |
| `cast sig "quoteExactInputSingle(address,address,uint256,uint24,uint160)"` (flat 5-arg) | 0x1296323f (NO es la usada) |

El encoder (`amm_math.rs:330-384`, `quoter_v2_function` + `encode_quote_calldata`) declara la
firma TUPLA y ethers computa el selector de ella → 0xc6a5026a. El QuoterV2 desplegado
(0x61fFE014bA17989E743c5F6cB21bF9697530B21e) expone exactamente esa variante.

### 1.2 Layout del calldata

Struct de SOLO tipos estáticos como argumento top-level → encoding INLINE (5 palabras, SIN
palabra de offset): `selector(4) || tokenIn[4..36] || tokenOut[36..68] || amountIn[68..100]
|| fee[100..132] || sqrtLimit[132..164]` = **164 bytes**. Verificado dos formas:
- El vector T7 esperado es **byte-idéntico** al generado por `cast calldata
  "quoteExactInputSingle((address,address,uint256,uint24,uint160))" "(WETH,USDC,1e18,500,0)"`
  (generador independiente del código Rust).
- El pin T1 (`v3_fee_catalog.rs:259-282`) ancla [100..132]=0x0bb8 para 3000, 0x05 para 5, len 164.
  Valores canónicos verificados: 3000=0x0bb8, 500=0x01f4, 1e18=0x0de0b6b3a7640000, WETH
  0xC02a...Cc2, USDC 0xA0b8...b48 — todos correctos.

### 1.3 Confirmación VIVA contra mainnet (el ancla definitiva)

`eth_call` read-only al QuoterV2 mainnet con el calldata de cast (= vector T7):

| Tier | amountOut (1 WETH → USDC) | Estado |
|---|---|---|
| 500 (0x01f4) | 0x90ad0662 = 2.427.959.394 (~2427.96 USDC) | **éxito** |
| 3000 (0x0bb8) | 0x909cd132 = 2.426.179.122 (~2426.18 USDC) | **éxito** |

Ambos tiers reales WETH/USDC responden; outputs distintos y plausibles (el tier 0.05% paga
ligeramente más que el 0.30%, dirección correcta). Un offset mal habría revertido o
devuelto basura en AMBOS. Nota de método: mis primeros intentos con hex tipeado a mano
revertían por MI typo (dígitos impares) — descartados; solo valen los de cast.

### 1.4 El episodio RED de T1 (builder)

El builder reporta que su supuesto inicial ([132..164] como palabra fee) FALLÓ contra el
encoder real y corrigió contra el encoding OBSERVADO. Correcto: [132..164] es
`sqrtPriceLimitX96`. El pin final ancla el layout correcto — este es el comportamiento
esperado de un pin test, no un defecto.

---

## 2. Semántica PIPS de `fee_bps` — barrido completo de consumidores

Convención REAL del campo (verificada por provenance): **V2 → bps (30), V3 → pips crudos
uint24 (100/500/3000/10000)**. La ruta pool_discovery RE-LEE el fee ON-CHAIN antes de
persistir (`pool_discovery.rs:496-508` + `read_pool_v3_fee`: "Resolve the immutable fee
ON-CHAIN before ANY writes... PG fee_tier and the legacy pool_index_v3/PoolRef fee_bps
fields are consumed as raw pips") — la escala V3 es pips CONSISTENTE en PG/Redis/PoolRef.

| Consumidor | Escala usada | Veredicto |
|---|---|---|
| `amm_math.rs:77-81` `v2_amount_out` (10.000−fee) | bps | **CORRECTO** — solo llamado con pools V2 (ver debajo) |
| `amm_math.rs:375-385` encoder QuoterV2 (`U256::from(req.fee_bps)`) | pips | **CORRECTO** (§1) |
| `state_projector.rs` project_v3_quote_checked → provider | pips (catálogo) | **CORRECTO** — es el fix WO-06 |
| `engines/dex_engine.rs:283-284` fee_a/fee_b | bps | **CORRECTO** — rama `a_is_v2 && b_is_v2` solamente |
| `engines/dex_engine.rs:515` `unwrap_or(30)` | bps | **CORRECTO** — rama else (V2/Curve/Balancer); V3 va por el proyector |
| `size_optimizer.rs:911-912` fee_a/fee_b `unwrap_or(30)` | bps | **CORRECTO** — kernel V2 (golden-section); rutas con V3 van al kernel V3 (:454) |
| `route_discovery/graph_builder.rs:361` (V2) ÷10.000 | bps | **CORRECTO** |
| `route_discovery/graph_builder.rs:396-399` (V3) ÷1.000.000 | pips | **CORRECTO** — comentario explícito "millionths... NOT the V2 basis-point convention" |
| `cartridge_boot.rs:234-256` pips→bps para cartuchos (raw/100) | convierte | **CORRECTO** (test :2402 clava 3000→30; V2 pasa como bps) |
| `cartridge/host_bindings.rs:870,881` `V3_DEFAULT_FEE_BPS = 3000` | pips | **CORRECTO en valor** (3000 pips al QuoterV2) — nombre engañoso (ver salvedad S2) |
| `calldata/univ3.rs:101,114` `path_fees_bps = fee_raw` | pips | **CORRECTO en valor** — comentario :100 dice "/100" pero NO convierte; naming/comment engañoso (S2) |
| `route_decoder.rs:257` → `RouteIntentLeg.fee_bps` | V2:bps / V3:pips | **CORRECTO** (dual-por-protocolo; tests :523/:533/:582-585 documentan raw pips) |
| `scanner.rs:777` / `impact_index.rs:671` (PG fee_tier) | pips | **CORRECTO** |
| `v3_fee_catalog.rs:186` `by_pool.insert(addr, info.fee_bps)` (V3PoolInfo) | pips | **CORRECTO** — el catálogo almacena pips |
| **`quote_anchor_runtime.rs:172`** `e.fee_bps.unwrap_or(0)/10_000` al invertir `log_weight` | bps uniforme | **INCORRECTO para V3** (pips÷10.000 = fee 100× mayor). Impacto acotado a la métrica observacional `liquidity_valued` (inversión de tasa errónea ~tier/100, p.ej. ~5% con tier 500). NO es gate, ni sizing, ni quoting. Pre-existente, FUERA del diff (S1) |
| `engines/triangular_atomic_engine.rs:73` γ=1−fee/10.000 | bps uniforme | Latente sin efecto: worker Phase 1.5 scaffold — nunca evalúa hops reales (`triangular_atomic_worker.rs:38-45` solo loguea ticks). Pre-existente (S1) |
| `engines/triangular_atomic_engine.rs` / `spanning_tree_engine.rs` / `flashloan_engine.rs` / `financing.rs` / `batch_quote.rs` | bps legítimos (financing/flashloan SÍ son bps reales) | **CORRECTOS** — son fees de protocolo en bps, no tiers V3 |

**Conclusión §2:** en el PATH DE QUOTING/SIZING (lo que WO-06 toca) no hay ningún consumidor
que divida pips entre 10.000. Los dos usos incorrectos/latentes son pre-existentes,
observacionales/scaffold, y quedan como salvedades registradas — ninguna puede fabricar una
oportunidad (RULE 00 intacta).

---

## 3. Preguntas de ataque de Sancho

### 3(a) ¿min_ev_usd=25 al probe fijo 1e18 ANTES del sizing variable? → **NO EXISTE TAL GATE**

- `min_ev_usd` vive SOLO en `canonical_knobs.rs` (:65, :183 default 25.0, :291 env
  ARBX_KNOB_MIN_EV_USD, :565 export JSON). **Cero consumidores ejecutables** en searcher-rs
  (grep exhaustivo): es un knob declarativo publicado a Redis `arbx:config:canonical_knobs`
  (`main.rs:369-388`) y servido por `api-server/routes/canonical-knobs.ts` al panel
  (`CanonicalKnobsPanel.tsx`) — pura observabilidad. Tampoco existe en relays-client ni
  api-server como gate.
- El probe 1e18 (`dex_engine.rs:228`) alimenta únicamente el estimado `gross_profit_usd` de
  METADATO del candidato. El comentario explícito `dex_engine.rs:319-324` ("we no longer
  pre-reject... emit honest data") y el flujo `orchestrator.rs:910-927` confirman que TODOS
  los candidatos emitidos llegan a `optimize_with_reason` **sin pre-filtro por gross del probe**.
- El sizing es VARIABLE en ambos kernels:
  - V2: golden-section sobre [1, min(cap_wei, reserve_in_a)] (`size_optimizer.rs:915-940`).
  - V3: bracket log-espaciado de 8 puntos sobre **[1, cap_wei]** (`size_optimizer.rs:1146`) —
    el punto mínimo es 1 wei, no 1e18; el 1e18 solo puede ser UN punto del bracket.
- Los `non_positive_profit` (2.844/15min) son por tanto rechazos honestos del optimizador
  (spread ≤ 0 en TODO el bracket, o upper-bound within-tick ≤ 0 en :1155-1166), no un artefacto
  de EV-floor sobre el probe. La línea de tiempo del diseño (`size_optimizer.rs:119/:3136`)
  no corresponde al árbol actual — la premisa de la salvedad NO se sostiene en el código.
- NOTA menor honesta: en el paper/edge puede existir un umbral distinto, pero en el binario
  auditado no hay ningún gate EV previo al sizing.

### 3(b) ¿spot_product usa fee V2 (γ=0.997) en ciclos con hops V3 → falsos aceptes? → **NO**

- El gate que emite `spot_product_le_one` es la ruta de ciclos MVP **V2 puros**:
  `triangular_engine.rs:423/435` y `triangular_worker.rs:760` computan
  `spot_product(reservas_V2, V2_FEE_BPS=30)` sobre reservas V2 del cache — γ=0.997 es
  EXACTO ahí (fee V2 canónico).
- Los ciclos con hops V3 NO pasan por spot_product con γ aproximada: van por
  `scan_v3_bearing_cycles` → `try_progress_plan` (`triangular_worker.rs:1001-1055`), donde
  cada hop V3 se cotiza con el **QuoterV2 real** (`V3QuoteRequest` multicall) — el fee tier
  lo aplica la pool on-chain, sin aproximación γ. El hop WETH↔USDC intermedio siempre es
  V2-resoluble (:1847-1849, documentado).
- Análisis de dirección (aunque la γ aproximada no aplica a V3): para TODOS los tiers
  estándar (100/500/3000 → γ_true 0.9999/0.9995/0.9970), γ_code=0.997 ≤ γ_true → S
  SUBestimado → conservador (falsos RECHAZOS, jamás falsos aceptes). Solo el tier 10000
  (γ=0.99) invertiría la dirección, y ningún camino del código aplica γ=0.997 a un hop V3.
- El early-reject within-tick del size_optimizer usa `v3_amount_out_single_tick(..., fee_pips,
  ...)` (pips correctos, `amm_math.rs:134-144` con guard `fee_pips >= 1_000_000`) y su
  resultado solo puede RECHAZAR; el profit final siempre es QuoterV2 real → no puede crear
  falsos aceptes en ninguna dirección.

---

## 4. Suite — conteo exacto re-ejecutado

```
cargo test -p searcher-rs --lib
→ test result: ok. 1274 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out
```
Coincide EXACTAMENTE con el reporte del builder (1274/0/3; los 3 ignored son hydrate_*
que requieren Redis vivo). Además re-ejecutado de forma independiente:
- `cargo fmt -p searcher-rs -- --check` → **exit 0**
- `cargo clippy -p searcher-rs -- -D warnings` → **exit 0** (lib + bin)

Tests T1-T7 presentes y semánticamente conformes a la especificación (T2 ancla
None→Catalog(3000) y Some(100)→Mismatch cotizando CON 3000; T3/T5 con mocks que
PANIQUEAN si el provider es invocado = cero RPC demostrado; T6 ancla labels exactos;
T4 captura [1,5] exactos y nunca pide 100). El trait `V3QuoteProvider` quedó intacto
(ningún hunk del diff toca state_projector.rs:69-81).

## 5. Verificación de diseño vs implementación (diff)

- `unwrap_or(500)` ELIMINADO (diff state_projector.rs:329 → reemplazado por
  `fee_catalog.resolve`). Orden de gates conforme al diseño: zero-amount → provider None →
  resolve (Catalog/Mismatch-catalog-wins/NotCatalogued→pair-then-pool, ambos cero RPC) →
  provider → record_observed post-OK.
- Labels/métricas/métricas-sembradas/gauge/timer-60s/boot-guard-30s/rpc_tier_revert:
  todos presentes como se reporta (scanner.rs, metrics.rs, v3_quote_provider.rs diffs).
- Las 4 desviaciones del builder son reales y justificadas (record_observed con tokens
  porque el wire Redis es symbol-keyed; LegQuote::Unavailable con label; sin refresh()
  separado; encode_quote_calldata pub(crate)).

## 6. SALVADEDADES (no bloquean el PASS)

- **S1 — `quote_anchor_runtime.rs:172` (pre-existente, fuera del diff):** invierte
  `log_weight` con `fee = fee_bps/10_000` uniforme; para edges V3 (pips) el fee queda 100×
  mayor → `rate` recuperada distorsionada ~tier/100 (p.ej. +5.2% con tier 500). Impacto
  acotado a la heurística observacional `liquidity_valued` (anclaje de quotes); no es gate
  ni sizing ni quoting. Recomendación para un WO futuro: bifurcar por protocolo como hace
  `graph_builder.rs:361/:398`.
- **S2 — naming/comentarios engañosos (pre-existentes):** `V3_DEFAULT_FEE_BPS = 3000`
  (host_bindings.rs:870) son pips, no bps; `calldata/univ3.rs:100` dice "convert to basis
  points (/100)" pero el código guarda raw. El campo `fee_bps` en V3 es pips — convención
  documentada en pool_discovery.rs:496-498 pero el nombre invita al error que este WO
  precisamente previno.
- **S3 — triángulo atomic engine /10.000 (pre-existente):** γ a escala bps con hop.fee_bps;
  hoy inocuo (worker Phase 1.5 scaffold que nunca evalúa), pero si Phase 2 lo cablea con
  pips heredaría γ 100× menor (conservador, falsos rechazos).
- **S4 — criterios de aceptación 2-5 del diseño son post-deploy** (caída >90% de
  v3_quote_unavailable, fee_resolution como árbitro de rama, accepts>0, cache_neg_hit<10%):
  no verificables en local, como el propio builder declaró.
- **S5 — arranque en frío de by_pair:** el wire Redis es symbol-keyed y no trae direcciones
  de token (desviación 1 del builder), así que `by_pair` nace vacío y se puebla por
  observación; hasta entonces todo pool no-catalogado cae en `pair_no_pools` (no en
  `not_catalogued`). Honestos ambos y cero-RPC, pero hay que leer la métrica
  `fee_resolution` con eso en mente durante los primeros minutos post-deploy.
- **S6 — catálogo-gana en Mismatch:** si el índice Redis tuviera un fee STALE para un pool
  real, se cotiza al tier del catálogo y puede revertir (`rpc_tier_revert` canario) en vez
  de cotizar al tier ofrecido correcto. Trade-off deliberado del diseño (una sola fuente de
  verdad) y observable por métrica; `record_observed` reconcilia tras el primer éxito.

## 7. Resumen por punto del charter

| Punto | Veredicto | Evidencia clave |
|---|---|---|
| 1. T1/T7 selector+layout | **VERIFICADO CORRECTO** | cast sig/calldata byte-idéntico + 4byte + openchain + eth_call VIVO mainnet OK en tiers 500 y 3000 |
| 2. PIPS vs bps en fee_bps | **CORRECTO en todo el path de quoting**; 2 usos pre-existentes incorrectos/latentes fuera del diff (S1/S3) | tabla §2 con file:line y veredicto |
| 3(a) min_ev_usd antes de sizing | **NO EXISTE el gate**; probe = metadato; sizing variable | canonical_knobs sin consumidores; orchestrator.rs:910; size_optimizer.rs:915-940/:1146 |
| 3(b) spot_product γ V2 en hops V3 | **NO**; V3 se cotiza on-chain real; aproximación hipotética sería conservadora | triangular_worker.rs:1001-1055/:1847; §3(b) |
| 4. Suite | **1274 passed; 0 failed; 3 ignored** (re-ejecutado) + fmt/clippy exit 0 | §4 |
| 5. Veredicto | **PASS-CON-SALVEDADES** (S1-S6) | — |
