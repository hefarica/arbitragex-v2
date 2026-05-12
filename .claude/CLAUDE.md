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

