# Reference Repos Blueprint — Arquitectura Mental para ArbitrageX v2

> **Versión**: 1.0 — generado 2026-05-12 tras directriz del usuario.
> **Propósito**: Estudiar 6 repos públicos como **referencia arquitectónica**, no como fuente para copiar código. Producir un mapa explícito de qué patrones tomar, dónde aplicarlos en ArbitrageX, qué NO copiar, y un plan de refactor incremental que no rompa lo actual.
> **Restricciones honradas**: cero cambios de código en este deliverable, cero modificación del frontend, cero contacto con VPS, cero deploy, cero direcciones/secretos/RPCs copiados.
> **Quien aplica esto**: cualquier sesión futura de Claude/Gemini que vaya a tocar `backend/searcher-rs/`, `backend/api-server/`, `edge/worker/` con intención de modularizar o introducir nuevos patrones.

---

## 1. Resumen ejecutivo

ArbitrageX v2 ya implementa los conceptos clave (mempool scanning, engines por estrategia, paper trading, runtime status), pero su `backend/searcher-rs/` es **estructuralmente monolítico** — todo vive en archivos al mismo nivel de `src/` (28 archivos `.rs`) con una subcarpeta `engines/`. La arquitectura mental de Paradigm Artemis (Collectors → Strategies → Executors orquestados por un Engine event-driven) ofrece el patrón canónico para escalar a más fuentes de detección y más caminos de ejecución sin convertir `scanner.rs` en un dios-objeto.

Las 6 referencias se distribuyen en 4 capas distintas del sistema:

| Capa ArbitrageX | Repo referencia primario | Repo referencia secundario |
|------------------|---------------------------|------------------------------|
| Motor MEV (Rust event loop + traits) | `paradigmxyz/artemis` | — |
| Pipeline arbitraje + bundle submission | `flashbots/simple-arbitrage` | — |
| Private orderflow / MEV-Share | `flashbots/mev-share-client-ts` | — |
| Backend service discipline + DB-as-truth | `cowprotocol/services` | — |
| Pool sync + swap simulation | `darkforestry/amms-rs` (sucesor de `0xKitsune/cfmms-rs` archivado) | — |
| Frontend monorepo organization | `Uniswap/interface` (solo arquitectura, no diseño) | — |

**Conclusión rectora**: si solo se pudiera escoger UNO como referencia mental #1, sería **Artemis** — porque resuelve "ordenar un motor event-driven multi-estrategia", que es el problema actual de ArbitrageX. La #2 sería **CoW services**, porque define disciplina seria para Rust + PostgreSQL + API horizontal-scalable.

---

## 2. Tabla maestra: repo → patrón → archivos ArbitrageX

| # | Repo | Lenguaje | Patrón útil | Aplica a (paths actuales) | Aplica a (paths target) |
|---|------|----------|-------------|---------------------------|--------------------------|
| 1 | `paradigmxyz/artemis` | Rust | Collector/Strategy/Executor traits + Engine con channels tokio | `backend/searcher-rs/src/scanner.rs`, `engines/*`, `opportunity_emitter.rs`, `orchestrator.rs` (mixed today) | `backend/searcher-rs/src/{collectors,strategies,execution,types,telemetry}/` |
| 2 | `flashbots/simple-arbitrage` | TS/Sol | Pipeline `discover → evaluate → rate → submit` | `engines/dex_engine.rs` (detect), `impact_index.rs` (score), `opportunity_emitter.rs` (paper) | `backend/searcher-rs/src/strategies/dex_arb/{discover,evaluate,rate}.rs` |
| 3 | `flashbots/mev-share-client-ts` | TS | `MevShareClient` (on/sendTransaction/sendBundle/simulateBundle) + hints model (`logs`/`calldata`/`functionSelector`/`contractAddress`/`txHash`) | (no existe hoy en ArbitrageX) | `backend/searcher-rs/src/collectors/private_hint_collector.rs` + `types/detection_source.rs` |
| 4 | `cowprotocol/services` | Rust | Crate workspace con `orderbook`/`autopilot`/`driver`/`contracts`/`database`/`shared`/`ethrpc`/`model`/`number`/`observe`/`e2e`/`testlib`. PG como source of truth. Orderbook horizontal-scalable, autopilot singleton. | `backend/api-server/` (TS, no Rust) y `backend/searcher-rs/` (mezclado) | Promover separación API/solver en api-server-rs futuro; replicar disciplina de validación/migrations |
| 5 | `darkforestry/amms-rs` (NO `cfmms-rs` que está archivado desde 2023-12-27) | Rust | Soporte UniV2/UniV3/Balancer/ERC4626 con sync + swap simulation | `backend/searcher-rs/src/{reserves.rs, pool_discovery.rs, amm_math.rs, impact_index.rs}` (implementación propia hoy) | Considerar amms-rs como dependencia opcional; o seguir su modelo de traits localmente |
| 6 | `Uniswap/interface` | TS (Bun monorepo) | Monorepo `apps/`/`packages/`/`config/` para Web/Mobile/Extension. Build con Bun. **NO**: design tokens, UI components | `frontend/` (single Next.js app) | NO refactorizar frontend salvo orden explícita. Si se añade mobile/extension futuro, este es el patrón. |

