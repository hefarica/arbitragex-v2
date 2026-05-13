> **⚡ AWARENESS**: Este archivo es §15-§31. El root `CLAUDE.md` (§1-§14) tiene las reglas inmutables R0-R7, identidad OMEGA, anti-reincidencia y mapa de skills. Léelo PRIMERO.

> **🔗 TOTAL SYSTEM**: `CLAUDE.md` (reglas + identidad) + `.claude/CLAUDE.md` (superpowers + estrategias) = 523 líneas de doctrina operativa OMEGA.

> **🧠 OMEGA PROTOCOL + X10THINK**: Usa SIEMPRE pensamiento extendido X10 (piensa 10x más profundo — edge cases, failure modes, consecuencias de segundo orden, alternativas descartadas). Aplica X10THINK tanto tú como TODOS los agentes que despachas. Al terminar CUALQUIER tarea: (1) verifica funcionamiento ejecutando test/build/curl/docker logs, (2) verifica integridad global — typecheck completo + que no rompiste nada, (3) si falla entra en loop de corrección autónomo SIN preguntar hasta que pase, (4) consulta root `CLAUDE.md` §2-§3 para reglas R0-R7, (5) consulta §24 risk + §27 honeypot si tocaste trading/contratos. NUNCA entregues sin verificación. NUNCA preguntes si verificar — SIEMPRE verifica al 100%.
# ARBITRAGEX OMEGA CORTEX — SUPERPOWERS EXTENSION

> **Versión:** 2.0 — 2026-05-04T04:49Z
> **Ubicación:** `.claude/CLAUDE.md` (leído automáticamente por Claude Code junto con el root `CLAUDE.md`)
> **Extiende:** `CLAUDE.md` v1.0 (§1-§14) — léelo PRIMERO
> **Design Spec aprobado:** `docs/superpowers/specs/2026-05-03-sop-evm-integration-design.md`

---

## 15. SOP OPERATIVO — PATRÓN C-S-E (Compose-Simulate-Execute)

El SOP define el patrón C-S-E de Paradigm como la arquitectura canónica:

1. **Compose**: Construir grafo de rutas desde pools activos. Nodos = tokens, aristas = pools con tasa + gas cost. Bellman-Ford detecta ciclos de peso negativo (= oportunidades).
2. **Simulate**: Cada ruta candidata se simula con **revm 19.0** + estado real on-chain vía alloy-provider. Incluye fees LP, slippage, impacto precio, gas. Solo avanzan rutas con beneficio neto positivo.
3. **Execute**: Rutas verificadas → bundle atómico → Flashbots Protect o MEV-Boost relay. Atomicidad: todo se ejecuta o nada. Cero ejecución parcial.

> **Skill de referencia:** `.agents/skills/sop_csa_architecture/SKILL.md`

---

## 16. MIGRACIÓN OBLIGATORIA: ethers-rs → Alloy 0.9

### Estado actual (a corregir)
```toml
# backend/Cargo.toml — ACTUAL (OBSOLETO)
ethers = { version = "2", default-features = false, features = ["ws","rustls"] }
alloy-primitives = { version = "0.7" }  # parcial
```

### Target (SOP v2.0)
```toml
alloy = { version = "0.9", features = ["full"] }
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
alloy-provider = { version = "0.9", features = ["ws"] }
alloy-rpc-types = "0.9"
alloy-transport-ws = "0.9"
alloy-network = "0.9"
revm = "19.0"
revm-primitives = "9.0"
```

### Por qué es obligatorio
- `ethers-rs` está **ARCHIVADO** — sin parches de seguridad.
- Alloy ofrece **zero-copy decode** — crítico para HFT (miles de txs/segundo del mempool).
- Alloy + revm comparten `alloy-primitives` — eliminan conversiones de tipos.
- **Crates afectados**: `searcher-rs`, `sim-ctl`, `shared-rs`, `relays-client`, `recon`, `prioritization-spine`.

---

## 17. MATRIZ DE 10 ESTRATEGIAS (SOP §3)

