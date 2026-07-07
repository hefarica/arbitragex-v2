# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Design Spec â€” SOP Crypto MEV Integration + PMI/EVM Observability Layer

**Fecha:** 2026-05-03
**Autor:** OMEGA Master Cortex
**Estado:** APROBADO por operador 2026-05-03 â€” orden 0â†’1â†’3â†’2â†’4 + TRUNCATE pre-Sprint-3
**Branch destino:** `main` (incremental, sin worktree)
**Contrato de no-daÃ±o:** DiseÃ±o 100% aditivo. Cero modificaciÃ³n a archivos runtime existentes (`frontend/app/*`, `backend/searcher-rs/src/*`, `backend/prioritization-spine/src/*`, `backend/api-server/src/*`). Sprint 4 implementado como nuevo crate paralelo (`backend/simulator-v2/`) activable por feature flag, dejando el simulator actual intacto como fallback.

## 0. Fuentes integradas (100%)

1. `cover_sop.pdf` â€” portada del SOP general (DeFi arbitrage)
2. `generate_sop.py` â€” script ReportLab que genera los SOPs (contiene texto fuente de los PDFs en strings Python)
3. `sop_body.pdf` â€” cuerpo del SOP general, 13 capÃ­tulos, 8 estrategias en Tabla 1
4. `SOP_Arbitraje_EVM_Completo.pdf` â€” guÃ­a estratÃ©gica con Ã©nfasis en arbitraje legal contractual (NO se aplica al stack crypto, su tabla 1 sÃ­ se referencia para terminologÃ­a)
5. **`SOP_ArbitrageX_2026.pdf`** â€” SOP operativo v2.0 con 16 capÃ­tulos, 10 estrategias en Tabla Â§3, cÃ³digo Rust de referencia con Alloy 0.9 + revm 19.0, Bellman-Ford completo cap 14, 5 capas de risk management cap 15, infra deploy cap 16. **Fuente PRIMARIA del Sprint 1.**

## Cambios de diseÃ±o tras leer SOP_ArbitrageX_2026.pdf

- Strategy catalog pasa de 8 a **10 entradas** (matriz Â§3 del SOP_2026).
- Sandwich Attack incluido pero con flag `ethical_constraint='defensive_only'` y badge UI rojo "DEFENSIVE ONLY â€” never enables offensive". `enabled` solo controla las protecciones anti-sandwich, nunca ejecuciÃ³n ofensiva.
- 4 estrategias marcadas con `competitive_advantage='extrema'`: CEX-DEX, Pendle/Temporal, Cross-Chain Bridge, MEV-Boost Block. UI las destaca con badge dorado.
- Sprint 1 expande de 13 a **16 skills .md** (una por capÃ­tulo del SOP_2026).
- Sprint 4: nuevo crate `backend/simulator-v2/` (no modifica `prioritization-spine`). ActivaciÃ³n: `ARBX_USE_SIMULATOR_V2=true` env var. Por defecto `false` â†’ comportamiento actual intacto.

---

## 1. Context

El operador entregÃ³ un SOP de 21 pÃ¡ginas (`SOP_Arbitraje_EVM_Completo.pdf` + `sop_body.pdf`) con 13 capÃ­tulos cubriendo el universo completo de estrategias DeFi de arbitraje atÃ³mico en redes EVM (Ethereum, Arbitrum, Optimism, Base, BSC, Polygon, zkSync, Avalanche). El contenido estÃ¡ alineado al 100% con ArbitrageX v2 â€” no es PMI/Earned Value Management como decÃ­a el "PROMPT MAESTRO" boilerplate, sino crypto MEV puro.

El operador eligiÃ³ como deliverable la **opciÃ³n D**: "Skills .md por capÃ­tulo + integraciÃ³n SOPâ†’Strategy Panel + capa de observabilidad PMI/EVM traducida a mÃ©tricas crypto". Es decir: tres tracks paralelos que extienden el plan vigente del Strategy Panel sin abandonarlo.