---

## 3. Análisis por repo

### 3.1 `paradigmxyz/artemis` — Blueprint #1

**Licencia**: Dual Apache-2.0 + MIT (permisivo, compatible con uso comercial y derivados privados).
**Cuándo es relevante**: Cuando se vaya a modularizar `searcher-rs` para soportar múltiples Collectors (mempool, block, oracle, private hints) y múltiples Executors (paper, private bundle, public broadcast) bajo un solo Engine.

**Arquitectura conceptual**:

```text
[Collector]──events──┐
[Collector]──events──┼──►[Engine (channels mpsc)]──►[Strategy]──actions──┐
[Collector]──events──┘                              [Strategy]──actions──┼──►[Executor]
                                                    [Strategy]──actions──┘   [Executor]
```

**Contratos a internalizar (sin copiar el código)**:
- `Collector<E>` trait: `async fn get_event_stream(&self) -> CollectorStream<'_, E>`.
- `Strategy<E, A>` trait: `async fn process_event(&mut self, event: E) -> Vec<A>` con estado mutable opcional.
- `Executor<A>` trait: `async fn execute(&self, action: A) -> Result<()>`.
- `Engine` orquesta `Vec<Box<dyn Collector>>`, `Vec<Box<dyn Strategy>>`, `Vec<Box<dyn Executor>>` con `tokio::sync::broadcast` para event fan-out.

**Mapeo concreto a ArbitrageX**:

| Artemis | ArbitrageX actual (mezclado) | ArbitrageX target |
|---------|------------------------------|-------------------|
| Collector mempool | `scanner.rs` (escucha WS Alchemy) | `collectors/mempool_collector.rs` |
| Collector block | (inferido en `scanner.rs`) | `collectors/block_collector.rs` |
| Collector private hint | (no existe) | `collectors/private_hint_collector.rs` (futuro MEV-Share) |
| Collector oracle/price | (no existe) | `collectors/oracle_collector.rs` (Chainlink/CEX) |
| Strategy dex_arb | `engines/dex_engine.rs` | `strategies/dex_arb/mod.rs` |
| Strategy triangular | `engines/triangular_engine.rs` | `strategies/triangular_arb/mod.rs` |
| Strategy flashloan | `engines/flashloan_engine.rs` | `strategies/flashloan_arb/mod.rs` |
| Strategy liquidation | `engines/liquidation_engine.rs` | `strategies/liquidation/mod.rs` |
| Executor paper | `opportunity_emitter.rs` (parcial) | `execution/paper_emitter.rs` |
| Executor simulator | (revm acoplado dentro de engines) | `execution/simulator_executor.rs` |
| Executor private bundle | `relays-client` (servicio separado) | `execution/relay_executor.rs` |
| Engine | `orchestrator.rs` (parcial) | `core/engine.rs` |

**Qué NO copiar de Artemis**:
- La única strategy de ejemplo (`opensea-sudoswap-arb`) y todo lo NFT.
- `bin/` y `examples/` (son demos con direcciones específicas).
- Smart contracts ejemplo en Solidity (25% del repo).
- Cualquier hardcoded address o RPC.