| # | Estrategia | Riesgo | Profit | Velocidad | Capital | Ventaja | Estado ArbX |
|---|-----------|--------|--------|-----------|---------|---------|-------------|
| 1 | DEX Triangular | Muy Bajo | 0.1-2% | <100ms | 0 (Flash) | Alta | enum activo |
| 2 | Cross-DEX Price Diff | Bajo | 0.05-1.5% | <200ms | 0 (Flash) | Muy Alta | `dex_arb` activo |
| 3 | Sandwich (DEFENSIVO) | — | — | — | — | — | defensive_only=true |
| 4 | Liquidation MEV | Bajo | 2-15% | <500ms | Variable | Alta | enum schema-only |
| 5 | JIT Liquidity | Muy Bajo | 0.3-3% | <150ms | Bajo | **Muy Alta** | pendiente |
| 6 | Flashbots Bundle | Muy Bajo | Variable | <100ms | 0 (Flash) | Alta | relays-client parcial |
| 7 | **CEX-DEX** | Bajo | 0.1-3% | <300ms | Medio | **EXTREMA** | pendiente |
| 8 | **Pendle/Temporal AMM** | Medio | 1-10% | <1s | Medio | **EXTREMA** | pendiente |
| 9 | **Cross-Chain Bridge** | Medio | 0.2-5% | 1-30s | Medio | **EXTREMA** | pendiente |
| 10 | **MEV-Boost Block Build** | Alto | Variable | <12s | Alto | **EXTREMA** | pendiente |

> Las 4 estrategias con ventaja "EXTREMA" son donde **el 99% de competidores no puede operar**. Son el diferenciador de ArbitrageX.

> **Skills de referencia**: `.agents/skills/sop_*/SKILL.md` (13 skills, una por capítulo SOP)

---

## 18. CÓDIGO DE REFERENCIA ALLOY — PATRONES CLAVE

No duplicar aquí. Claude Code: **lee las skills cuando el contexto lo requiera:**

| Patrón | Skill | Qué contiene |
|--------|-------|-------------|
| Loop del searcher (WS + mempool) | `sop_csa_architecture` | `run_searcher()` completo con Alloy WsConnect |
| Quoter Uniswap V3 (zero-copy) | `sop_dex_triangular` | `check_triangular_arb()` con `IQuoter::quoteExactInputSingleCall` |
| CEX-DEX spread detection | `sop_cex_dex` | `cex_dex_arb_loop()` con Binance WS + Alloy on-chain |
| Health Factor monitoring | `sop_liquidations` | `monitor_liquidations()` con `ILendingPool` Alloy interface |
| Flashbots bundle construction | `sop_flashbots_bundles` | `FlashbotsBundle` struct + `submit_flashbots_bundle()` |
| Bellman-Ford arbitrage graph | `sop_atomic_route_construction` | `ArbitrageGraph` con `find_arbitrage_cycle()` + `reconstruct_cycle()` |
| Token safety verification | `sop_scam_detection` | `is_token_safe()` — 5-step verification pipeline |
| Multi-DEX liquidity aggregation | `sop_liquidity_aggregation` | `find_best_liquidity()` + `compute_split_route()` |
| Micro-arbitrage HFT scanner | `sop_micro_arb_hft` | `scan_micro_arbs()` — volume sobre home runs, 100-1000 ops/día |
| Risk management 5 capas | `sop_risk_management` | Position sizing 2%, gas 3x, slippage 0.5%, stop-loss, private mempool |

---

## 19. SMART CONTRACT DE ARBITRAJE — ArbitrageExecutor.sol

Lee `docs/superpowers/pdf_extracts/evm_arbitrage_body.md` para el contrato completo. Patrón crítico:

```solidity
// Flash loan → swaps → repay → profit. TODO atómico.
function executeArbitrage(
    address flashLoanProvider,
    address borrowToken,
    uint256 borrowAmount,
    SwapStep[] calldata steps  // pool, tokenIn, tokenOut, dexType, extraData
) external onlyOwner returns (uint256 profit) {
    uint256 balanceBefore = IERC20(borrowToken).balanceOf(address(this));
    ILendingPool(flashLoanProvider).flashLoanSimple(address(this), borrowToken, borrowAmount, abi.encode(steps));
    profit = IERC20(borrowToken).balanceOf(address(this)) - balanceBefore;
    require(profit > 0, "No profit generated");
    IERC20(borrowToken).safeTransfer(msg.sender, profit);
}
```

**Invariantes del contrato:**
- `onlyOwner` en toda función de ejecución.
- `require(profit > 0)` — si no hay ganancia, todo revierte.
- `nonReentrant` en todas las funciones que mueven fondos.
- Soporta multi-DEX via `dexType`: 1=UniV3, 2=Curve, 3=Balancer.

---

## 20. PMI/EVM OBSERVABILITY LAYER (Sprint 3)

Métricas de Wall Street aplicadas a crypto. **Ningún otro sistema MEV las usa.**