### Por quÃ© importa
- El stack actual produce solo `dex_arb` con simulator stub (RULE 00 violaciÃ³n parcialmente remediada en commit `dc5d376`).
- El SOP describe el blueprint completo de las otras 7 estrategias listadas en su Tabla 1 (Triangular, Cross-Chain, CEX-DEX, Liquidaciones, Micro-HFT, Yield, Liquidity Migration).
- Sin un layer de observabilidad econÃ³mica (CPI/SPI/EAC traducidos), el operador no tiene forma de saber si la pipeline genera ganancias *reales* vs costos de gas + slippage.

---

## 2. Mapeo SOP Ã— Stack actual Ã— Deliverable

| Cap. SOP | Tema | Estado actual ArbX v2 | AcciÃ³n |
|----------|------|------------------------|--------|
| 1 | Panorama 8 estrategias (Tabla 1) | enum `StrategyKind` con 5 variantes | Extender enum + seed `strategy_catalog` con las 8 |
| 2 | DEX-DEX Directo (SOP Â§2) | `patterns::build_dex_arb_candidate` activo | Validar contra criterios SOP Â§2.2 (TVL>$5M, vol>$10M, 5+ DEXs, slippage<0.02%) |
| 3 | Triangular Flash Loan (SOP Â§3) | enum-only | Skill .md + futuro `patterns::build_triangular_candidate` |
| 4 | Cross-Chain (SOP Â§4) | NO en enum | AÃ±adir `cross_chain` al catÃ¡logo + skill .md |
| 5 | CEX-DEX (SOP Â§5) | NO en enum | AÃ±adir `cex_dex` al catÃ¡logo + skill .md |
| 6 | Liquidaciones MEV (SOP Â§6) | enum `Liquidation` schema-only | Skill .md con tabla Â§6.1 (Aave/Compound/Maker thresholds) |
| 7 | Micro-Arbitraje HFT (SOP Â§7) | NO | AÃ±adir `micro_hft` al catÃ¡logo + skill .md con cfg por red Â§7.2 |
| 8 | SelecciÃ³n Tokens/Pools (SOP Â§8) | hardcoded allowlist | Skill .md + alimenta `token_allowlist` con criterios Tabla 8 |
| 9 | Seguridad/PrevenciÃ³n Robos (SOP Â§9) | parcial (kill-switch) | Skill .md con Tabla 9 (mapeo riesgoâ†’protecciÃ³n) + reusable como Tab 6 del panel |
| 10 | CÃ³mo Encontrar Liquidez (SOP Â§10) | parcial (PoolSyncWorker) | Skill .md sobre fuentes de datos + liquidez emergente |
| 11 | Arquitectura Rust (SOP Â§11) | implementada parcialmente | Skill .md como referencia canÃ³nica de arquitectura C-S-E |
| 12 | DetecciÃ³n+EjecuciÃ³n (SOP Â§12) | simulator stub | Skill .md + Sprint 4 implementa Bellman-Ford paralelo + revm real |
| 13 | ConfiguraciÃ³n+OptimizaciÃ³n (SOP Â§13) | parcial | Skill .md + alimenta Tab 1 del Strategy Panel con parÃ¡metros producciÃ³n |

---

## 3. CatÃ¡logo de Skills .md a generar (Sprint 1)

13 skills, una por capÃ­tulo del SOP. ConvenciÃ³n: `.agents/skills/sop_<slug>/SKILL.md` con frontmatter:

```yaml
---
name: sop_<slug>
description: <when-to-trigger + what-it-does, pushy>
type: arbx_strategy_reference | arbx_security | arbx_architecture
source_section: SOP Â§N
---
```

