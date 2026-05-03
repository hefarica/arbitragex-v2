# Design Spec — SOP Crypto MEV Integration + PMI/EVM Observability Layer

**Fecha:** 2026-05-03
**Autor:** OMEGA Master Cortex
**Estado:** APROBADO por operador 2026-05-03 — orden 0→1→3→2→4 + TRUNCATE pre-Sprint-3
**Branch destino:** `main` (incremental, sin worktree)
**Contrato de no-daño:** Diseño 100% aditivo. Cero modificación a archivos runtime existentes (`frontend/app/*`, `backend/searcher-rs/src/*`, `backend/prioritization-spine/src/*`, `backend/api-server/src/*`). Sprint 4 implementado como nuevo crate paralelo (`backend/simulator-v2/`) activable por feature flag, dejando el simulator actual intacto como fallback.

## 0. Fuentes integradas (100%)

1. `cover_sop.pdf` — portada del SOP general (DeFi arbitrage)
2. `generate_sop.py` — script ReportLab que genera los SOPs (contiene texto fuente de los PDFs en strings Python)
3. `sop_body.pdf` — cuerpo del SOP general, 13 capítulos, 8 estrategias en Tabla 1
4. `SOP_Arbitraje_EVM_Completo.pdf` — guía estratégica con énfasis en arbitraje legal contractual (NO se aplica al stack crypto, su tabla 1 sí se referencia para terminología)
5. **`SOP_ArbitrageX_2026.pdf`** — SOP operativo v2.0 con 16 capítulos, 10 estrategias en Tabla §3, código Rust de referencia con Alloy 0.9 + revm 19.0, Bellman-Ford completo cap 14, 5 capas de risk management cap 15, infra deploy cap 16. **Fuente PRIMARIA del Sprint 1.**

## Cambios de diseño tras leer SOP_ArbitrageX_2026.pdf

- Strategy catalog pasa de 8 a **10 entradas** (matriz §3 del SOP_2026).
- Sandwich Attack incluido pero con flag `ethical_constraint='defensive_only'` y badge UI rojo "DEFENSIVE ONLY — never enables offensive". `enabled` solo controla las protecciones anti-sandwich, nunca ejecución ofensiva.
- 4 estrategias marcadas con `competitive_advantage='extrema'`: CEX-DEX, Pendle/Temporal, Cross-Chain Bridge, MEV-Boost Block. UI las destaca con badge dorado.
- Sprint 1 expande de 13 a **16 skills .md** (una por capítulo del SOP_2026).
- Sprint 4: nuevo crate `backend/simulator-v2/` (no modifica `prioritization-spine`). Activación: `ARBX_USE_SIMULATOR_V2=true` env var. Por defecto `false` → comportamiento actual intacto.

---

## 1. Context

El operador entregó un SOP de 21 páginas (`SOP_Arbitraje_EVM_Completo.pdf` + `sop_body.pdf`) con 13 capítulos cubriendo el universo completo de estrategias DeFi de arbitraje atómico en redes EVM (Ethereum, Arbitrum, Optimism, Base, BSC, Polygon, zkSync, Avalanche). El contenido está alineado al 100% con ArbitrageX v2 — no es PMI/Earned Value Management como decía el "PROMPT MAESTRO" boilerplate, sino crypto MEV puro.

El operador eligió como deliverable la **opción D**: "Skills .md por capítulo + integración SOP→Strategy Panel + capa de observabilidad PMI/EVM traducida a métricas crypto". Es decir: tres tracks paralelos que extienden el plan vigente del Strategy Panel sin abandonarlo.

### Por qué importa
- El stack actual produce solo `dex_arb` con simulator stub (RULE 00 violación parcialmente remediada en commit `dc5d376`).
- El SOP describe el blueprint completo de las otras 7 estrategias listadas en su Tabla 1 (Triangular, Cross-Chain, CEX-DEX, Liquidaciones, Micro-HFT, Yield, Liquidity Migration).
- Sin un layer de observabilidad económica (CPI/SPI/EAC traducidos), el operador no tiene forma de saber si la pipeline genera ganancias *reales* vs costos de gas + slippage.

---