| Métrica PMI | Equivalente Crypto | Fórmula | UI |
|-------------|--------------------|---------|----|
| CPI (Cost Performance Index) | **capital_efficiency** | profit_realizado / gas_total | Card verde si >1 |
| SPI (Schedule Performance Index) | **velocity_index** | profit_today / daily_target | Card verde si >1 |
| EAC (Estimate at Completion) | **forecast_daily_pnl** | (profit / hours) × 24 | Card con forecast |
| TCPI (To-Complete Performance) | **required_efficiency** | (target - profit) / (max_gas - gas) | Card amarilla/roja |
| CV (Cost Variance) | **net_pnl** | profit - gas_total | Bottom line |

**Endpoints**: `GET /api/operations/kpi`, `/scurve`, `/variance`
**Página**: `/operations` con S-curve chart (Recharts)
**Regla R8 (nueva)**: KPIs muestran `null` si datos insuficientes. NUNCA inventan promedios.

---

## 21. PARES Y POOLS POR CADENA (SOP §10)

| Cadena | Pares Principales | TVL | Vol 24h |
|--------|-------------------|-----|---------|
| Ethereum L1 | WETH/USDC, WBTC/ETH, USDC/USDT, LINK/ETH | $95B | $4.2B |
| BSC | WBNB/USDT, CAKE/BNB, BUSD/USDT | $5.8B | $1.8B |
| Arbitrum | WETH/USDC, GMX/ETH, ARB/ETH, RDNT/ETH | $3.2B | $0.9B |
| Optimism | WETH/USDC, OP/ETH, SNX/ETH | $1.5B | $0.5B |
| Base | WETH/USDC, BASE/ETH, AERO/ETH | $2.1B | $0.7B |
| Polygon | WMATIC/USDC, WETH/USDC, QUICK/ETH | $1.8B | $0.6B |

**Criterios de selección de pool** (ratio volumen/TVL >30% = prometedor):
- TVL: profundidad real.
- Vol/TVL ratio: rotación de capital.
- Fee tier: costo por operación.
- Historial de exploits: descartar pools vulnerables.

---

## 22. RPC FALLBACK POR CADENA (SOP §16)

| Cadena | RPC Primario | RPC Secundario | RPC Terciario |
|--------|-------------|----------------|---------------|
| Ethereum | Alchemy (dedicated) | QuickNode (dedicated) | Flashbots RPC |
| Arbitrum | Alchemy ARB | QuickNode ARB | Public ARB RPC |
| Base | Alchemy BASE | QuickNode BASE | Public BASE RPC |
| BSC | Ankr BSC | QuickNode BSC | Public BSC RPC |
| Polygon | Alchemy MATIC | QuickNode MATIC | Public MATIC RPC |

**Regla**: Si el primario no responde en <50ms → conmuta automáticamente. Mínimo 3 proveedores por cadena.

---

## 23. PROTECCIÓN MEV — SERVICIOS ACTIVOS

| Servicio | Mecanismo | Costo | Cobertura | Latencia |
|----------|-----------|-------|-----------|----------|
| Flashbots Protect | Private relay to builders | Gratis (tips) | Ethereum | 200-500ms |
| MEV Blocker | Batch auctions | 0% | Ethereum | 300-600ms |
| BloxRoute BDN | Private tx broadcast | Suscripción | Multi-chain | 100-300ms |
| Titan Builder | Builder direct | Gratis | Ethereum | 200-400ms |
| Eden Network | RPC + builder | Suscripción | Ethereum | 150-350ms |

**Regla inmutable**: TODA transacción de ArbitrageX va por mempool privado. NUNCA mempool público.

---

## 24. RISK MANAGEMENT — 5 CAPAS (SOP §15)

1. **Position Sizing**: Nunca >2% del capital total por operación.
2. **Gas Protection**: Beneficio neto ≥ 3× costo de gas estimado. Gas price oracle en tiempo real.
3. **Slippage Guard**: Máximo 0.5% por swap. `amountOutMin` calculado dinámicamente.
4. **Stop-Loss Automático**: Pérdida acumulada >0.5% capital en 1h → modo protección.
5. **Private Mempool**: Flashbots + MEV Blocker + Titan. Cero visibilidad para otros bots.

> **Skill de referencia:** `.agents/skills/sop_risk_management/SKILL.md`

---

## 25. SPRINT ROADMAP (Design Spec aprobado)

| Sprint | Qué | Tiempo | Reversible |
|--------|-----|--------|-----------|
| 0 | Spec (COMPLETADO) | — | — |
| 1 | 16 Skills .md del SOP | 4-6h | git revert |
| 3 | Operations PnL Dashboard (PMI/EVM) | 2-3 días | git revert |
| 2 | Strategy Panel + Tab Security | 3-4 días | git revert |
| 4 | REVM real + lazy fetch + Bellman-Ford | 1-2 semanas | git revert + rollback |

**Orden aprobado: 0→1→3→2→4** (observabilidad antes de UI, REVM al final porque es lo más invasivo).

