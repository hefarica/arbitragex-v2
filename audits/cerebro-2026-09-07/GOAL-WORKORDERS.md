# /goal — EL CEREBRO (orden del operador 2026-09-07): "el ferrari con motor"

> "Ahora es el momento que el gang trabaje en función del cerebro que hará que esto tenga vida,
> el ferrari con motor, bien implementado y perfectamente en funcionamiento para que este sea
> mejor que nada en arbitrage." — Operador
>
> **Ambición**: el pipeline matemático de detección→evaluación→sizing→simulación→aprendizaje en
> condición de clase mundial, VERIFICADO contra la doctrina (ROUTES_CROWN_JEWEL: fees on-chain,
> sizing convexo CFMM, financiamiento como dimensión de ruta, fail-honest R8) y medido con datos
> vivos. La rentabilidad la dicta el mercado — la telemetría mostrará la verdad sin maquillaje.

## Evidencia forense acumulada (insumos del gang)

- **El embudo muere temprano**: 59,140 opps/24h TODAS legs=1; 0 multihop en 7 días; 100%
  rejected (XEN+AGLD = 78% del flujo; gas_floor_breach 101K/7d; TokenNotAllowed #1).
- **missing_reserves = 79% del sink multihop** (196,927/h): 114/235 pools con reserves
  cacheadas, TTL ~30s, carrera por legs-frescas-simultáneas.
- **RU-3 ya ON** (500 ciclos/bloque, 272-380 despachados) + **PR #555** (HOPS-LIVE-01: top-25
  ciclos/bloque al pipeline canónico) en CI — el fix que conecta el descubrimiento al feed.
- **Calibración bayesiana inerte** (WO-07 diseñado+verificado, flip gated): 31 operadores
  puntúan sin señal aprendida.
- **Latencia p95 FAIL** en el propio panel (764ms vs target 29ms) — ahora medible con las
  series WO-10 vivas.
- **SIM_BACKEND** decisión WO-12 pendiente (revm in-process Tier 1/2 vs anvil fork).
- **Doctrina**: docs/ROUTES_CROWN_JEWEL_DOCTRINE.md · skills/arbitragex-ultra/world/
  (graph-algorithms, quant-math, mev-practice) · los 31 operadores · SizeOptimizer ES el motor
  económico (memoria del proyecto).

## Kanban

| WO | Ítem | Acción | Estado |
|---|---|---|---|
| **BR-00 (P0 ABSOLUTO)** | **D-SIM-01 — Desajuste detector-simulador**: 98.2% de rechazos = `strategy_not_simulatable_in_s4` — el detector encuentra oportunidades que simulator-v2 (REVM 42) NO sabe simular (hipótesis: los cartuchos mev_01_*/mev_04_* emiten topologías sin handler). Con gas a 0.087 gwei (−99.9%) el mercado está IDEAL — el 0% de aprobación es falla del sistema, no del mercado. **Sin esto todo lo demás es irrelevante** | (a) auditar cobertura de strategy_kind en el simulador vs las kinds que emite el detector (censo real de PG 24h: kinds emitidas × kinds simulables); (b) mapear mev_01_*/mev_04_* → handlers de simulación; (c) implementar handlers faltantes O filtro honesto en el detector (RULE 00); (d) **métrica de éxito: 98.2% → <50%** | **APPLY ATERIZADO 2026-09-08** (BR-00-APPLY.md, ecc:rust-reviewer): gate `!= dex_arb()` ELIMINADO — admisión por ESTRUCTURA de ruta (directiva operador: stems jamás colapsados, kind fluye end-to-end sin mutación). (a) dex_arb + todo stem con ruta abierta admitidos; (b) cíclicos (triangular/flashloan/cartridges cerrados) → `strategy_cyclic_route_not_simulatable_in_s4:<kind>` POR STEM; liquidation → `not_simulatable:<kind>`. **BONUS: decoder V2 decodificaba amountOut=32 (offset) — 100% del flujo V2 simulado computaba slippage contra basura — CORREGIDO array-primero** + `output_undecodable` honesto. Caza stem-stomping: **0 sitios**. Verify: fmt/check/clippy limpios, 66/0/2 tests (+8 BR-00). Files: sim-ctl/{tx_builder,sim_engine,persistence}.rs. **RE-VERIFY = PASS-WITH-ADVISORIES 2026-09-08** (BR-00-REVERIFY.md, ecc:security-reviewer): batería re-ejecutada idéntica (66/0/2, clippy 0, fmt 0), gate fail-closed confirmado (passed exige eth_call+gas+decode+umbral), sufijo `:<kind>` rompe 0 consumidores, stem-stomping 0, §34.3/§34.1 intactos; decoder además cerraba passed-fabricado con amount_in≤32 wei. 4 advisories: PR rebanado SOLO sim-ctl (P-∅) · query cobertura por-kind post-deploy · protocolo §1.4 completo post-deploy · probe certifica hop directo no topología multihop. **✅ LANDED END-TO-END 2026-09-09** — PR **#556** (branch fix/br00-sim-structural-gate, 3 archivos +231/−29, worktree aislado): CI **30/30** verde (estable 2-polls) → squash merge **3be8274d** → deploy veraz VPS (gates G4 KNOWN_GOOD_REVISION anclado; incidente disco 100% ENOSPC mató build#1 → 21.13GB builder-prune → relanzado nohup; imagen fb457020f5ce) → **L4 PASS**: contenedor healthy, VPS HEAD 3be8274d, 2,911 sims/25min persistidas. **MÉTRICA P0 §4 (era post-deploy 04:10Z+): 19.34% < 50% GATE PASS** (98.2%→19.34%): reverted REAL 57.57% (anvil) · strategy_cyclic_route_not_simulatable_in_s4 19.34% (not_implemented, POR-STEM: :triangular 457, :mev_01_032 9, :mev_01_031 9, :mev_02_017 8, :mev_02_003 6...) · build_error 13.40% · rpc_error 8.07% · sim_timeout 1.61%; label viejo desnudo = **0 emisión** ✅ directiva stems cumplida; XLEN arbx:opps:simulated=0 consistente con 0 passed (fail-honest). Browser-verify dominio vivo (contexto aislado): NO_GO/2 PENDING/$0 capital/Flip blocked §34.3, gates renderizados, feed honesto. **Colateral honesto post-fix**: G-PIPE-1 BLOCKED "consumer stalled 500 behind" — el consumer ahora SIMULA DE VERDAD (probes anvil reales vs morir gratis antes) → capacidad finita; remedios = BR-07 (revm in-process) + BR-08 (latencia). Observación menor: persist_err ~2/min (FK-vs-purge preexistente, no bloquea; forense BR-01). BR-02 des-gated |

