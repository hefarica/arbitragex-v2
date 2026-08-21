# MACRO PLAN — Migración Paralela ethers-rs → alloy (Dual-Track Toggleable)

> **Modo:** Plan macro / solo lectura · **Fecha:** 2026-08-18 · **NO ejecutar**
> **Objetivo:** Construir un segundo camino completo con alloy SIN tocar el camino ethers que corre en producción, con toggle end-to-end manejable desde el frontend.

---

## 1. Estado Actual (mapa exacto verificado)

### 1.1 Doble dependencia ya existente (la migración empezó pero no terminó)

| Crate | ethers-rs | alloy | Nota |
|---|---|---|---|
| shared-rs | ✅ | ✅ | Solo 1 import ethers (chains.rs); rpc_failover YA usa alloy |
| searcher-rs | ✅ (abigen, rustls) | ✅ | **59 archivos** importan ethers; el hot-path completo |
| relays-client | ✅ | ✅ | Signer + bundle building usan ethers |
| sim-ctl | ✅ | ❌ | Providers + tipos ethers |
| simulator-v2 | ✅ | ✅ (solo primitives) | revm-based, ethers solo para tipos |
| sim-core | ✅ | ❌ | Tipos ethers |
| prioritization-spine | ✅ | ❌ | Tipos ethers |
| recon | ✅ | ✅ | Ya tiene ambos |
| mcp-sim-engine | ✅ | ❌ | Providers ethers |

### 1.2 Qué usa cada crate de ethers (por categoría)

| Categoría | ethers-rs API | Usado en | Reemplazo alloy |
|---|---|---|---|
| **Tipos** | Address, U256, H256, Bytes | TODOS (59 archivos en searcher-rs) | alloy::primitives::{Address, U256, Bytes} |
| **Provider HTTP** | Provider::new(Http::new(url)) | shared-rs, sim-ctl, mcp-sim | alloy::providers::RootProvider |
| **Provider WS** | Provider::new(WebSocket::new(url)) | searcher-rs (scanner, mempool) | alloy::providers::WsConnect + RootProvider |
| **Signer** | LocalWallet::from_str(key) | relays-client, shared-rs | alloy::signers::local::PrivateKeySigner |
| **Contract calls** | Contract::new(addr, abi, provider) | searcher-rs (univ2, univ3, reserve_reader) | alloy::contract::Call |
| **Event listening** | provider.subscribe_logs() | searcher-rs (mempool_listener, block_scanner) | alloy::providers::WsConnect + subscription |
| **Transaction building** | TransactionRequest::new() | relays-client (bundle_builder) | alloy::rpc::types::TransactionRequest |
| **ABI encoding** | ethers::abi::encode / decode | searcher-rs (route_decoder, universal_router) | alloy::sol_types::SolValue / SolCall |

### 1.3 Por qué ethers causa 7 RUSTSEC advisories

```
ethers-rs 2.0.14 (deprecated desde 2024-Q4)
  ├── ethers-providers 2.0.14
  │     ├── reqwest 0.11.27 (deprecated)
  │     │     ├── rustls 0.21.12 → rustls-webpki 0.101.7 ← 3 advisories HIGH
  │     │     └── rustls-pemfile 1.0.4 ← 1 unmaintained
  │     └── jsonwebtoken 8.3.0
  │           └── ring 0.16.20 ← 2 advisories (unmaintained + AES panic)
  ├── ethers-middleware 2.0.14
  │     └── instant ← 1 unmaintained
  └── hashers ← fxhash ← 1 unmaintained
```

**La cadena completa muere al eliminar ethers-rs.**

---

## 2. Arquitectura del Dual-Track

### 2.1 Principio: trait + dos implementaciones + toggle runtime

```
                    ┌─────────────────────────────────────────┐
                    │           TRAIT (contrato neutral)       │
                    │                                         │
                    │  trait RpcBackend {                     │
                    │      fn get_block_number()              │
                    │      fn get_reserves(pool)              │
                    │      fn subscribe_events()              │
                    │      fn sign_and_send(tx)               │
                    │  }                                      │
                    └─────────────┬─────────────┬─────────────┘
                                  │             │
                    ┌─────────────┴───┐   ┌─────┴─────────────┐
                    │  EthersBackend  │   │  AlloyBackend     │
                    │  (production)   │   │  (nuevo camino)   │
                    │                 │   │                   │
                    │  ethers-rs      │   │  alloy-rs         │
                    │  2.0.14         │   │  1.x              │
                    └─────────────────┘   └───────────────────┘
                              │                     │
                              └──────────┬──────────┘
                                         │
                    ┌────────────────────┴────────────────────┐
                    │         RUNTIME TOGGLE (Redis)           │
                    │                                         │
                    │  arbx:rpc_backend = "ethers"           │
                    │                     "alloy"            │
                    │                     "shadow"           │
                    │                                         │
                    │  Frontend → /api/config → Redis        │
                    └─────────────────────────────────────────┘
```