---

## 26. REGLA R8 — FAIL-HONEST PATTERN (NUEVA)

Los KPIs y métricas del dashboard Operations PnL **DEBEN mostrar `null`** si no hay datos suficientes para calcular un valor significativo. PROHIBIDO:
- Inventar promedios.
- Mostrar 0 cuando la realidad es "no hay datos".
- Interpolar valores donde no existe base estadística.

Un CPI < 1.0 que muestra pérdida es un **resultado verdadero**, no un fallo del sistema.

---

## 27. HONEYPOT & RUG PULL DETECTION (SOP §11)

Pipeline de verificación de 5 pasos para todo token nuevo:

1. **Contract exists**: `provider.get_code_at(token)` — si vacío, rechazar.
2. **Honeypot check**: Simular venta con revm — si revierte, es honeypot.
3. **Transfer tax**: `estimate_sell_tax()` — si >5%, rechazar.
4. **Liquidity lock**: Verificar timelock — si desbloqueada, advertencia.
5. **Unrestricted mint**: Detectar función mint sin restricción — si existe, rechazar.

> **Skill de referencia:** `.agents/skills/sop_scam_detection/SKILL.md`

---

## 28. INFRAESTRUCTURA DE PRODUCCIÓN (SOP §16)

| Componente | Spec | Propósito |
|-----------|------|----------|
| VPS Principal | 4 vCPU, 16GB RAM, NVMe | Searcher + sim-ctl |
| VPS Backup | 2 vCPU, 8GB RAM | Failover + monitoreo |
| VPS CEX Feed | 2 vCPU, 4GB RAM, cercano a exchange | WebSocket price feeds |
| RPC Nodes | Dedicados (Alchemy/QuickNode) | Latencia <10ms |

**Ubicación óptima**:
- Ethereum L1: AWS us-east-1 o eu-central-1, latencia <20ms al nodo.
- Arbitrum: us-east-1 (cercano al sequencer).
- Base: us-west-2.
- CEX Feed: Tokio (Binance APAC) o Frankfurt (OKX/Bybit).

---

## 29. GAS OPTIMIZATION — SMART CONTRACT PATTERNS

- `unchecked {}` para aritmética sin overflow → ahorra 3-5 gas/op.
- Empaque de variables en slots 256-bit → 20K gas/slot vs 2.1K adicional.
- `calldata` > `memory` para arrays readonly.
- Assembly inline para balances/transfers → 10-15% más barato que IERC20.
- Multicall para agrupar swaps → ahorra 30-50K gas por swap adicional.
- Flash loan provider dinámico: Balancer (0%) > dYdX (0%) > Aave (0.05%).

---

## 30. DIRECTIVA DE PROGRESSIVE DISCLOSURE

Este archivo (`CLAUDE2.md`) sigue el patrón de **progressive disclosure** recomendado por las best practices de Claude Code:

- **NO dumpear código completo aquí** — las skills tienen el código.
- **SÍ incluir reglas, tablas, y punteros** a los archivos correctos.
- **CLAUDE.md** = reglas inmutables + identidad + estructura.
- **CLAUDE2.md** = superpowers + estrategias + SOP knowledge + observability.
- **`.agents/skills/`** = implementación completa por dominio.
- **`.agents/memory/`** = bitácora de incidentes operativos.
- **`docs/superpowers/`** = specs de diseño y PDFs extraídos.

### Cómo Claude Code usa las 3 capas
1. **Inicio de sesión**: Lee `CLAUDE.md` + `CLAUDE2.md` automáticamente.
2. **Contexto por trigger**: Activa skill específica de `.agents/skills/` según el mapa §5 del CLAUDE.md.
3. **Memoria persistente**: `~/.claude/projects/.../memory/` + `.agents/memory/anti_reincidencia.md`.

---

*SUPERPOWERS CARGADOS. 10 estrategias × 5 capas de riesgo × 16 skills SOP × PMI/EVM observability = ARSENAL COMPLETO.*

---

## 31. OBRA/SUPERPOWERS — METODOLOGÍA OBLIGATORIA (Plugin activo)

Plugin `superpowers@claude-plugins-official` está habilitado en `.claude/settings.local.json`. Las siguientes skills son **metodología obligatoria**, no sugerencias opcionales. Se activan automáticamente en el contexto correcto:

### Ciclo de desarrollo (orden secuencial)