### Análisis del operador que origina el P0 (2026-09-07, insumo verificado)

- **Rechazos**: strategy_not_simulatable_in_s4 98.2% (24h) · gas_floor_breach 101,221/7d · TokenNotAllowed 206,845 (~23%) · missing_reserves 41,862 · v3_quote_unavailable 40,556
- **Mercado FAVORABLE**: gas 0.087 gwei (−99.9% vs 2024) · ETH $2,489 · MEV ~$393M/año · builder Titan 53%
- **Proyección**: break-even 5% aprobación (~$1,170/mes con $2.3M capital) · 10% ≈ $4,650/mes · 20% ≈ $13,950/mes — **gap crítico >10%; actual 0%**
- Camino corregido: Fase 0 = BR-00 → Fase 1 = calibración (BR-05, post-sim-viable ≥5%) → Fase 2 = A.9 sign-off (procedimental, RUNBOOK-A6A9-01) → Fase 3 = crucible 72h
- Drifts confirmados adicionales: D-CON-01 (235-285 huérfanos — YA en PR #555/deploy) · D-TRIM-01 (488-547 recortadas sin consumir — WO-15 runbook) · D-REL-01 (relay catalog 0 filas) · D-VAULT-01 (Vault sealed no cableado — AppRole P2)

| BR-01 | **Forense del embudo completo stage-by-stage** con datos vivos (24h): por cada compuerta (decode→graph→engine→size→oracle→gas→emit) su cuota exacta de muerte con números reales; el mapa "dónde se pierde el Topological Yield" — el cerebro no puede optimizar lo que no ve | Análisis PG/Redis/logs read-only + panel de verdad | PENDIENTE |
| BR-02 | **Matar la carrera de reserves** (el asesino #1): cobertura 114/235 → universo completo fresco (pool_sync knobs/watchlist); TTL/backfill coherentes con el tick de evaluación; el grafo y la evaluación ven LA MISMA realidad | Diseño + apply + verificación con datos | PENDIENTE |
| BR-03 | **Cascada de oráculos**: cobertura de precios del universo (unknown_token_price, missing meta) — sin precio no hay gas-floor ni sizing honestos; leer de la cadena lo que la cadena dice | Diseño + apply | PENDIENTE |
| BR-04 | **Anti-spam tiering** (WO-06 diseñado+verificado): XEN/AGLD comen el 78% del presupuesto del cerebro; filtro dinámico por liquidez/risk-screen (RULE 00: cero hardcode de tokens) | Apply del diseño WO-06 con sus 3 MEDIUMs corregidos | PENDIENTE |
| BR-05 | **Activar la calibración** (WO-07): el cerebro que APRENDE — shrinkage Beta-Bernoulli κ=20 por operador; los 31 operadores dejan de puntuar a ciegas | Apply del diseño verificado + flip wiring (activación = operador si aplica) | PENDIENTE |
| BR-06 | **Verificación matemática del SizeOptimizer**: property tests del sizing convexo CFMM (fórmula cuadrática cerrada) contra fixtures on-chain verificados; la matemática ES el producto — demostrar que es exacta | Tests + evidencia | PENDIENTE |
| BR-07 | **SIM_BACKEND** (WO-12): aterrizar la decisión — revm in-process Tier 1/2 declarado + Anvil retenido Tier 3; la simulación en el hot-path sin dependencia de fork externo | Apply | PENDIENTE |
| BR-08 | **Latencia p95**: el panel declara FAIL (764ms vs 29ms); con las series `arbx_pipeline_latency_seconds` vivas, atacar el cuello real medido (no a ciegas) | Medir→optimizar→re-medir | PENDIENTE |
| BR-09 | **Prueba de vida golden-path**: UNA ruta rentable REAL atraviesa el cerebro completo end-to-end (graph→engine→size→sim→emit viable) y emerge en el feed — la prueba de que el motor tiene vida | E2E con evidencia browser | PENDIENTE |
| BR-10 | **Telemetría del cerebro en la UI**: los paneles muestran el embudo BR-01 con stages vivos, latencia por stage, calibración por operador — el operador VE el cerebro pensar | Frontend + browser-verify | PENDIENTE |
| **BR-11 (BUG operador)** | **"Cuando aparecen valores USD, los hops >2 legs DESAPARECEN"** + **mostrar el dinero configurado y por hop**: cada tarjeta multihop debe mostrar los valores USD configurados (capital, amount_in), **cuánto gana CADA hop** y **cuánto entrega el hop final**. **SEED forense del orquestador (2026-09-07 13:05Z): la API live es INOCENTE** — `/api/opportunities/live?limit=100` devuelve triangular (30/100, viable_only:false, pool_addresses en todos) mientras PG corre 800 triangular/3min ⇒ el filtro que los desaparece vive en el FRONTEND cuando hay rows con USD (sospechas: sort que empuja unscored bajo el fold, filtro cliente por presencia de roi/usd, o interacción con el feed congelado D-11). Precedente #445 (viable_only ocultó 579K). Entregable: fix del disappearance + route_metadata enriquecido con amount_in/out POR LEG (dinero real de la sim) + X-Ray de tarjeta con waterfall hop-a-hop + entrega final del hop | Forense + backend enrichment + frontend X-Ray + browser-verify | **EN CURSO** — ampliación operador 13:1x: la tarjeta muestra TODO el dinero configurado (capital, amount_in) y el MODO DE FINANCIAMIENTO con flash loan en CUALQUIERA de sus presentaciones/nombres (TLS / flash-loan / AAVE_FL / BALANCER_FL / V2_FLASH_SWAP / OWN_CAPITAL — el lexicon no puede esconder la configuración) — todo se configura y se VE ahí |

## Reglas duras
- **ORDEN SAGRADO DE MODIFICACIÓN (orden del operador, invariante)**: TODA modificación a la DApp
  ocurre SIEMPRE en este orden: **(1) LOCAL** (edición + tests/tsc/cargo) → **(2) REPO** (commit →
  PR → CI verde → merge) → **(3) VPS** (deploy veraz con KNOWN_GOOD_REVISION + migraciones +
  build) → **(4) DOMINIO VIVO** (verificación en Chromium de que salió PERFECTO — cero saltos de
  capa, cero "asumo que llegó"). Nada salta capas; nada se declara hecho sin el paso 4.
- §34.3 INTOCABLE (terminus gated) · RULE 00 (cero mocks; None≠0) · R8 fail-honest en cada stage ·
  fees on-chain (nunca hardcode) · hot-path mode-invariant §34.1 · diffs `// BR-XX (2026-09-07)` ·
  NO-GIT (PRs del orquestador al final) · VPS read-only · serial-group "rust" para applies Rust ·
  cada WO con verify adversarial · browser-validation del orquestador al final.
- **DIRECTIVA DEL OPERADOR (2026-09-07): máximo 15 agentes PhD en este gang** (flota chica y
  confiable para cero errores). Roles con historial SIN errores en este programa (usar estos):
  ecc:rust-reviewer · ecc:typescript-reviewer · ecc:security-reviewer · ecc:database-reviewer ·
  ecc:performance-optimizer · math-validator · cs-validator. El composer debe componer ≤15
  especialistas cubriendo BR-00 primero (P0), luego BR-02..BR-09 por prioridad económica
  (BR-02 reserves → BR-03 oráculos → BR-04 anti-spam → BR-05 calibración post-sim-viable).