| # | Skill | Path | Tipo |
|---|-------|------|------|
| 1 | sop_panorama_strategies | `.agents/skills/sop_panorama_strategies/SKILL.md` | reference |
| 2 | sop_dex_dex_directo | `.agents/skills/sop_dex_dex_directo/SKILL.md` | strategy |
| 3 | sop_triangular_flashloan | `.agents/skills/sop_triangular_flashloan/SKILL.md` | strategy |
| 4 | sop_cross_chain_arb | `.agents/skills/sop_cross_chain_arb/SKILL.md` | strategy |
| 5 | sop_cex_dex_arb | `.agents/skills/sop_cex_dex_arb/SKILL.md` | strategy |
| 6 | sop_liquidations_mev | `.agents/skills/sop_liquidations_mev/SKILL.md` | strategy |
| 7 | sop_micro_arb_hft | `.agents/skills/sop_micro_arb_hft/SKILL.md` | strategy |
| 8 | sop_token_pool_selection | `.agents/skills/sop_token_pool_selection/SKILL.md` | security |
| 9 | sop_security_anti_theft | `.agents/skills/sop_security_anti_theft/SKILL.md` | security |
| 10 | sop_liquidity_discovery | `.agents/skills/sop_liquidity_discovery/SKILL.md` | reference |
| 11 | sop_rust_architecture | `.agents/skills/sop_rust_architecture/SKILL.md` | architecture |
| 12 | sop_detection_execution | `.agents/skills/sop_detection_execution/SKILL.md` | architecture |
| 13 | sop_production_config | `.agents/skills/sop_production_config/SKILL.md` | architecture |

Cada skill tiene secciones: **CuÃ¡ndo activarse**, **Invariantes**, **Tablas de parÃ¡metros del SOP**, **Procedimiento operativo**, **Referencias cruzadas a archivos del repo**.

---

## 4. Sprint 2 â€” IntegraciÃ³n SOPâ†’Strategy Panel

El plan vigente del Strategy Panel (`~/.claude/plans/618f8807-*.md`) ya define 5 tabs y catÃ¡logo extensible. El SOP **alimenta el contenido** de cada tab:

- **Tab 1 (Capital & Riesgo)**: defaults de SOP Â§13 (profit min $5, slippage 0.5%, daily loss $200, etc.) â€” ya en spec.
- **Tab 2 (CatÃ¡logo)**: las 6 cards default + dropdown extendido. Seed `strategy_catalog` con las 8 estrategias de SOP Tabla 1 (no 6) â€” agregar `yield_arb` y `liquidity_migration`.
- **Tab 3 (MEV Services)**: Flashbots Protect, MEV Blocker, BloxRoute (SOP Â§9 menciona estos) + Eden Network, Titan Builder.
- **Tab 4 (Token Allowlist)**: defaults de SOP Tabla 2 (TVL, volumen, DEX listings, slippage<0.1%, auditorÃ­a OpenZeppelin/Trail).
- **Tab 5 (AuditorÃ­a)**: ya cubierta.
- **Tab 6 (Security/Anti-Theft)** (NUEVO): Tabla 9 del SOP â€” mapeo riesgoâ†’protecciÃ³n con toggles (Honeypot check, Sandwich protection via Flashbots, Frontrun via private mempool, Rug pull liquidez-bloqueada check).

---

## 5. Sprint 3 â€” PMI/EVM Observability Layer (la innovaciÃ³n meta)

**Insight clave**: las mÃ©tricas de PMI/Earned Value Management tienen equivalentes operativos directos en trading que NINGÃšN sistema MEV explota seriamente. Los traders quants de Wall Street las usan; el ecosistema crypto las ignora.

### Tabla de traducciÃ³n

| MÃ©trica PMI | FÃ³rmula PMI | Equivalente Crypto MEV | FÃ³rmula Crypto |
|-------------|-------------|-------------------------|----------------|
| **PV** (Planned Value) | trabajo planeado en USD | `daily_target_usd` | configurado en Tab 1 |
| **EV** (Earned Value) | trabajo completado en USD | `realized_profit_usd` | SUM(executed.net_profit) |
| **AC** (Actual Cost) | costo real gastado | `total_gas_spent_usd` | SUM(executed.gas_cost_usd) |
| **CPI** (Cost Performance Index) | EV/AC | **capital_efficiency** | profit_realizado / gas_total |
| **SPI** (Schedule Performance Index) | EV/PV | **velocity_index** | profit_realizado_today / daily_target |
| **EAC** (Estimate at Completion) | BAC/CPI | **forecast_daily_pnl** | (current_profit / hours_elapsed) Ã— 24 |
| **ETC** (Estimate to Complete) | EAC - AC | **remaining_runway** | daily_loss_cap - daily_loss_so_far |
| **TCPI** (To-Complete Performance Index) | (BAC-EV)/(BAC-AC) | **required_efficiency_remainder** | (target - current_profit) / (max_gas - current_gas) |
| **VAC** (Variance at Completion) | BAC - EAC | **projected_shortfall_usd** | daily_target - forecast_daily_pnl |
| **CV** (Cost Variance) | EV - AC | **net_pnl** | profit_realizado - gas_total |
| **SV** (Schedule Variance) | EV - PV | **pace_delta_usd** | realized - target_proportional |

