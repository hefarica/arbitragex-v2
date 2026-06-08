# Integración de Módulos OMEGA — ArbitrageX v2
**Fecha de integración:** 2026-05-22  
**Sprint:** S2 — Extensión de estrategias  
**Estado:** Phase 1 (Scaffold) — Engines y Workers registrados

---

## Resumen de módulos integrados

Los siguientes tres módulos de estrategia han sido integrados en la arquitectura del `searcher-rs`:

| Módulo | Clasificación | Archivo Engine | Archivo Worker | Migración DB |
|--------|--------------|----------------|----------------|--------------|
| Backrunning No-Extractivo | ✅ PERMITIDO | `engines/backrun_engine.rs` | `workers/backrun_worker.rs` | `073_strategy_catalog_new_modules.sql` |
| Sincronización Cross-Pool | ✅ PERMITIDO | `engines/spatial_engine.rs` | `workers/spatial_worker.rs` | `073_strategy_catalog_new_modules.sql` |
| Convergencia CEX-DEX | ✅ PERMITIDO | `engines/cex_dex_engine.rs` | `workers/cex_dex_worker.rs` (existente, extendido) | `073_strategy_catalog_new_modules.sql` |

---

## Módulo 1 — Backrunning No-Extractivo (Zero Victim)

### Eigen-State
Un usuario ha ejecutado un swap masivo en un AMM. Su transacción está finalizada y su precio de liquidación está bloqueado. Sin embargo, el swap dejó la curva AMM (x·y=k) en un estado de desequilibrio temporal frente al mercado global.

### Objetivo
Capturar el arbitraje residual **estrictamente después** de que el usuario original ha completado su operación. Zero Front-Running.

### Mecánica de ejecución
1. El orquestador detecta el cambio de estado confirmado en el mempool/bloque.
2. Inyecta una transacción ordenada matemática y temporalmente **justo detrás** de la transacción objetivo.
3. Ejecuta un reverse-swap o ruteo hacia otro pool para capturar el diferencial.

### Condición de fallo (Fail-Honest)
Si el motor de simulación (`revm` en `sim-ctl`) detecta que nuestra transacción alteraría el precio de ejecución del usuario objetivo de **cualquier manera**, el bundle se descarta inmediatamente.

### Parámetros configurables
| Parámetro | Valor por defecto | Descripción |
|-----------|------------------|-------------|
| `min_profit_usd` | 5.0 | Profit mínimo para emitir candidato |
| `max_gas_price_gwei` | 150.0 | Gas máximo para no front-runear |
| `zero_victim_check` | true | Activar verificación de impacto cero |

### Fases de implementación
- **Phase 1 (actual):** Engine + Worker scaffold. Detecta eigen-states, logs candidatos, NO emite al pipeline.
- **Phase 2:** Wiring real con `arbx:impact_events` Redis stream + verificación zero-victim en `sim-ctl`.
- **Phase 3:** Bundle composition con target_tx + our_tx vía `relays-client`.

---

## Módulo 2 — Sincronización Topológica Cross-Pool

### Eigen-State
Divergencia de precios natural entre dos reservas de liquidez aisladas (ej. Uniswap V3 vs Sushiswap V2) debido a la asincronía del flujo de órdenes orgánico.

### Objetivo
Restaurar la paridad de precio entre múltiples manifolds de liquidez sin interactuar ni interferir con transacciones pendientes de usuarios minoristas.

### Mecánica de ejecución
1. Monitoreo constante de factory contracts y pares de liquidez en memoria.
2. Al detectar un ΔPrice que supere los umbrales de gas burn, se ejecuta un flashloan.
3. Compra atómica en el pool deprimido y venta simultánea en el pool sobrecalentado.
4. Repago del préstamo y cristalización del margen en la misma transacción.

### Parámetros configurables
| Parámetro | Valor por defecto | Descripción |
|-----------|------------------|-------------|
| `min_profit_usd` | 10.0 | Profit mínimo para emitir candidato |
| `min_divergence_bps` | 15 | Divergencia mínima en basis points (0.15%) |
| `flashloan_enabled` | true | Usar flashloan para amplificar capital |