### 2.2 Los 4 traits a definir (en shared-rs, crate nuevo `rpc-bridge`)

```rust
// ── Trait 1: Provider (lectura RPC) ──
#[async_trait]
pub trait RpcReader: Send + Sync {
    async fn get_block_number(&self) -> Result<u64>;
    async fn get_chain_id(&self) -> Result<u64>;
    async fn get_reserves_v2(&self, pool: Address) -> Result<(U256, U256)>;
    async fn get_slot0_v3(&self, pool: Address) -> Result<Slot0>;
    async fn eth_call(&self, to: Address, data: Bytes) -> Result<Bytes>;
}

// ── Trait 2: Subscriber (eventos WebSocket) ──
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn subscribe_pending_txs(&self) -> Result<Receiver<PendingTx>>;
    async fn subscribe_new_blocks(&self) -> Result<Receiver<NewBlock>>;
    async fn subscribe_logs(&self, filter: LogFilter) -> Result<Receiver<Log>>;
}

// ── Trait 3: Signer (firma, SOLO relays-client) ──
#[async_trait]
pub trait TransactionSigner: Send + Sync {
    fn address(&self) -> Address;
    async fn sign_transaction(&self, tx: TransactionRequest) -> Result<SignedTx>;
    async fn sign_message(&self, msg: &[u8]) -> Result<Signature>;
}

// ── Trait 4: Coder (ABI encode/decode) ──
pub trait AbiCoder: Send + Sync {
    fn encode_call(&self, selector: [u8; 4], params: &[DynSolValue]) -> Bytes;
    fn decode_response(&self, data: &Bytes, output_types: &[TokenType]) -> Vec<DynSolValue>;
}
```

### 2.3 Toggle mechanism (patrón ya probado: paper/live mode §34)

```
Redis key: arbx:rpc_backend:<service>
Valores:
  "ethers"  → camino A (production, default)
  "alloy"   → camino B (nuevo)
  "shadow"  → AMBOS corren, comparar outputs, log diferencias

Frontend:
  /config → toggle UI (como paper/live switch)
  → PUT /api/admin/rpc-backend
  → Redis SET arbx:rpc_backend:searcher-rs "alloy"
  → searcher-rs re-lee en el próximo tick (o hot-reload)

Hot-reload: cada servicio re-lee el toggle cada 30s (o pub/sub en Redis)
Kill-switch: si alloy backend falla → auto-fallback a ethers + alerta
```

---

## 3. Fases del Plan (orden por riesgo ascendente)

### FASE 0 — Crate `rpc-bridge` (infraestructura de abstracción)
**Riesgo: CERO** (solo añade código nuevo, no toca nada existente)

- Crear `backend/shared-rs/src/rpc_bridge/` con:
  - Los 4 traits (RpcReader, EventSubscriber, TransactionSigner, AbiCoder)
  - Tipo neutral `RpcContext` que encapsula URL, chain_id, timeouts
  - Enum `BackendSelection { Ethers, Alloy, Shadow }` + lector de Redis
  - `BackendFactory::create(selection, config) -> Arc<dyn RpcReader>`
- **NO** tocar ningún archivo existente

**Archivos nuevos:** ~8 archivos, ~500 líneas
**Archivos tocados:** 0 (solo `mod.rs` añade `pub mod rpc_bridge`)

---

### FASE 1 — AlloyBackend (implementación del segundo camino)
**Riesgo: BAJO** (código nuevo, compila en paralelo, no se ejecuta en producción)

- Implementar los 4 traits con alloy:
  - `alloy_reader.rs` → RpcReader con alloy::providers
  - `alloy_subscriber.rs` → EventSubscriber con alloy WsConnect
  - `alloy_signer.rs` → TransactionSigner con alloy::signers
  - `alloy_coder.rs` → AbiCoder con alloy::sol_types
- Tests unitarios contra Anvil fork (mismo que usa sim-ctl)
- **NO** conectar a producción aún

**Archivos nuevos:** ~6 archivos, ~800 líneas
**Dependencia:** alloy ya está en Cargo.toml de shared-rs

---

### FASE 2 — EthersBackend (adaptador del camino existente)
**Riesgo: BAJO** (envuelve código existente, no lo modifica)