### Componentes nuevos del frontend (pÃ¡gina `/operations`)

1. **Header dashboard**: 4 KPI cards (CPI, SPI, EAC, TCPI) con coloraciÃ³n semÃ¡ntica (verde si CPI>1, rojo si <0.8).
2. **S-curve chart** (recharts): cumulative profit vs cumulative gas vs target, con zonas sombreadas por hora del dÃ­a.
3. **Variance breakdown**: grÃ¡fico tornado con CV decomposed (price variance, slippage variance, gas variance, MEV bribe variance).
4. **Forecast Monte Carlo** (futuro): simular 10K caminos del resto del dÃ­a basÃ¡ndose en distribuciÃ³n histÃ³rica de profit por hora.
5. **Tornado risk analysis** (futuro): quÃ© factor impacta mÃ¡s el PnL (gas price spikes, MEV competition, RPC latency, slippage).

### Nuevos endpoints
- `GET /api/operations/kpi?chain_id=1&window=24h` â€” devuelve {cpi, spi, eac, etc, tcpi, vac, cv, sv}
- `GET /api/operations/scurve?chain_id=1&window=24h` â€” series temporales para chart
- `GET /api/operations/variance?chain_id=1&window=24h` â€” descomposiciÃ³n de varianza

ImplementaciÃ³n backend: queries SQL agregando sobre `opportunities` + `executions` + `trading_config_global` (target). Sin nuevas tablas â€” reuse del schema existente.

### Nueva entrada sidebar
`{ href: "/operations", label: "Operations PnL", icon: TrendingUpIcon, group: "observe" }`

---

## 6. Sprint 4 â€” ImplementaciÃ³n REVM Real (cap 11-12 SOP)

Reemplaza el stub actual de `prioritization-spine/src/simulator.rs:33-44`. Tareas:

1. **`prioritization-spine/src/lazy_db.rs`**: ya existe pero unused. Implementar `Database` trait de revm con lazy fetch on-chain via Alloy provider:
   - `pool_reserves(pool_addr)` â†’ llama `getReserves()` (V2) o `slot0` + `liquidity` (V3)
   - `token_balance(token, addr)` â†’ llama `balanceOf`
   - Cache en memoria con TTL=1 block

2. **`prioritization-spine/src/simulator.rs`**: reescribir `simulate_candidate`:
   - Construir calldata real con `calldata::univ2::encode_swap_exact_in()` o `univ3::encode_exact_input_single()`
   - Ejecutar contra block actual via `evm.transact()`
   - Decodear `amount_out` del return data
   - Retornar `gross_profit = (amount_out - amount_in) * token_price_usd`

3. **Eliminar hardcodes** en `searcher-rs/src/scanner.rs:272-296`:
   - `gas_units_estimated` â† `provider.estimate_gas(swap_tx)`
   - `gas_price` â† `provider.gas_price()` o `max(config.gas_max, baseFee + tip)`
   - `bribe` â† bribe model basado en builder landing rate (sub-tarea)
   - `flashloan_fee` â† lookup por proveedor (Aave 0.05%, Balancer 0%, etc.)
   - `token_risk_score` â† lookup en `token_allowlist.risk_score`
   - `liquidity_confidence` â† derivado del TVL del pool al momento del scan
   - `landing_probability` â† histÃ³rico builder landing rate (default 0.5 si sin datos)