### Fases de implementación
- **Phase 1 (actual):** Engine + Worker scaffold. Lee precios de Redis, evalúa divergencias, logs candidatos.
- **Phase 2:** Wiring real con `amm_math::v3_quote_exact_in_multicall` para cálculo óptimo de input.
- **Phase 3:** Construcción de calldata Yul optimizado para minimizar gas en el bundle.

---

## Módulo 3 — Convergencia Híbrida CEX-DEX

### Eigen-State
El precio off-chain (CEX: Binance/Bybit) diverge del precio on-chain (DEX) en una magnitud explotable.

### Objetivo
Arbitrar el diferencial macroeconómico usando infraestructura propietaria. Ningún usuario on-chain se ve afectado. El sistema actúa como Market Maker orgánico.

### Mecánica de ejecución
1. El sistema de telemetría ingiere el Order Book de Nivel 2 del CEX vía WebSocket.
2. El orquestador Rust evalúa el estado del DEX localmente.
3. Al confirmarse el arbitraje: orden de mercado en CEX (Short) + orden atómica en DEX (Long).
4. Neutralización de exposición direccional y aseguramiento del spread.

### Prerrequisitos operacionales
- Inventario propio (self-rebalancing) pre-fijado en los exchanges.
- Reconciliación de PnL en tiempo real.
- Circuit Breakers estrictos para caídas de API CEX o latencia RPC excesiva.

### Parámetros configurables
| Parámetro | Valor por defecto | Descripción |
|-----------|------------------|-------------|
| `min_profit_usd` | 15.0 | Profit mínimo para emitir candidato |
| `min_spread_bps` | 20 | Spread mínimo en basis points (0.20%) |
| `cex_provider` | `binance` | Proveedor CEX principal |
| `circuit_breaker_enabled` | true | Activar circuit breaker |
| `emit_candidates` | false | Phase 1: NO emite (requiere inventario real) |

### Fases de implementación
- **Phase 1 (actual):** Engine scaffold. Evalúa spreads, logs, NO emite (requiere inventario real).
- **Phase 2:** WebSocket subscription a Binance/OKX `bookTicker`. Wiring DEX quoter.
- **Phase 3:** Construcción de Opportunity completa + emisión al pipeline cuando inventario disponible.

---

## Arquitectura de integración

```
mempool_listener
      │
      ▼
  ImpactSet
      │
      ├──► BackrunEngine ──► BackrunWorker ──► [Phase 2: pipeline]
      │
      ├──► SpatialEngine ──► SpatialWorker ──► [Phase 2: pipeline]
      │
      └──► CexDexEngine  ──► CexDexWorker  ──► [Phase 3: pipeline]
                                                      │
                                              OpportunityEmitter
                                                      │
                                              Redis + PostgreSQL
                                                      │
                                              relays-client
                                                      │
                                              Flashbots/bloXroute
```

---

## Criterios de promoción Phase 1 → Phase 2

Para promover cualquier módulo de `scaffold` a `live`, el operador debe verificar:

1. El engine pasa todos los tests unitarios con datos reales de mainnet.
2. El worker se conecta correctamente al Redis stream y procesa eventos sin errores.
3. El `sim-ctl` valida la condición de fallo (zero-victim para backrun, atomicidad para spatial).
4. El profit neto esperado supera el umbral mínimo en al menos 100 oportunidades simuladas.
5. El operador actualiza `lifecycle_status = 'live'` en `strategy_catalog` vía API admin.

---

## Referencias

- `backend/searcher-rs/src/engines/backrun_engine.rs`
- `backend/searcher-rs/src/engines/spatial_engine.rs`
- `backend/searcher-rs/src/engines/cex_dex_engine.rs`
- `backend/searcher-rs/src/workers/backrun_worker.rs`
- `backend/searcher-rs/src/workers/spatial_worker.rs`
- `database/migrations/073_strategy_catalog_new_modules.sql`
- `docs/governance/DATA-MATRIX.md`