- Implementar los 4 traits envolviendo las llamadas ethers existentes:
  - `ethers_reader.rs` → delega a las funciones ethers que YA funcionan
  - `ethers_subscriber.rs` → envuelve provider.subscribe_logs()
  - etc.
- Esto crea una capa de indirección pero NO cambia el comportamiento
- El camino ethers sigue siendo el DEFAULT

**Archivos nuevos:** ~6 archivos, ~600 líneas

---

### FASE 3 — Shadow Mode (comparación A/B)
**Riesgo: MEDIO** (ejecuta alloy en paralelo pero NO afecta decisiones)

- Cuando `BackendSelection::Shadow`:
  - El request va a AMBOS backends
  - El resultado de ETHERS es el que se usa (production)
  - El resultado de ALLOY se compara y se loggea
  - Diferencias > threshold → alerta
- Telemetría: `rpc_backend_comparison{field, ethers_val, alloy_val, delta}`
- Acumular 7 días de datos de comparación antes de cambiar el default

**Métricas a comparar:**
- Block number (debe ser idéntico)
- Reserves V2 (debe ser idéntico, hasta el último wei)
- Slot0 V3 (debe ser idéntico)
- Latencia de respuesta (alloy debería ser ≤ ethers)
- Errores de red (deben ser equivalentes)

**Archivos nuevos:** ~3 archivos (comparator, metrics, alerting)
**Archivos tocados:** 0 (el shadow mode es aditivo)

---

### FASE 4 — Toggle Frontend + API
**Riesgo: BAJO** (solo UI + endpoint de config)

- Endpoint: `PUT /api/admin/rpc-backend` (admin-gated, como paper/live)
- Redis: `arbx:rpc_backend:<service>` con TTL de expiración (safety)
- Frontend: toggle en /config/settings (al lado de paper/live switch)
- Auto-fallback: si alloy backend falla 3 veces → auto-switch a ethers + alerta
- Audit log: todo cambio de backend queda registrado (quién, cuándo, por qué)

**Archivos nuevos:** ~4 (API route, frontend toggle component, auto-fallback logic)
**Patrón a seguir:** exactamente el mismo que `usePaperModeState` (§34)

---

### FASE 5 — Migración por crate (menos crítico → más crítico)
**Riesgo: ESCALONADO** (cada paso es independiente y reversible)

**Orden de migración (por riesgo ascendente):**

| Paso | Crate | ethers usage | Riesgo | Rollback |
|---|---|---|---|---|
| 5.1 | mcp-sim-engine | Providers (stdio local) | Mínimo | git revert |
| 5.2 | sim-core | Solo tipos | Mínimo | git revert |
| 5.3 | prioritization-spine | Solo tipos | Bajo | git revert |
| 5.4 | sim-ctl | Providers + tipos | Bajo | git revert |
| 5.5 | recon | Ya tiene ambos | Bajo | git revert |
| 5.6 | simulator-v2 | Solo tipos (revm ya es alloy-based) | Bajo | git revert |
| 5.7 | relays-client | **Signer + broadcast** | **ALTO** (§34) | toggle a ethers |
| 5.8 | shared-rs | 1 import (chains.rs) | Medio | toggle a ethers |
| 5.9 | searcher-rs | **59 archivos, hot-path 24/7** | **MÁXIMO** | toggle a ethers |

**Para cada paso:**
1. Cambiar los imports `use ethers::*` → `use alloy::*`
2. Cambiar tipos (Address, U256 son compatibles en alloy-primitives)
3. Cambiar provider calls
4. Correr TODOS los tests
5. Deploy con toggle en "ethers" (sin cambio visible)
6. Toggle a "shadow" por 24h
7. Toggle a "alloy" (cambio activo)
8. Monitorear 24h
9. Si todo bien → siguiente crate

---

### FASE 6 — Cleanup (eliminar ethers)
**Riesgo: CERO** (una vez que todo corre en alloy)

- Remover `ethers = { workspace = true }` de los 9 Cargo.toml
- Remover los EthersBackend adapters
- Remover `ethers-rs` del workspace Cargo.toml
- **7 RUSTSEC advisories desaparecen automáticamente**
- cargo audit pasa sin ignores (o con muchos menos)

---

## 4. Ventajas de este enfoque

### 4.1 Ventajas inmediatas (desde FASE 3)

| Ventaja | Impacto |
|---|---|
| **A/B comparison real** | Verificar que alloy produce EXACTAMENTE los mismos resultados que ethers en producción |
| **Performance data** | Medir latencia ethers vs alloy en condiciones reales (alloy promete 2-5x más rápido) |
| **Confidence building** | 7 días de shadow mode = confianza estadística antes de switch |
| **Kill-switch instantáneo** | Si algo sale mal, toggle de vuelta a ethers en <30s |