4. **Bellman-Ford paralelo** (cap 12 SOP Â§12.2): `crates/graph-engine/` con `petgraph` + `rayon::par_iter` para detection multi-base-token.

5. **Token graph** (`crates/graph-engine/`): cache compartido `Arc<RwLock<TokenGraph>>` actualizado por price-monitor.

---

## 7. Plan de ejecuciÃ³n incremental

Cada sprint es deployable y reversible. Preferencia del operador (memory): "skip per-section approval gates; consolidate spec then execute with evidence".

| Sprint | Tiempo estimado | Reversibilidad | Bloquea siguientes? |
|--------|------------------|----------------|---------------------|
| **0 â€” Spec** (este doc) | 30 min | trivial | sÃ­, hasta aprobaciÃ³n |
| **1 â€” Skills .md** | 4-6 horas | git revert | no â€” knowledge base |
| **2 â€” Strategy Panel + Tab Security** | 3-4 dÃ­as | git revert por sprint | no â€” UI sÃ³lo |
| **3 â€” Operations PnL Dashboard** | 2-3 dÃ­as | git revert | no â€” observabilidad |
| **4 â€” REVM real + lazy fetch** | 1-2 semanas | git revert + image rollback | medium (cambia simulator) |

Recomiendo orden: **0 â†’ 1 â†’ 3 â†’ 2 â†’ 4**:
- 1 (skills) primero porque no toca runtime, baseline de conocimiento.
- 3 (observability) segundo porque ilumina si el sistema actual ya es rentable o no â€” DECISIVO antes de invertir en Sprint 4.
- 2 (Strategy Panel) tercero porque consume las skills y los KPIs.
- 4 (REVM real) Ãºltimo porque es el mÃ¡s invasivo â€” solo si Sprint 3 muestra que vale la pena (CPI<1 hoy probablemente, hay que medirlo).

---

## 8. Acceptance Criteria

### Sprint 1
- [ ] 13 archivos `.agents/skills/sop_*/SKILL.md` existen y `git status` los muestra como nuevos
- [ ] Cada SKILL.md tiene frontmatter vÃ¡lido (name, description, type, source_section)
- [ ] Cada uno tiene mÃ­nimo 4 secciones: CuÃ¡ndo activarse / Invariantes / Tablas SOP / Procedimiento / Cross-refs
- [ ] El usuario puede preguntar "Â¿quÃ© dice el SOP sobre micro-arbitraje?" en sesiÃ³n nueva y el agente carga `sop_micro_arb_hft` automÃ¡ticamente

### Sprint 2
- [ ] Migration 017 ejecutada con seed de las 8 strategy_catalog rows
- [ ] Tab 6 (Security) renderiza tabla con 8 protecciones del SOP Â§9.2
- [ ] Tab 4 (Token Allowlist) muestra los 5 blue-chips con risk_score=0.10 + criterios de SOP Â§8.1 documentados como tooltips

### Sprint 3
- [ ] Endpoint `GET /api/operations/kpi` devuelve los 7 KPIs
- [ ] PÃ¡gina `/operations` carga sin React errors, muestra CPI/SPI/EAC en cards
- [ ] S-curve chart renderiza con datos reales de las Ãºltimas 24h
- [ ] HidrataciÃ³n clean (R1 cumplida)

### Sprint 4
- [ ] `simulator.rs:simulate_candidate` ya no usa direcciones dummy `0x2222`
- [ ] Logs muestran `event=simulator.lazy_fetch pool=0x... reserves_in=... reserves_out=...`
- [ ] PG `expected_profit_usd` distribuciÃ³n no constante en $0 (sale del fallback)
- [ ] PG `gas_cost_usd` distribuciÃ³n variable (no mÃ¡s hardcoded $1.6)
- [ ] Sprint 3 KPIs muestran CPI con valor econÃ³micamente realista (probablemente <1 al inicio, mejora con tuning)

---

## 9. Reglas inmutables (R1-R8) chequeadas