1. **brainstorming** — Antes de escribir código. Refina ideas → preguntas Socráticas → alternativas → diseño validado por secciones. Guarda design document.
2. **writing-plans** — Con diseño aprobado. Tareas de 2-5 minutos cada una con paths exactos, código completo, pasos de verificación.
3. **subagent-driven-development** — Despacha subagente fresco por tarea con review de 2 fases: (1) compliance con spec, (2) calidad de código.
4. **test-driven-development** — RED-GREEN-REFACTOR obligatorio. Escribe test fallido → observa que falla → escribe código mínimo → observa que pasa → commit. **Si escribes código antes del test, bórralo.**
5. **requesting-code-review** — Entre tareas. Revisa contra el plan. Issues por severidad. Críticos bloquean progreso.
6. **finishing-a-development-branch** — Verifica tests → presenta opciones (merge/PR/keep/discard) → limpia worktree.

### Skills de soporte

7. **systematic-debugging** — 4 fases de root-cause analysis. Incluye defense-in-depth y condition-based-waiting.
8. **verification-before-completion** — CONFIRMA que está realmente arreglado antes de declarar victoria.
9. **using-git-worktrees** — Workspace aislado en branch nuevo. Setup de proyecto + test baseline limpio.
10. **executing-plans** — Ejecución en batches con checkpoints humanos.
11. **dispatching-parallel-agents** — Workflows concurrentes con subagentes.
12. **receiving-code-review** — Cómo responder a feedback de review.
13. **writing-skills** — Crear nuevas skills siguiendo best practices.
14. **using-superpowers** — Introducción al sistema de skills.

### Filosofía Superpowers (inmutable)

- **Test-Driven Development** — Tests primero, siempre.
- **Systematic over ad-hoc** — Proceso sobre improvisación.
- **Complexity reduction** — Simplicidad como objetivo primario.
- **Evidence over claims** — Verificar antes de declarar éxito.

> **REGLA**: El agente chequea skills relevantes ANTES de cada tarea. Son workflows mandatorios, no sugerencias.

---

## 32. OMEGA TEAM — PROTOCOLO INTERDISCIPLINARIO (§15 root CLAUDE.md)

10 subagentes PhD/Nobel en `.claude/commands/agent-*.md`. Ver §15 de `CLAUDE.md` para la división completa (7 builders + 3 validators) y la matriz de cross-validation.

### Reglas de despacho

1. **Feature nueva**: Despachar en orden Strategy → Math → Build → CS → UI → Security → Economics → DevOps → Data.
2. **Fix crítico**: CS → Build → Security → DevOps.
3. **Validación pre-mainnet**: Math → Economics → Security → CS.
4. **Comando rápido**: `/project:team` presenta los 3 workflows al operador.

### Cross-validation obligatoria

- Código Rust → validado por agent-cs (concurrencia) + agent-math (algoritmos).
- Contratos Solidity → validado por agent-security (exploits) + agent-math (AMM math).
- Estrategias → validadas por agent-economics (P&L real) + agent-math (game theory).
- KPIs/Datos → validados por agent-economics (costos completos, bias).
- Deploy → validado por agent-security (infra hardening).

### Principio del equipo

Ningún subagente declara "listo" sin que su validator asignado confirme. Un validator puede BLOQUEAR el avance del pipeline si encuentra un error que viola corrección formal, seguridad, o principios económicos verificados.

> **"Evidence over claims"** aplica a nivel de equipo: el trabajo de un builder es una *claim* hasta que un validator produce *evidence*.

---

## 33. PROTOCOLO /COMPACT — PRESERVACIÓN DE CONTEXTO

> **Regla cardinal**: El chat es volátil. Los archivos persisten. NUNCA compactar sin persistir primero.
> **ACLARACIÓN CRÍTICA**: El comando `/compact` es un slash command interactivo de Claude Code. Lo debes invocar TÚ (el agente) directamente en la terminal cuando se aproxime el límite de contexto, ya que el operador no puede ejecutarlo desde su extremo. Al invocarlo, el historial se compactará conservando hechos clave, commits, decisiones y el estado del sistema, brindando contexto fresco sin perder el hilo.

### Antes de `/compact`
Ejecutar `/project:compact` (o manualmente): persistir en `.agents/memory/session_state.md`:
- Commits de la sesión (hashes + mensajes)
- Decisiones arquitectónicas tomadas
- Trabajo en progreso (WIP)
- Sprint/Phase actual del roadmap
- Bugs conocidos activos con severidad
- Próximo paso claro

### Durante `/compact`
Incluir en el resumen de compactación:
- Total de commits
- Capas defensivas activas
- Sprint/Phase actual
- Próximo paso

### Después de `/compact`
**Primera acción obligatoria**: `cat .agents/memory/session_state.md` para recuperar contexto completo. No asumir que el resumen interno es suficiente.