### 4.2 Ventajas estructurales (a largo plazo)

| Ventaja | Impacto |
|---|---|
| **7 RUSTSEC advisories eliminados** | cargo audit pasa limpio sin el treadmill de la allowlist |
| **ethers-rs eliminado** | La cadena completa de deps deprecated muere (reqwest 0.11, rustls 0.21, ring 0.16, jsonwebtoken 8, etc.) |
| **Cargo.lock más limpio** | Menos deps transitivas = builds más rápidos, binarios más pequeños |
| **alloy es el futuro** | Alineado con reth/revm/foundry ecosystem (los mantenedores de alloy son los de Paradigm/Foundry) |
| **Mejor type safety** | alloy-primitives tiene tipos más fuertes que ethers-types |
| **Zero-copy decoding** | alloy usa bytes VIEW para decodificar (sin allocaciones) → hot-path más rápido |
| **Mantenimiento activo** | alloy tiene releases mensuales; ethers-rs está archived |

### 4.3 Ventajas del toggle frontend

| Ventaja | Impacto |
|---|---|
| **Operador en control** | El operador ve el estado del backend en el dashboard y puede switchear sin SSH |
| **Transición gradual** | Puedes tener searcher-rs en ethers mientras relays-client ya usa alloy |
| **Per-service granularity** | Cada servicio tiene su propio toggle (no un switch global) |
| **Audit trail** | Todo cambio queda registrado (quién, cuándo, resultado) |
| **Demo capability** | Puedes mostrar el toggle funcionando (como paper/live) |

---

## 5. Riesgos y mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|
| alloy produce resultado diferente | Media | Alto (reserves incorrectas) | Shadow mode 7 días antes de switch |
| alloy WebSocket inestable | Baja | Alto (detección se cae) | Auto-fallback a ethers + alerta |
| alloy signer incompatible | Media | Crítico (§34) | Toggle relays-client independiente, NO migrar hasta verificación exhaustiva |
| Performance regression | Baja | Medio | Benchmarks en shadow mode |
| Breaking alloy API (major bump) | Media | Medio | Pin alloy versión, upgrade conscientemente |
| Cargo workspace conflict (ethers+alloy coexisten) | Baja | Bajo | Ya coexisten hoy (verificado en Cargo.lock) |

---

## 6. Estimación de effort

| Fase | Tiempo | Complejidad |
|---|---|---|
| FASE 0: rpc-bridge traits | 2-3 días | Baja (diseño de traits) |
| FASE 1: AlloyBackend | 3-5 días | Media (alloy API learning curve) |
| FASE 2: EthersBackend | 2-3 días | Baja (envolver existente) |
| FASE 3: Shadow Mode | 2-3 días | Media (comparación + alerting) |
| FASE 4: Toggle Frontend | 2-3 días | Baja (patrón ya existente) |
| FASE 5: Migración (9 crates) | 2-4 semanas | Variable (searcher-rs es la pesada) |
| FASE 6: Cleanup | 1 día | Trivial |
| **TOTAL** | **4-7 semanas** | |

---

## 7. Dependencia con otros trabajos

| Trabajo actual | Interacción | Nota |
|---|---|---|
| PR #405 (cargo audit fix) | **Sin conflicto** | #405 es el desbloqueo inmediato; este plan es el fix estructural |
| RU-1 a RU-6 (Universo de Rutas) | **Sin conflicto** | Los cartuchos/workers no cambian — solo la capa RPC underneath |
| §IV motor matemático | **Sin conflicto** | El motor opera sobre RouteIntent, no sobre types de ethers/alloy |
| A1 route_metadata | **Sin conflicto** | route_metadata es PG, no ethers |
| A2 Executor deploy | **Sin conflicto** | El Executor se despliega igual en ambos caminos |

---

## 8. Conclusión

El enfoque dual-track con toggle es **la arquitectura correcta** para esta migración porque:

1. **Zero riesgo en producción** — el camino ethers nunca se toca hasta que alloy está verificado
2. **Evidencia antes de confianza** — shadow mode provee datos A/B reales
3. **Control del operador** — toggle desde el frontend, como paper/live
4. **Elimina 7 RUSTSEC advisories** en la raíz, no con patches
5. **Cada paso es reversible** — el toggle permite volver a ethers en segundos

**El esfuerzo (4-7 semanas) es significativo pero la alternativa (seguir manteniendo la allowlist de 19+ RUSTSEC ignores que crece cada semana) es un treadmill infinito.**