**Limitaciones del análisis**: El README de Artemis no expone trait signatures completos. Para extraerlas con precisión hay que mirar `crates/artemis-core/src/types.rs` directamente — no se hizo en este blueprint para evitar el riesgo de "auto-copiar" código sin licencia.

---

### 3.2 `flashbots/simple-arbitrage` — Flujo MEV de referencia

**Licencia**: ⚠️ **No declarada explícitamente en el repo** (LICENSE file → 404). El README no menciona licencia. **Asumir restrictivo: solo estudiar patrones, NO copiar código**.

**Patrón útil**: Pipeline declarativo `discover → evaluate → rate → submit`.

**Mapeo conceptual**:

| Stage simple-arbitrage | ArbitrageX hoy | Notas |
|------------------------|-----------------|-------|
| `discover` | `scanner.rs` + `engines/*::detect()` | Multi-source ya existe |
| `evaluate` | `engines/*::evaluate()` + `revm` sim | OK |
| `rate` | `impact_index.rs` + opt sizing | OK, mejorable |
| `submit` | `relays-client` (separado) o `opportunity_emitter.rs` (paper) | Disciplinar handoff |

**Warning crítico del propio repo**: "very unlikely to be profitable, as many users have access to it, and it is targeting well-known Ethereum opportunities". Es decir, **el repo es educativo, no productivo**.

**Qué NO copiar**:
- `contracts/BundleExecutor.sol` (los Solidity contracts) — la doctrina ArbitrageX exige diseñar `ArbitrageExecutor.sol` propio, ya existe en `contracts/`.
- Direcciones de WETH, tokens, factories.
- Thresholds de profit hardcoded.
- Private keys o flow de bot wallet (ArbitrageX usa Flashbots Protect/private mempool).

**Riesgo de licencia**: ALTO si se copia código. **CERO si solo se internalizan los 4 nombres de stage** como estructura conceptual.

---

### 3.3 `flashbots/mev-share-client-ts` — Private orderflow

**Licencia**: ⚠️ **No declarada explícitamente en el repo** (LICENSE file → 404). Flashbots históricamente usa Apache-2.0 en proyectos relacionados, pero **no está confirmado aquí** — verificar antes de cualquier copia.

**Contrato de integración a internalizar (no copiar la implementación TS)**:

| API | Propósito | Aplica a ArbitrageX |
|-----|-----------|---------------------|
| `client.on("transaction", cb)` | Suscribirse a pending tx stream | `collectors/private_hint_collector.rs` |
| `client.sendTransaction(signed, opts)` | Enviar tx privada con privacy hints | (futuro) `execution/private_tx_executor.rs` |
| `client.sendBundle(params)` | Bundle multi-tx con ordering | `relays-client` (ya existe) |
| `client.simulateBundle(params)` | Simulación pre-submit | `sim-ctl` (ya existe) |

**Modelo de hints (privacy preferences)** — qué exponer al builder:
- `logs` (eventos)
- `calldata` (argumentos de función)
- `functionSelector` (4 bytes)
- `contractAddress` (destino)
- `txHash` (identificador)

**Tipos de evento**: `IPendingTransaction` y `IPendingBundle` con hint-control granular.

**Aplicación target en ArbitrageX**:

```rust
// backend/searcher-rs/src/types/detection_source.rs (futuro)
pub enum DetectionSource {
    PublicMempool,
    NewBlock,
    MevSharePendingTx,    // ← este repo
    MevShareBundleHint,   // ← este repo
    OracleUpdate,
    PrivateRpcHint,
}
```

**Qué NO copiar**:
- Direcciones de relays.
- API keys.
- Implementación TypeScript cruda (ArbitrageX es Rust).
- Builders preferidos hardcoded.

**Riesgo de licencia**: ALTO si se copia código. BAJO si solo se internaliza el modelo de hints + nombres de eventos.

---

### 3.4 `cowprotocol/services` — Backend discipline Rust + PostgreSQL

**Licencia**: Triple Apache-2.0 / GPL-3.0 / MIT (elegir Apache-2.0 o MIT para compatibilidad con uso comercial).
**Cuándo es relevante**: Cuando se vaya a portar `api-server` (hoy Node.js TypeScript) a Rust, o cuando se diseñe un futuro `solver-rs` separado del searcher.

**Crate workspace observado**:

| Crate CoW | Propósito | Equivalente ArbitrageX (actual o futuro) |
|-----------|-----------|------------------------------------------|
| `orderbook` | HTTP API + persistence + validation | `api-server` (TS hoy, podría ser Rust futuro) |
| `autopilot` | Auction driver, settlement competition manager | Posiblemente `orchestrator.rs` lifted al nivel de servicio |
| `driver` | Solver con co-location externa | `relays-client` o futuro `executor-svc` |
| `contracts` | Alloy bindings de smart contracts | `crates/contracts-bindings` (futuro) |
| `database` | Shared persistence layer | `crates/persistence` (futuro) — hoy disperso en `persistence.rs` |
| `shared` | Cross-service utilities | `crates/shared-rs` (ya existe parcialmente) |
| `ethrpc` | RPC layer | `chain_client.rs` lifted a crate |
| `model` | Serialization types | `models.rs` lifted a crate |
| `number` | Numeric types (fixed-point, big-num) | `amm_math.rs` parcialmente |
| `observe` | Metrics/tracing | `metrics.rs` + `counters.rs` |
| `e2e`, `testlib` | Integration testing | (no existe sistemático hoy) |

**Patrones de disciplina backend que se deben replicar**:
- **PostgreSQL como source of truth** para `opportunities`, `recon`, `strategy_scores`. ArbitrageX ya lo hace parcialmente.
- **Validación on-chain antes de exponer** (ej. order validity ≈ opportunity viability).
- **Migrations versionadas + tests con DB real** (no mocks).
- **Multiple orderbook instances escalando horizontalmente** + singleton autopilot. ArbitrageX podría correr múltiples `api-server` detrás de nginx y un único `orchestrator`.
- **OpenAPI spec para API contract**. ArbitrageX tiene `docs/API_CONTRACTS.md` — podría formalizarse a OpenAPI YAML.

**Service Separation Pattern**:
- `orderbook` (read/write API público) ↔ `autopilot` (escritor de auctions) ↔ `driver` (solver/searcher consume).
- Aplicado a ArbitrageX: `api-server` (read API) ↔ `searcher-rs` (escritor de opportunities) ↔ `relays-client`/`sim-ctl` (consumidores).

**Qué NO copiar**:
- Lógica de solver de CoW (es para batch auctions, no aplica MEV-arb directo).
- Schemas de orders específicos de CoW.
- Contracts de Vault/Settlement de CoW.
- Cualquier address de mainnet/Gnosis Chain.

**Riesgo de licencia**: BAJO bajo Apache-2.0 o MIT. Pero respetar atribución si se copia algo verbatim.

---

### 3.5 `darkforestry/amms-rs` — Pool sync + swap simulation

**Nota crítica**: `0xKitsune/cfmms-rs` está **archivado desde 2023-12-27**. El sucesor es `amms-rs`. **Usar amms-rs como referencia, no cfmms-rs**.

**Licencia**: ⚠️ **No declarada en README**. Verificar archivo LICENSE en el repo antes de cualquier dependencia o copia.

**Soporte declarado**: UniswapV2 ✅ / UniswapV3 ✅ / Balancer ✅ / ERC4626 Vaults ✅.

**Patrones útiles**:
- Trait `AMM` (o equivalente) que abstrae sync + swap simulation por tipo de pool.
- Factory pattern para descubrir pools nuevos en una cadena.
- State sync con `get_reserves`/`slot0` por tipo.

**Decisión recomendada para ArbitrageX**:

**Opción A — Usar amms-rs como dependencia** (recomendada si licencia es permisiva):
- Eliminar `amm_math.rs`, parte de `reserves.rs`, parte de `pool_discovery.rs`.
- Ganar mantenimiento upstream + correctness validada por la comunidad.

**Opción B — Mantener implementación propia** (actual):
- ArbitrageX ya tiene `amm_math.rs`, `reserves.rs`, `pool_discovery.rs` funcionando.
- Si amms-rs no aporta features adicionales necesarias (e.g., Curve, Solidly), no migrar.
- Riesgo: divergencia de correctness con upstream.