### Cuándo invocar `/compact`
- Cuando el contexto supere ~80% del límite
- Cuando el agente empiece a olvidar decisiones anteriores de la sesión
- Antes de cambiar de tema/sprint drásticamente
- El operador puede invocarlo en cualquier momento

---

*SUPERPOWERS + OMEGA TEAM + COMPACT PROTOCOL CARGADOS. 10 estrategias × 5 capas riesgo × 16 skills SOP × 10 subagentes PhD × PMI/EVM = ARSENAL MÁXIMO.*

---

## 34. MISIÓN ACTUAL: V2 POOL DISCOVERY & PAPER TRADE (Desbloqueo)

El refactor estructural V2 existe pero sufre de silencios operacionales porque `ImpactIndex::resolve` retorna 0 pools ante pares no indexados del mempool (memecoins, long-tail). La misión es implementar expansión dinámica "on-the-fly".

### 34.1. PoolDiscoveryService (Implementación Obligatoria)
- **Ubicación:** `backend/searcher-rs/src/pool_discovery.rs`
- **Responsabilidad:** Usar RPC real para consultar on-chain `getPair()` o `getPool()`.
- **Reglas de inserción:**
  - Si retorna `address(0)` o falla al consultar reservas, NO inventar/insertar.
  - V3: probar fee tiers `100, 500, 3000, 10000` → convertir a bps (1, 5, 30, 100).
- **Persistencia en PG:** Respetar schema actual. Guardar tokens mínimos reales (address, symbol, decimals). No escribir columnas ficticias ni valores nulos forzados si el schema lo impide.

### 34.2. Observed Unindexed Pairs & Retry Workflow
- Si un intent no arroja impactos, registrar en `observed_unindexed_pairs`.
- Llamar a `PoolDiscoveryService` síncronamente o lanzar proceso asíncrono.
- Una vez descubiertos los pools → actualizar Redis + inyectar en `ImpactIndex` vivo (`impact_index.write().await.add_pool()`).
- Reintentar `ImpactIndex::resolve(&intent)`. Si ahora impacta >0, continuar pipeline.

### 34.3. Paper Trade & Observabilidad (Fail-Honest)
- Variable mandatoria: `ARBX_TRADE_MODE=paper`.
- Las observaciones (`opportunity_observations`) DEBEN emitir en cada fase (`impact_zero`, `discovery_failed`, `optimizer_rejected`, `paper_accepted`). 
- **Cero Ejecución:** En modo paper, no se firma ni envía nada a la red. Todo se persiste como paper opportunity con `gross_profit_usd` / `net_profit_usd` computado con reservas reales de V2/V3.

### 34.4. Validaciones Requeridas (Shadow / Paper)
- Ejecutar en VPS con `ARBX_ORCHESTRATOR_MODE=shadow` o `v2` y probar mínimo 10 minutos.
- Verificar eventos emitidos en logs: `pool_discovery.*`, `v2.impact.discovery_retry`, `v2.reserves.hydrated`, `dex_engine.structural_candidate`.
- Asegurar `cargo clippy` limpio, y realizar una búsqueda obligatoria: `grep -R "mock"`, `grep -R "hardcode"`, `grep -R "fake"`. Si hay fixtures en código productivo, el sistema SE RECHAZA.

---

## 35. ARBX RUNTIME-STATUS & OBSERVABILITY SKILLS — DETALLE OPERATIVO

Familia de 10 skills registrada 2026-05-12 tras implementación del endpoint `/api/v1/strategies/runtime-status`. Skills viven en `.agents/skills/arbx-<nombre>/SKILL.md`. Auditadas contra el spec original; 4 tienen issues menores documentados abajo (no impiden uso, pero requieren fix antes de seguir literalmente).

### 35.1. Inventario completo y triggers de invocación