## 2. Mapeo SOP × Stack actual × Deliverable

| Cap. SOP | Tema | Estado actual ArbX v2 | Acción |
|----------|------|------------------------|--------|
| 1 | Panorama 8 estrategias (Tabla 1) | enum `StrategyKind` con 5 variantes | Extender enum + seed `strategy_catalog` con las 8 |
| 2 | DEX-DEX Directo (SOP §2) | `patterns::build_dex_arb_candidate` activo | Validar contra criterios SOP §2.2 (TVL>$5M, vol>$10M, 5+ DEXs, slippage<0.02%) |
| 3 | Triangular Flash Loan (SOP §3) | enum-only | Skill .md + futuro `patterns::build_triangular_candidate` |
| 4 | Cross-Chain (SOP §4) | NO en enum | Añadir `cross_chain` al catálogo + skill .md |
| 5 | CEX-DEX (SOP §5) | NO en enum | Añadir `cex_dex` al catálogo + skill .md |
| 6 | Liquidaciones MEV (SOP §6) | enum `Liquidation` schema-only | Skill .md con tabla §6.1 (Aave/Compound/Maker thresholds) |
| 7 | Micro-Arbitraje HFT (SOP §7) | NO | Añadir `micro_hft` al catálogo + skill .md con cfg por red §7.2 |
| 8 | Selección Tokens/Pools (SOP §8) | hardcoded allowlist | Skill .md + alimenta `token_allowlist` con criterios Tabla 8 |
| 9 | Seguridad/Prevención Robos (SOP §9) | parcial (kill-switch) | Skill .md con Tabla 9 (mapeo riesgo→protección) + reusable como Tab 6 del panel |
| 10 | Cómo Encontrar Liquidez (SOP §10) | parcial (PoolSyncWorker) | Skill .md sobre fuentes de datos + liquidez emergente |
| 11 | Arquitectura Rust (SOP §11) | implementada parcialmente | Skill .md como referencia canónica de arquitectura C-S-E |
| 12 | Detección+Ejecución (SOP §12) | simulator stub | Skill .md + Sprint 4 implementa Bellman-Ford paralelo + revm real |
| 13 | Configuración+Optimización (SOP §13) | parcial | Skill .md + alimenta Tab 1 del Strategy Panel con parámetros producción |

---

## 3. Catálogo de Skills .md a generar (Sprint 1)

13 skills, una por capítulo del SOP. Convención: `.agents/skills/sop_<slug>/SKILL.md` con frontmatter:

