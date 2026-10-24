# BARRIDO P1/P2 — auditoría de premisas contra el destino (iteración 6 del loop)

Medición ejecutada 2026-10-21 sobre `backend/` del clone (HEAD `af331ce7`). Método: grep con rutas+línea;
sin suposiciones. Este acto responde a que 3 de 5 unidades medidas (REA-04, REA-09, REA-10) resultaron
YA SATISFECHAS: el backlog del REA_MASTER fue escrito contra un estado anterior del repo.

## Tabla

| Unidad | Evidencia medida | Veredicto |
|---|---|---|
| `REA-11-CONNECTORS` (ccxt) | `workers/binance_ws.rs` existe; `workers/price_worker.rs:15-60` documenta sources (Binance WS primario, Chainlink, Coingecko fallback con backoff); `workers/cex_dex_worker.rs`, `funding_rate_worker.rs`, `liquidation_worker.rs`, `jit_v3_worker.rs` presentes | **PARCIAL** — hay venues y normalización básica; falta ccxt-normalizado multi-exchange (Bybit/OKX/etc.). Añadir venues = integración de red (API keys, WS), NO port de matemática → fuera del alcance REA |
| `REA-12-BUNDLE` (mev-boost/mev-share) | `relays-client/src/relay_{flashbots,bloxroute,titan}.rs` (eth_sendBundle), `submit_engine.rs`, `relay_no_submit_sim.rs` (validación sin submit) | **MAYORMENTE PRESENTE** — la inclusión existe (paper/no-submit). El "precio del bundle frente al tip" no aparece como tal (grep tip/bid sin resultados propios) → **PARCIAL** en pricing de inclusión |
| `REA-13-BEST-SOLUTION` (cowprotocol) | `math-engine/src/operators/op_32_multi_objective.rs:14` cita solvers CoW (arXiv:2510.21647); `canonical_enums.rs:81-92` registra `INTENT_AUCTION`; `strategies/classifier.rs:62` nombra `intents_solver_engine` | **SCAFFOLD** — nombres/familias registradas, sin implementación de subasta → gap real pero GRANDE (coordinación off-chain) |
| `REA-14-SIGNED-ORDER` (1inch/UniswapX/Seaport) | CERO matches de `signed_order\|intent_order\|limit_order\|uniswapx\|seaport` en todo `backend/` | **GAP REAL** — capacidad genuinamente ausente; portable como matemática EIP-712 + fill amounts con vectores del upstream |
| `REA-15-NO-TRANSFER` (whack-a-mole) | `searcher-rs/src/engines/cex_dex_engine.rs:1-17` — "Hybrid CEX-DEX Convergence… Requires use of proprietary inventory… Self-Rebalancing"; `canonical_enums.rs:77` CEX_DEX; `detector_policy.rs:140-156` | **PRESENTE** — el invariante (inventario a ambos lados, sin transferir fondos) ya está en el engine → premisa stale |
| `REA-16-MEV-CLASS` (mev-inspect-rs) | `math-engine/src/strategies/classifier.rs:83` clasifica IDs `MEV-XX-YYY` (familias de estrategia), NO trazas de tx; `recon/src/drift_tracker.rs` = clasificación de estado de evidencia, no de MEV | **GAP REAL (analítica)** — falta clasificación por tipo sobre trazas. Fuente ARCHIVADA y SIN LICENCIA → solo semántica; vectores propios |
| `REA-17-LIQUIDATION` (aave-v3-core) | `engines/liquidation_engine.rs` + `liquidation_snipe_engine.rs` + `workers/liquidation_worker.rs` + `detector_policy` + `cartridge_boot.rs` (22 refs) | **PRESENTE** — motor de liquidación existe; el health-factor/close-factor exactos ya están (los usa el tool `loan_liq` del harness) → premisa mayormente stale |
| `REA-18-V3-SIM` (revm + v3-core) | Deriva de `REA-03-WIRE`: sin slot0 en `RouteLeg`/`StateProjector`, la sim V3 exacta no tiene insumos | **GAP REAL BLOQUEADO** — depende de la plomería de datos de REA-03-WIRE |

## Decisión de selección (siguiente unidad)

1. **REA-14-SIGNED-ORDER** — primer gap real con matemática portable y vectores citables
   (EIP-712 order hash + getMakingAmount/getTakingAmount con su semántica de redondeo).
   Fuente autorizada en §3 del master; licencias a verificar en el sha.
2. `REA-13` y `REA-16` quedan como candidatas siguientes (grande la primera; analítica la segunda).
3. `REA-18` permanece bloqueada por REA-03-WIRE (FOLLOWUP ya registrado).
4. `REA-11/12/15/17` NO se re-implementan: presentes o fuera de alcance REA (integración de red).

## Nota de fase

La puerta de fase P0 (`status='executed'`) sigue en manos de GATE1 (equipo, integrador-2 en vuelo);
este barrido no la toca.