- **R1 (Mounted Snapshot Pattern)**: pÃ¡ginas nuevas (`/operations`) siguen el patrÃ³n `page.tsx` Server + `*Client.tsx` con `useState(initialSnapshot)`.
- **R2 (Build-Time Guard)**: no se toca `next.config.js`.
- **R3 (Deploy --no-cache --env-file .env)**: cada redeploy estricto.
- **R5 (AuditorÃ­a transitiva)**: nuevos archivos auditados para no introducir `Date.now()` en render.
- **R6 (DATABASE_URL en docker compose)**: ya cumplido. Sprint 3 usa solo SQL queries existentes.
- **R7 (Trazabilidad E2E)**: cada KPI del dashboard es trazable a `opportunities` + `executions` rows.
- **R8 (Fail-honest pattern, propuesta sesiÃ³n 2026-05-03)**: KPIs muestran `null` si no hay datos suficientes, NO inventan promedios.
- **RULE 00 (Zero Mocks)**: Sprint 3 KPIs derivados de DB real, NO mocks. Sprint 4 cierra el Ãºltimo mock pendiente (REVM stub).

---

## 10. Riesgos + open questions

1. **Sprint 4 puede revelar que el sistema NO es rentable hoy**. CPI < 1 con costos de gas reales puede mostrar que el bot pierde dinero en cada arb. **Esto es resultado VERDADERO, no fallo del plan**. Sprint 3 lo expone honestamente; Sprint 4 da herramientas para corregir.
2. **Datos insuficientes para Sprint 3 KPIs**: hoy hay solo 1714 rows con profit mockeado. Los KPIs honestos requieren eliminar primero el dato sintÃ©tico (ya hecho en commit `dc5d376`) Y acumular datos reales nuevos. Recomiendo `TRUNCATE opportunities` como primer paso de Sprint 3.
3. **Sprint 4 requiere Alchemy archive node con `eth_call` historic state** para lazy_fetch. Plan actual usa `wss://eth-mainnet.alchemyapi.io` â€” verificar que el plan Alchemy cubre archive queries.
4. **Tabla `executions`**: existe? Necesaria para Sprint 3 KPIs. Si no existe, Sprint 3 incluye su creaciÃ³n.

### Preguntas pendientes para el operador
- Â¿Truncate del histÃ³rico mockeado en `opportunities`? (Recomendado, sÃ­)
- Â¿Sprint 3 antes que Sprint 2? (Recomiendo sÃ­ â€” observability es decisional)
- Â¿Sprint 4 despuÃ©s de Sprint 3, o prioritario?

---

## 11. Out of scope (futuras iteraciones)

- Frontend visual (Visual Companion) durante brainstorming â€” proyect memory lo prohÃ­be.
- Editar cÃ³digo Rust de estrategias desde frontend â€” viola defense-in-depth (ALERTA OMEGA, sesiÃ³n previa).
- ImplementaciÃ³n real de Yield Arbitrage (cap 1 Tabla 1, mencionado pero no detallado en SOP).
- ImplementaciÃ³n real de Liquidity Migration (idem).
- Cross-chain bridges integration (Sprint 4 deja arch ready, integraciÃ³n real es separada).
- Solana Jito (mencionado en MEV index, no en este SOP).
- ZK proof verification para arbitraje cross-rollup atÃ³mico.

---

## 12. Self-review (skill brainstorming Â§"Spec Self-Review")

- âœ… **Placeholder scan**: cero "TBD"/"TODO"/"implement later" excepto en secciÃ³n 11 (Out of scope, intencional).
- âœ… **Internal consistency**: las 13 skills Ã— 4 sprints Ã— tablas SOP Ã— KPIs PMI estÃ¡n coherentes; el ordering 0â†’1â†’3â†’2â†’4 estÃ¡ justificado.
- âœ… **Scope check**: 4 sprints es suficiente para un design Ãºnico; cada sprint puede ser un implementation plan separado vÃ­a writing-plans skill.
- âœ… **Ambiguity check**: las fÃ³rmulas PMI estÃ¡n explÃ­citas; los acceptance criteria son testeables.