**Qué internalizar como concepto, sin importar la decisión**:
- Trait `Pool` unificado para V2/V3/Balancer.
- Separación clara `sync()` (lee state on-chain) vs `simulate()` (ejecuta swap localmente).
- Factory abstraction para descubrir pools nuevos por chain.

**Qué NO copiar**:
- Direcciones de factories (V2 factory, V3 factory).
- Fee tiers hardcoded.
- Pool addresses específicas.
- ABIs si el proyecto tiene ABIs duplicadas/divergentes.

---

### 3.6 `Uniswap/interface` — Frontend monorepo (NO redesign)

**Licencia**: ⚠️ **No declarada en README, LICENSE root → 404**. Históricamente Uniswap usa **BUSL-1.1 (Business Source License)** o **GPL-2.0** en algunos sub-paquetes — **muy restrictivo para uso comercial derivado**.

**Patrón observado**:
- Monorepo Bun con `apps/{web,mobile,extension}/` + `packages/` (compartido) + `config/` (build/lint).
- 97.4% TypeScript. Apps móvil/extensión coexisten con web.

**Aplicación a ArbitrageX**:

**Aplica HOY**: NADA. ArbitrageX tiene un único frontend Next.js 14. No hay mobile/extension.

**Aplica MAÑANA (si se decide)**: SOLO si se decide añadir mobile companion app o browser extension para alertas. Ese día, este es el modelo de organización monorepo.

**🛑 PROHIBICIÓN EXPLÍCITA** (per directiva del usuario "FRONTEND FREEZE PROTOCOL"):
- NO refactorizar frontend para monorepo sin orden explícita.
- NO copiar componentes UI.
- NO copiar design tokens.
- NO copiar layout/styling.
- NO copiar hooks de wallet/network (ArbitrageX no tiene wallet connection en UI — usa admin tokens server-side).

**Qué se PODRÍA estudiar (solo lectura, no copia)**:
- Cómo organizan dependencies entre apps y packages compartidos.
- Build/configuration con Bun (ArbitrageX usa npm workspaces + Next, no Bun).
- Naming conventions de packages.

**Riesgo de licencia**: **ALTO** si se copia cualquier componente. BUSL-1.1 prohíbe uso comercial competitivo durante 4 años desde publicación.

---

## 4. Estructura target propuesta (vista 360°)

```text
backend/
  searcher-rs/                          ← REFACTOR PROPUESTO (NO ejecutar hasta plan aprobado)
    src/
      core/
        engine.rs                       ← Engine event-driven (Artemis-inspired)
        lib.rs
        main.rs
      collectors/                       ← NUEVO (hoy mezclado en scanner.rs)
        mod.rs
        mempool_collector.rs            ← era parte de scanner.rs
        block_collector.rs              ← era parte de scanner.rs
        private_hint_collector.rs       ← FUTURO (MEV-Share)
        oracle_collector.rs             ← FUTURO (Chainlink/CEX feeds)
      types/                            ← NUEVO (hoy mezclado en models.rs)
        mod.rs
        event_envelope.rs               ← canonical event type
        route_intent.rs                 ← ya existe en route_intent.rs
        detection_source.rs             ← NUEVO enum (PublicMempool/MevShareHint/…)
        candidate.rs                    ← ya existe en engines/candidate.rs
      strategies/                       ← MOVE desde engines/
        mod.rs
        strategy_trait.rs
        strategy_registry.rs
        dex_arb/
          mod.rs                        ← era engines/dex_engine.rs
          discover.rs
          evaluate.rs
          rate.rs
        triangular_arb/
          mod.rs                        ← era engines/triangular_engine.rs
        flashloan_arb/
          mod.rs                        ← era engines/flashloan_engine.rs
        liquidation/
          mod.rs                        ← era engines/liquidation_engine.rs
      simulation/                       ← NUEVO grouping
        mod.rs
        state_projector.rs              ← era parte de reserves.rs + impact_index.rs
        size_optimizer.rs               ← era parte de engines/*
        post_state_simulator.rs         ← era parte de engines/*
      execution/                        ← NUEVO grouping
        mod.rs
        paper_emitter.rs                ← era opportunity_emitter.rs
        bundle_builder.rs               ← FUTURO
        relay_executor.rs               ← interface con relays-client
      observability/                    ← MOVE desde metrics.rs + counters.rs
        mod.rs
        heartbeat.rs                    ← FUTURO crate
        runtime_status.rs               ← writer side (api-server lee)
        metrics.rs                      ← era metrics.rs
        counters.rs                     ← era counters.rs
      persistence/                      ← MOVE desde persistence.rs
        mod.rs                          ← era persistence.rs
        opportunities.rs
        observations.rs
      onchain/                          ← MOVE
        chain_client.rs                 ← era chain_client.rs
        reserves.rs                     ← era reserves.rs
        pool_discovery.rs               ← era pool_discovery.rs
        impact_index.rs                 ← era impact_index.rs
        amm_math.rs                     ← era amm_math.rs (o reemplazar con amms-rs)
        calldata/                       ← era calldata/ (sin cambios)
        route_decoder.rs                ← era route_decoder.rs

  api-server/                           ← MANTENER TS por ahora (CoW pattern aplica si se porta a Rust)
    src/
      routes/                           ← ya existe
      services/                         ← ya existe
      schemas/                          ← ya existe

edge/
  worker/                               ← MANTENER (sin cambios)
  dev-local/                            ← MANTENER (sin cambios)

frontend/                               ← 🚫 NO TOCAR (FRONTEND FREEZE PROTOCOL)
```