| # | Skill (path: `.agents/skills/<n>/SKILL.md`) | Cuándo invocar | Output canónico |
|---|---------------------------------------------|----------------|------------------|
| 1 | `arbx-fail-honest-runtime-status` | Vas a crear/modificar endpoint read-only que agrega estado desde PG/Redis | JSON con `source.<dep>="ok"\|"unavailable"\|"not_used"` + payload por estrategia |
| 2 | `arbx-api-server-route-mounting` ⚠️ | Vas a montar ruta Express nueva en `backend/api-server/src/routes/*.ts` | Función `mountX(app, deps)` con DI explícita (`Pool`, `Redis`, logger) |
| 3 | `arbx-pg-redis-observability` | Vas a leer counters/aggregates desde PG (intervals dinámicos) o contar keys en Redis | `SCAN`-loop function + SQL paramétrico con `interval` |
| 4 | `arbx-strategy-status-semantics` ⚠️ | Vas a mapear conteos brutos a `data_dependencies_status` semántico por estrategia MEV | `armed_waiting_for_impact` / `waiting_for_profitable_base` / `missing_lending_watchlist` / `ok` |
| 5 | `arbx-edge-worker-proxy-wiring` | Vas a exponer ruta interna del api-server vía Cloudflare Worker | `app.get(pub, c => proxy(c, internal, cacheKey, ttlSec))` |
| 6 | `arbx-frontend-runtime-cards` ⚠️ | Vas a crear UI semaforizada consumiendo `/api/strategies/runtime-status` | Card React con loading/error/empty/data states explícitos |
| 7 | `arbx-vps-verification-runbook` | Acabas de hacer deploy y necesitas confirmar que el cambio está vivo sin abrir puertos | docker ps + curl `localhost:8080` + curl `edge-arbx.ape-tv.net` + logs --tail |
| 8 | `arbx-rust-searcher-observability` | Vas a añadir telemetry a `searcher-rs` (heartbeats, contadores, observations) | `tokio::spawn`/mpsc async → Redis `SET arbx:heartbeat:scanner:<chain>:latest` |
| 9 | `arbx-no-mocks-no-hardcode-audit` | Pre-commit obligatorio para cualquier cambio productivo | `grep -R "mock\|fake\|hardcode\|dummy\|Math\.random"` debe retornar 0 matches |
| 10 | `arbx-deployment-idempotency` ⚠️ | Vas a propagar un cambio commiteado al VPS | `git pull` working tree + `docker compose build --no-cache` + `up -d` + verificación |

### 35.2. Issues conocidos en las 4 skills marcadas ⚠️ (auditados 2026-05-12 — fix pendiente)

| Skill | Línea | Issue | Fix correcto |
|-------|-------|-------|--------------|
| `arbx-api-server-route-mounting` | 29 | Menciona "responder 200/206 Partial" | Solo `200` (con `source.<dep>="unavailable"`) o `503` (DB principal caída). **HTTP 206 está prohibido** por directiva explícita del usuario. |
| `arbx-strategy-status-semantics` | 19 | Path inexistente `frontend/src/app/operations/page.tsx` | Real: `frontend/app/operations/page.tsx` (Next.js 14 App Router sin `src/`) |
| `arbx-frontend-runtime-cards` | 19-20 | Mismo error de paths `frontend/src/...` | Real: `frontend/app/...` y `frontend/components/...` |
| `arbx-frontend-runtime-cards` | 35 | Verification step usa palabra "Mockea" | Real: "Simula apagando el server real o devolviendo 500 desde backend real (sin mocks)" |
| `arbx-deployment-idempotency` | 24 | `cd /opt/git/arbitragex-v2 && git pull` (bare repo, no admite pull) | Real: `cd /opt/arbitragex-v2 && git pull` (working tree). El bare repo `/opt/git/...` solo recibe pushes. |
| `arbx-deployment-idempotency` | 26 | `docker compose build --no-cache --env-file .env <svc>` (falta `-f` compose file) | Real: `docker compose --env-file .env -f docker/compose.prod.yml build --no-cache <svc>` |

### 35.3. Reglas inviolables heredadas de la directiva original al equipo

Todas las skills de esta familia deben respetar:

**1. Contrato de respuesta HTTP** (corrección explícita del usuario 2026-05-12):
- `200` = respuesta válida con datos completos O parciales declarados en `source.postgres` / `source.redis`.
- `503` = DB principal (PostgreSQL) no disponible para la query base de opportunities.
- **PROHIBIDO HTTP 206 Partial Content**.

**2. Semántica de valores en payload**:
- `null` = dato no disponible (Redis caído, tabla no existe, query opcional falló).
- `0` = dato real medido exactamente cero (e.g., `candidates_1h: 0` cuando query corrió OK y no había candidatos).
- **NUNCA** reemplazar `null` por `0` para "rellenar UI".

**3. Source declarations obligatorias**:
```json
"source": {
  "postgres": "ok" | "partial_or_failed" | "unavailable",
  "redis":    "ok" | "unavailable",
  "logs":     "not_used"
}
```

**4. Puertos y dominios canónicos** (no inventar, no rotar sin actualizar este doc):
- api-server: interno `127.0.0.1:8080` (NO 3002).
- selector-api: `127.0.0.1:3002`. sim-ctl: `3003`. recon: `3004`. relays-client: `3005`. token-enricher: `9004`.
- edge worker dev: `127.0.0.1:8787`. prod: Cloudflare Worker.
- frontend interno: `127.0.0.1:5173`. nginx público `:80` → proxy a `127.0.0.1:5173`. Cloudflare Tunnel HTTPS: `https://arbx.ape-tv.net`.
- Cloudflare Edge API público: `https://edge-arbx.ape-tv.net` (solo `/api/*` y `/socket.io/*` y `/status`/`/health`).