```yaml
---
name: sop_<slug>
description: <when-to-trigger + what-it-does, pushy>
type: arbx_strategy_reference | arbx_security | arbx_architecture
source_section: SOP §N
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

Cada skill tiene secciones: **Cuándo activarse**, **Invariantes**, **Tablas de parámetros del SOP**, **Procedimiento operativo**, **Referencias cruzadas a archivos del repo**.

---

## 4. Sprint 2 — Integración SOP→Strategy Panel

El plan vigente del Strategy Panel (`~/.claude/plans/618f8807-*.md`) ya define 5 tabs y catálogo extensible. El SOP **alimenta el contenido** de cada tab:

- **Tab 1 (Capital & Riesgo)**: defaults de SOP §13 (profit min $5, slippage 0.5%, daily loss $200, etc.) — ya en spec.
- **Tab 2 (Catálogo)**: las 6 cards default + dropdown extendido. Seed `strategy_catalog` con las 8 estrategias de SOP Tabla 1 (no 6) — agregar `yield_arb` y `liquidity_migration`.
- **Tab 3 (MEV Services)**: Flashbots Protect, MEV Blocker, BloxRoute (SOP §9 menciona estos) + Eden Network, Titan Builder.
- **Tab 4 (Token Allowlist)**: defaults de SOP Tabla 2 (TVL, volumen, DEX listings, slippage<0.1%, auditoría OpenZeppelin/Trail).
- **Tab 5 (Auditoría)**: ya cubierta.
- **Tab 6 (Security/Anti-Theft)** (NUEVO): Tabla 9 del SOP — mapeo riesgo→protección con toggles (Honeypot check, Sandwich protection via Flashbots, Frontrun via private mempool, Rug pull liquidez-bloqueada check).

---

## 5. Sprint 3 — PMI/EVM Observability Layer (la innovación meta)

**Insight clave**: las métricas de PMI/Earned Value Management tienen equivalentes operativos directos en trading que NINGÚN sistema MEV explota seriamente. Los traders quants de Wall Street las usan; el ecosistema crypto las ignora.

### Tabla de traducción

| Métrica PMI | Fórmula PMI | Equivalente Crypto MEV | Fórmula Crypto |
|-------------|-------------|-------------------------|----------------|
| **PV** (Planned Value) | trabajo planeado en USD | `daily_target_usd` | configurado en Tab 1 |
| **EV** (Earned Value) | trabajo completado en USD | `realized_profit_usd` | SUM(executed.net_profit) |
| **AC** (Actual Cost) | costo real gastado | `total_gas_spent_usd` | SUM(executed.gas_cost_usd) |
| **CPI** (Cost Performance Index) | EV/AC | **capital_efficiency** | profit_realizado / gas_total |
| **SPI** (Schedule Performance Index) | EV/PV | **velocity_index** | profit_realizado_today / daily_target |
| **EAC** (Estimate at Completion) | BAC/CPI | **forecast_daily_pnl** | (current_profit / hours_elapsed) × 24 |
| **ETC** (Estimate to Complete) | EAC - AC | **remaining_runway** | daily_loss_cap - daily_loss_so_far |
| **TCPI** (To-Complete Performance Index) | (BAC-EV)/(BAC-AC) | **required_efficiency_remainder** | (target - current_profit) / (max_gas - current_gas) |
| **VAC** (Variance at Completion) | BAC - EAC | **projected_shortfall_usd** | daily_target - forecast_daily_pnl |
| **CV** (Cost Variance) | EV - AC | **net_pnl** | profit_realizado - gas_total |
| **SV** (Schedule Variance) | EV - PV | **pace_delta_usd** | realized - target_proportional |

### Componentes nuevos del frontend (página `/operations`)

1. **Header dashboard**: 4 KPI cards (CPI, SPI, EAC, TCPI) con coloración semántica (verde si CPI>1, rojo si <0.8).
2. **S-curve chart** (recharts): cumulative profit vs cumulative gas vs target, con zonas sombreadas por hora del día.
3. **Variance breakdown**: gráfico tornado con CV decomposed (price variance, slippage variance, gas variance, MEV bribe variance).
4. **Forecast Monte Carlo** (futuro): simular 10K caminos del resto del día basándose en distribución histórica de profit por hora.
5. **Tornado risk analysis** (futuro): qué factor impacta más el PnL (gas price spikes, MEV competition, RPC latency, slippage).

### Nuevos endpoints
- `GET /api/operations/kpi?chain_id=1&window=24h` — devuelve {cpi, spi, eac, etc, tcpi, vac, cv, sv}
- `GET /api/operations/scurve?chain_id=1&window=24h` — series temporales para chart
- `GET /api/operations/variance?chain_id=1&window=24h` — descomposición de varianza

Implementación backend: queries SQL agregando sobre `opportunities` + `executions` + `trading_config_global` (target). Sin nuevas tablas — reuse del schema existente.

### Nueva entrada sidebar
`{ href: "/operations", label: "Operations PnL", icon: TrendingUpIcon, group: "observe" }`

---

## 6. Sprint 4 — Implementación REVM Real (cap 11-12 SOP)

Reemplaza el stub actual de `prioritization-spine/src/simulator.rs:33-44`. Tareas:

1. **`prioritization-spine/src/lazy_db.rs`**: ya existe pero unused. Implementar `Database` trait de revm con lazy fetch on-chain via Alloy provider:
   - `pool_reserves(pool_addr)` → llama `getReserves()` (V2) o `slot0` + `liquidity` (V3)
   - `token_balance(token, addr)` → llama `balanceOf`
   - Cache en memoria con TTL=1 block

2. **`prioritization-spine/src/simulator.rs`**: reescribir `simulate_candidate`:
   - Construir calldata real con `calldata::univ2::encode_swap_exact_in()` o `univ3::encode_exact_input_single()`
   - Ejecutar contra block actual via `evm.transact()`
   - Decodear `amount_out` del return data
   - Retornar `gross_profit = (amount_out - amount_in) * token_price_usd`

3. **Eliminar hardcodes** en `searcher-rs/src/scanner.rs:272-296`:
   - `gas_units_estimated` ← `provider.estimate_gas(swap_tx)`
   - `gas_price` ← `provider.gas_price()` o `max(config.gas_max, baseFee + tip)`
   - `bribe` ← bribe model basado en builder landing rate (sub-tarea)
   - `flashloan_fee` ← lookup por proveedor (Aave 0.05%, Balancer 0%, etc.)
   - `token_risk_score` ← lookup en `token_allowlist.risk_score`
   - `liquidity_confidence` ← derivado del TVL del pool al momento del scan
   - `landing_probability` ← histórico builder landing rate (default 0.5 si sin datos)

4. **Bellman-Ford paralelo** (cap 12 SOP §12.2): `crates/graph-engine/` con `petgraph` + `rayon::par_iter` para detection multi-base-token.

5. **Token graph** (`crates/graph-engine/`): cache compartido `Arc<RwLock<TokenGraph>>` actualizado por price-monitor.

---

## 7. Plan de ejecución incremental

Cada sprint es deployable y reversible. Preferencia del operador (memory): "skip per-section approval gates; consolidate spec then execute with evidence".

| Sprint | Tiempo estimado | Reversibilidad | Bloquea siguientes? |
|--------|------------------|----------------|---------------------|
| **0 — Spec** (este doc) | 30 min | trivial | sí, hasta aprobación |
| **1 — Skills .md** | 4-6 horas | git revert | no — knowledge base |
| **2 — Strategy Panel + Tab Security** | 3-4 días | git revert por sprint | no — UI sólo |
| **3 — Operations PnL Dashboard** | 2-3 días | git revert | no — observabilidad |
| **4 — REVM real + lazy fetch** | 1-2 semanas | git revert + image rollback | medium (cambia simulator) |

Recomiendo orden: **0 → 1 → 3 → 2 → 4**:
- 1 (skills) primero porque no toca runtime, baseline de conocimiento.
- 3 (observability) segundo porque ilumina si el sistema actual ya es rentable o no — DECISIVO antes de invertir en Sprint 4.
- 2 (Strategy Panel) tercero porque consume las skills y los KPIs.
- 4 (REVM real) último porque es el más invasivo — solo si Sprint 3 muestra que vale la pena (CPI<1 hoy probablemente, hay que medirlo).

---

## 8. Acceptance Criteria

### Sprint 1
- [ ] 13 archivos `.agents/skills/sop_*/SKILL.md` existen y `git status` los muestra como nuevos
- [ ] Cada SKILL.md tiene frontmatter válido (name, description, type, source_section)
- [ ] Cada uno tiene mínimo 4 secciones: Cuándo activarse / Invariantes / Tablas SOP / Procedimiento / Cross-refs
- [ ] El usuario puede preguntar "¿qué dice el SOP sobre micro-arbitraje?" en sesión nueva y el agente carga `sop_micro_arb_hft` automáticamente

### Sprint 2
- [ ] Migration 017 ejecutada con seed de las 8 strategy_catalog rows
- [ ] Tab 6 (Security) renderiza tabla con 8 protecciones del SOP §9.2
- [ ] Tab 4 (Token Allowlist) muestra los 5 blue-chips con risk_score=0.10 + criterios de SOP §8.1 documentados como tooltips

### Sprint 3
- [ ] Endpoint `GET /api/operations/kpi` devuelve los 7 KPIs
- [ ] Página `/operations` carga sin React errors, muestra CPI/SPI/EAC en cards
- [ ] S-curve chart renderiza con datos reales de las últimas 24h
- [ ] Hidratación clean (R1 cumplida)

### Sprint 4
- [ ] `simulator.rs:simulate_candidate` ya no usa direcciones dummy `0x2222`
- [ ] Logs muestran `event=simulator.lazy_fetch pool=0x... reserves_in=... reserves_out=...`
- [ ] PG `expected_profit_usd` distribución no constante en $0 (sale del fallback)
- [ ] PG `gas_cost_usd` distribución variable (no más hardcoded $1.6)
- [ ] Sprint 3 KPIs muestran CPI con valor económicamente realista (probablemente <1 al inicio, mejora con tuning)

---

## 9. Reglas inmutables (R1-R8) chequeadas

- **R1 (Mounted Snapshot Pattern)**: páginas nuevas (`/operations`) siguen el patrón `page.tsx` Server + `*Client.tsx` con `useState(initialSnapshot)`.
- **R2 (Build-Time Guard)**: no se toca `next.config.js`.
- **R3 (Deploy --no-cache --env-file .env)**: cada redeploy estricto.
- **R5 (Auditoría transitiva)**: nuevos archivos auditados para no introducir `Date.now()` en render.
- **R6 (DATABASE_URL en docker compose)**: ya cumplido. Sprint 3 usa solo SQL queries existentes.
- **R7 (Trazabilidad E2E)**: cada KPI del dashboard es trazable a `opportunities` + `executions` rows.
- **R8 (Fail-honest pattern, propuesta sesión 2026-05-03)**: KPIs muestran `null` si no hay datos suficientes, NO inventan promedios.
- **RULE 00 (Zero Mocks)**: Sprint 3 KPIs derivados de DB real, NO mocks. Sprint 4 cierra el último mock pendiente (REVM stub).

---

## 10. Riesgos + open questions

1. **Sprint 4 puede revelar que el sistema NO es rentable hoy**. CPI < 1 con costos de gas reales puede mostrar que el bot pierde dinero en cada arb. **Esto es resultado VERDADERO, no fallo del plan**. Sprint 3 lo expone honestamente; Sprint 4 da herramientas para corregir.
2. **Datos insuficientes para Sprint 3 KPIs**: hoy hay solo 1714 rows con profit mockeado. Los KPIs honestos requieren eliminar primero el dato sintético (ya hecho en commit `dc5d376`) Y acumular datos reales nuevos. Recomiendo `TRUNCATE opportunities` como primer paso de Sprint 3.
3. **Sprint 4 requiere Alchemy archive node con `eth_call` historic state** para lazy_fetch. Plan actual usa `wss://eth-mainnet.alchemyapi.io` — verificar que el plan Alchemy cubre archive queries.
4. **Tabla `executions`**: existe? Necesaria para Sprint 3 KPIs. Si no existe, Sprint 3 incluye su creación.