---

## 5. Gap analysis: estado actual vs target

| Capa target | Existencia actual | Gap |
|-------------|--------------------|-----|
| `collectors/mempool_collector.rs` | Existe parcialmente como `scanner.rs` | Falta extraer como trait + struct dedicado |
| `collectors/block_collector.rs` | Mezclado en `scanner.rs` | Extraer del scanner |
| `collectors/private_hint_collector.rs` | **NO existe** | Implementación futura (MEV-Share) |
| `collectors/oracle_collector.rs` | **NO existe** | Implementación futura (Chainlink/CEX feeds) |
| `types/event_envelope.rs` | Disperso en `models.rs` | Consolidar |
| `types/detection_source.rs` | **NO existe** | Crear enum |
| `strategies/dex_arb/` | `engines/dex_engine.rs` (un solo file) | Sub-modular: discover/evaluate/rate |
| `strategies/triangular_arb/` | `engines/triangular_engine.rs` | Sub-modular |
| `strategies/flashloan_arb/` | `engines/flashloan_engine.rs` | Sub-modular |
| `strategies/liquidation/` | `engines/liquidation_engine.rs` + `lending_position_indexer.rs` | Consolidar bajo strategy folder |
| `simulation/state_projector.rs` | Lógica mezclada en `engines/*` + `reserves.rs` | Extraer |
| `simulation/size_optimizer.rs` | Mezclado en engines | Extraer |
| `execution/paper_emitter.rs` | `opportunity_emitter.rs` | Rename + move |
| `execution/bundle_builder.rs` | **NO existe productivo** | Futuro |
| `execution/relay_executor.rs` | Existe en servicio `relays-client` separado | Interface adapter en searcher |
| `observability/runtime_status.rs` | Writer side **NO existe**, reader side existe en api-server | Crear writer en searcher |
| `persistence/` | `persistence.rs` flat | Sub-modular |
| `onchain/*` | Disperso en root de `src/` | Grouping |

**Total**: ~70% del target ya existe pero está mal organizado; ~30% es nuevo (private hints, oracle collector, sub-modularización).

---

## 6. Plan de refactor incremental (propuesta — NO ejecutar sin aprobación)

> **Principio rector**: ningún refactor debe romper el pipeline live (`searcher-rs` está produciendo oportunidades reales hoy). Cada fase debe pasar `cargo check + clippy + test` antes de la siguiente, y dejar el binario funcional.

### Fase 1 (1-2 días) — Grouping sin renombrar archivos

**Objetivo**: reorganizar carpetas SIN cambiar nombres ni lógica.

- Crear `src/onchain/` y mover `chain_client.rs`, `reserves.rs`, `pool_discovery.rs`, `impact_index.rs`, `amm_math.rs`, `calldata/`, `route_decoder.rs`.
- Crear `src/observability/` y mover `metrics.rs`, `counters.rs`.
- Crear `src/persistence/` y mover `persistence.rs` adentro (rename a `mod.rs`).
- Actualizar `lib.rs` y todos los `use` statements.
- Verificar: `cargo check && cargo test && cargo clippy`.
- Deploy shadow mínimo 10 min para confirmar comportamiento idéntico.