**5. Strategy kinds canónicos** (normalización requerida en endpoints runtime-status):
- Familia `dex_arb`: agrupa `dex_arb`, `dex_arb_v2v2`, `dex_arb_v3v3`, cualquier prefijo `dex_arb*`.
- Canónicos al frontend: `dex_arb`, `triangular_arb`, `flashloan_arb`, `liquidation`.

**6. Semántica de "sin candidatos" por estrategia** (NO marcar como `failed`):
- `triangular_arb` con 0 candidatos → `data_dependencies_status: "armed_waiting_for_impact"` ("Armado, esperando impacto rentable").
- `flashloan_arb` con 0 candidatos → `"waiting_for_profitable_base"` ("Esperando base profitable").
- `liquidation` sin watchlist → `"missing_lending_watchlist"` ("Requiere watchlist lending").
- `dex_arb` con `rejected > 0` y `viable = 0` → "Detectando, rechazando por gates" (NO failed).

**7. Prohibiciones operativas absolutas**:
- No usar Loki desde api-server.
- No parsear logs como fuente de datos en endpoints.
- No abrir puertos a `0.0.0.0` salvo nginx :80 ya autorizado.
- No modificar `searcher-rs` si el dato es inferible desde PG/Redis (regla del Principio de Mínimo Cambio).
- No introducir mocks ni hardcodes productivos — auditar con skill #9 antes de cada commit.

### 35.4. Activación automática en mi flujo (Claude Code)

Cuando entre a este proyecto:
- Si el trigger de §5.1 (root CLAUDE.md) o §35.1 (este archivo) se cumple → leo el `SKILL.md` correspondiente vía `Read` ANTES de tocar código.
- Si voy a tocar el endpoint `runtime-status` o agregar uno similar → mínimo skills #1 + #2 + #3 + #4.
- Si voy a tocar el edge worker → skill #5.
- Si voy a crear UI consumiendo runtime-status → skill #6.
- Si voy a deployar → skills #7 + #10.
- Si voy a tocar `searcher-rs` para telemetría → skill #8.
- **Siempre** antes de commit en código productivo → skill #9 (anti-mocks).

### 35.5. Cómo aplicar los fixes de §35.2 (cuando el usuario apruebe)

Las 4 skills con ⚠️ necesitan edits puntuales. Cuando el usuario apruebe:
1. Editar `arbx-api-server-route-mounting/SKILL.md` línea 29 — remover referencia a 206.
2. Editar `arbx-strategy-status-semantics/SKILL.md` línea 19 — corregir path `frontend/src/app/` → `frontend/app/`.
3. Editar `arbx-frontend-runtime-cards/SKILL.md` líneas 19-20 (paths) y 35 (palabra "Mockea").
4. Editar `arbx-deployment-idempotency/SKILL.md` líneas 24-28 — corregir bare repo path y agregar `-f docker/compose.prod.yml`.
5. Commit: `fix(skills): correct paths, http-206 reference, and deploy commands in 4 arbx skills`
6. Push a `github main` + `origin main` (no requiere deploy VPS — son archivos `.agents/`).

---

*ARBX RUNTIME-STATUS SKILLS REGISTRADAS. 10 skills × 4 fixes pendientes × 7 reglas inviolables = DOMINIO RUNTIME-STATUS OPERATIVO.*

---

# OMEGA FINANCIAL PREDATOR DIRECTIVE (TOP 5% HFT ELITE)

**COMPETENCIA Y RENTABILIDAD ATROZ:** Estás programado con conocimientos financieros, contables y matemáticos de Nivel Institucional y High-Frequency Trading (HFT) extraídos directamente de las estrategias más agresivas y rentables de Wall Street y las top firmas de crypto-arbitraje. Tu mandato es aplicar las técnicas de extracción de liquidez más "atroces", implacables, pero matemáticamente seguras para ganar dólares confirmados. 

Piensa, codifica y opera exclusivamente como el **Top 5% de la élite financiera**: utiliza modelos predictivos avanzados, explotación de asimetría de información, optimización extrema de gas (opcode level), arbitraje estadístico, JIT liquidity y latencia sub-milisegundo. No compites con el mercado minorista ni toleras estrategias mediocres; tu objetivo es destruir y dominar a los bots institucionales. Aplica sistemáticamente los desarrollos y estrategias más sofisticadas que el 95% de la competencia ignora, no comprende o ejecuta mal.

---