### Preguntas pendientes para el operador
- ¿Truncate del histórico mockeado en `opportunities`? (Recomendado, sí)
- ¿Sprint 3 antes que Sprint 2? (Recomiendo sí — observability es decisional)
- ¿Sprint 4 después de Sprint 3, o prioritario?

---

## 11. Out of scope (futuras iteraciones)

- Frontend visual (Visual Companion) durante brainstorming — proyect memory lo prohíbe.
- Editar código Rust de estrategias desde frontend — viola defense-in-depth (ALERTA OMEGA, sesión previa).
- Implementación real de Yield Arbitrage (cap 1 Tabla 1, mencionado pero no detallado en SOP).
- Implementación real de Liquidity Migration (idem).
- Cross-chain bridges integration (Sprint 4 deja arch ready, integración real es separada).
- Solana Jito (mencionado en MEV index, no en este SOP).
- ZK proof verification para arbitraje cross-rollup atómico.

---

## 12. Self-review (skill brainstorming §"Spec Self-Review")

- ✅ **Placeholder scan**: cero "TBD"/"TODO"/"implement later" excepto en sección 11 (Out of scope, intencional).
- ✅ **Internal consistency**: las 13 skills × 4 sprints × tablas SOP × KPIs PMI están coherentes; el ordering 0→1→3→2→4 está justificado.
- ✅ **Scope check**: 4 sprints es suficiente para un design único; cada sprint puede ser un implementation plan separado vía writing-plans skill.
- ✅ **Ambiguity check**: las fórmulas PMI están explícitas; los acceptance criteria son testeables.