**Riesgo**: BAJO. Es solo reorganización física. Reversible con `git revert`.

### Fase 2 (2-3 días) — Engines → Strategies con sub-modularización

**Objetivo**: renombrar `engines/` a `strategies/` y dividir cada engine en discover/evaluate/rate.

- `engines/dex_engine.rs` → `strategies/dex_arb/{mod.rs, discover.rs, evaluate.rs, rate.rs}`.
- Idem triangular, flashloan, liquidation.
- Mantener API pública sin cambios (orchestrator sigue llamando los mismos métodos).
- Verificar `cargo check + clippy + test`.

**Riesgo**: MEDIO. Cambia paths de imports. Reversible pero más trabajoso.

### Fase 3 (1 semana) — Extraer Collector trait + traits Strategy/Executor

**Objetivo**: introducir traits Artemis-style sin romper el orchestrator existente.

- Crear `core/engine.rs` con traits `Collector<E>`, `Strategy<E,A>`, `Executor<A>`.
- Implementar `Collector` para mempool/block (extraer de scanner.rs).
- Implementar `Strategy` para cada strategy actual (wrap).
- Implementar `Executor` para paper emitter.
- `orchestrator.rs` se convierte en thin wrapper que registra implementaciones en el Engine.
- Verificar bit-for-bit que el output de oportunidades es idéntico antes/después.

**Riesgo**: ALTO. Cambio estructural mayor. Requiere paper trading shadow >24h para validar.

### Fase 4 (futuro) — Nuevos Collectors

- `private_hint_collector.rs` con MEV-Share client (Rust port o FFI a mev-share-client-ts).
- `oracle_collector.rs` con feeds Chainlink + CEX WS.
- Cada uno aporta nuevos eventos al Engine sin tocar strategies existentes.

**Riesgo**: BAJO (additive). Pero cada nuevo collector necesita su propio paper-trade validation.

### Fase 5 (futuro) — Migración api-server TS → Rust (opcional)

- Si y solo si hay carga real que lo justifique. Hoy api-server TS funciona.
- Aplicar disciplina CoW: workspace crates, OpenAPI, migrations versionadas, e2e tests.

**Riesgo**: ALTO. Requiere reescribir ~10 routes + schemas + middleware. Sin beneficio claro en performance hoy.

---

## 7. License risk matrix

| Repo | LICENSE file | License | Riesgo si se copia código | Acción |
|------|--------------|---------|---------------------------|--------|
| `paradigmxyz/artemis` | ✅ Apache-2.0 + MIT (dual) | Permisivo | **BAJO** — atribución + copy NOTICE | OK copiar bajo condiciones; preferir patrones |
| `flashbots/simple-arbitrage` | ❌ **404 no existe** | **DESCONOCIDO** | **ALTO** — sin licencia, todo derechos reservados por defecto | NO copiar código. Solo internalizar nombres de stages |
| `flashbots/mev-share-client-ts` | ❌ **404 no existe** | **DESCONOCIDO** | **ALTO** — sin licencia explícita | NO copiar. Solo internalizar API contract (nombres de método + tipos de evento) |
| `cowprotocol/services` | ✅ Apache-2.0 + GPL-3.0 + MIT (triple) | Permisivo (Apache/MIT) | **BAJO** bajo Apache/MIT | OK copiar con atribución |
| `darkforestry/amms-rs` | ⚠️ No verificado en README | **DESCONOCIDO** | **MEDIO** — verificar antes de dependencia | Check LICENSE file directamente antes de añadir como dep |
| `Uniswap/interface` | ❌ **404 en root** (parts BUSL-1.1 historically) | **RESTRICTIVO** (BUSL/GPL en sub-paquetes) | **ALTO** — BUSL prohíbe uso comercial competitivo | NO copiar código. Solo internalizar patrón de organización monorepo |

**Doctrina derivada**:
1. **Repos sin LICENSE explícita = "todos los derechos reservados"** por defecto. NO copiar.
2. **Apache-2.0 / MIT** = libre con atribución. OK con NOTICE file.
3. **GPL-3.0** = obliga liberar derivados bajo GPL. **NO compatible con ArbitrageX productivo cerrado**.
4. **BUSL-1.1** = prohíbe uso comercial competitivo por 4 años. **NO usar en ArbitrageX**.
5. **Si la licencia no es verificable**, internalizar solo PATRONES (nombres, flow, conceptos) — nunca código verbatim.

---

## 8. Anti-patterns a evitar (compilado de las 6 referencias)

Cosas que **explícitamente NO deben aparecer** en ArbitrageX bajo ningún motivo:

| Anti-pattern | Origen | Por qué |
|--------------|--------|---------|
| Hardcoded addresses (WETH, USDC, factory) | simple-arbitrage, Uniswap | Doctrina No-Hardcode (RULE 00) ArbitrageX |
| Direcciones de relay específicas en código | mev-share-client | Deben venir de `.env` o config |
| Hardcoded thresholds de profit | simple-arbitrage | Configurable via Redis trading_config |
| Single-strategy bot (solo NFT arb) | Artemis examples | ArbitrageX es multi-strategy por diseño |
| BundleExecutor.sol genérico | simple-arbitrage | ArbitrageX usa `ArbitrageExecutor.sol` propio |
| UI components copiados | Uniswap interface | Frontend Freeze Protocol |
| Wallet connection en UI | Uniswap interface | ArbitrageX usa admin tokens server-side |
| Pool addresses hardcoded en searcher | cfmms-rs examples | Discovery dinámico vía RPC |
| Mocks/fixtures en producción | (todos los repos en sus `examples/`) | Doctrina Zero-Mocks ArbitrageX |
| API keys en repo | (todos) | `.env` only, gitignore obligatorio |
| Private keys en repo | simple-arbitrage notably | NUNCA. Vault/secret manager |

---

## 9. Constraints honradas en este blueprint

- ✅ Cero cambios de código en este deliverable (solo doc nuevo en `docs/architecture/`).
- ✅ Cero modificaciones al frontend (Frontend Freeze Protocol respetado).
- ✅ Cero contacto con VPS (no SSH, no deploy, no rebuild).
- ✅ Cero direcciones / secretos / RPCs copiados.
- ✅ Cero contratos copiados sin licencia revisada.
- ✅ Cero copia de código frontend.
- ✅ Licencias revisadas explícitamente con resultado documentado (3 de 6 sin LICENSE en raíz, flagged).
- ✅ Refactor propuesto en fases reversibles, sin obligar nada.

---

## 10. Glosario rápido

- **Collector**: componente que escucha el mundo externo (mempool, block, hint privado, oracle) y emite eventos internos al Engine.
- **Strategy**: componente que consume eventos del Engine, detecta oportunidades y emite acciones.
- **Executor**: componente que ejecuta acciones (paper, simulated, real bundle, broadcast público).
- **Engine**: orquestador event-driven que cablea Collectors, Strategies, Executors con channels async.
- **DetectionSource**: enum que identifica de qué Collector originó una oportunidad (clave para auditoría, recon, atribución de profit).
- **Hint**: pieza de información parcial sobre una pending tx (calldata, function selector, contract addr) — modelo de MEV-Share.

---

## 11. Próximos pasos sugeridos (decisión del operador)

1. **Si solo se quiere documentar**: este blueprint queda como referencia. No requiere acción.
2. **Si se quiere empezar refactor**: ejecutar Fase 1 (grouping sin renombrar) en un branch dedicado con shadow paper-trade de 10 min antes de merge.
3. **Si se quiere evaluar amms-rs como dependencia**: primero verificar archivo LICENSE en el repo. Si es permisivo (MIT/Apache), considerar PoC reemplazando `amm_math.rs`.
4. **Si se quiere agregar private hints (MEV-Share)**: implementar `collectors/private_hint_collector.rs` Rust-native (sin depender del TS client), respetando hints model documentado en §3.3.
5. **Si se quiere portar api-server a Rust**: este blueprint sirve como guía (sección 3.4), pero el ROI debe justificarse — TS funciona hoy.

---

*Blueprint generado bajo directiva explícita: "estudia, compara, documenta y propone; no toques frontend, no toques VPS, no copies direcciones, no deployes nada."*
