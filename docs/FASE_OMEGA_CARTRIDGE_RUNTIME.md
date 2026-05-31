# FASE OMEGA — Dynamic Strategy Cartridge Runtime

## Resumen Ejecutivo

El sistema de cartuchos dinámicos de ArbitrageX v2 implementa la arquitectura **"PlayStation MEV"**: un motor de scripting Rhai sandboxeado que permite desplegar, actualizar y remover lógica de estrategia en tiempo real sin recompilar el binario Rust.

**Soporte Multi-Chain Universal**: Los cartuchos son completamente agnósticos a la cadena. Funcionan en cualquier EVM-compatible (Ethereum, Arbitrum, Base, Polygon, BSC, Avalanche, Optimism, zkSync, Linea, Scroll, o cualquier chain futura) sin modificación alguna.

---

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         STRATEGY FORGE (UI)                                  │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  ┌───────────────┐  │
│  │ Code Editor │  │ Test Console │  │ Chain Selector│  │ Deploy Button │  │
│  └──────┬──────┘  └──────┬───────┘  └───────┬───────┘  └───────┬───────┘  │
└─────────┼────────────────┼───────────────────┼───────────────────┼──────────┘
          │                │                   │                   │
          ▼                ▼                   ▼                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         API SERVER (Node.js)                                 │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ POST /api/v1/cartridges → Validate → PG Insert → Redis Mirror → Pub  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
          │                                    │
          ▼                                    ▼
┌──────────────────┐              ┌────────────────────────────────────────────┐
│   PostgreSQL     │              │              Redis                          │
│  (Source of      │              │  arbx:cartridge:source:<slug>  (source)    │
│   Truth)         │              │  arbx:cartridge:injection      (pub/sub)   │
│                  │              │  arbx:cartridge:ack             (ack)       │
│  cartridge_      │              │  arbx:cartridge:signals         (signals)  │
│  registry        │              │  arbx:cartridge:telemetry       (logs)     │
└──────────────────┘              └────────────────────────────────────────────┘
                                               │
                                               ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SEARCHER-RS (Rust Binary)                                  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    CartridgeSubscriber                                 │  │
│  │  Redis PubSub → Deserialize Event → Fetch Source → Load Cartridge    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                              │                                               │
│                              ▼                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    CartridgeRunner                                     │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────────┐  │  │
│  │  │ Rhai Engine │  │ AST Registry │  │ Host Bindings (Peripherals)│  │  │
│  │  │ (Sandboxed) │  │ HashMap<ID,  │  │ get_reserves()             │  │  │
│  │  │ max_ops: 1M │  │  CompiledAST>│  │ get_token_meta()           │  │  │
│  │  │ max_arr: 4K │  │              │  │ get_pool_index()           │  │  │
│  │  │ max_str: 64K│  │              │  │ simulate_swap()            │  │  │
│  │  │ max_map: 1K │  │              │  │ get_base_fee()             │  │  │
│  │  │ no imports  │  │              │  │ get_block_number()         │  │  │
│  │  │ no eval()   │  │              │  │ log_quantum()              │  │  │
│  │  └─────────────┘  └──────────────┘  │ emit_signal()             │  │  │
│  │                                      │ math_*()                  │  │  │
│  │                                      │ to_wei() / from_wei()     │  │  │
│  │                                      └────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                              │                                               │
│                              ▼                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    Orchestrator Integration                            │  │
│  │  For each pending tx → For each active cartridge → evaluate()        │  │
│  │  If is_opportunity → build_payload() → Emit to execution pipeline    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Contrato Universal del Cartucho

Todo archivo `.rhai` DEBE exportar estas tres funciones para ser considerado un cartucho válido:

| Función | Parámetros | Retorno | Propósito |
|---------|-----------|---------|-----------|
| `init_strategy()` | Ninguno | `Map` | Metadata: nombre, versión, autor, chains |
| `evaluate_opportunity(pool_data)` | `Map` con datos de pool | `Map` con resultado | Lógica core de detección |
| `build_payload(opportunity)` | `Map` con oportunidad | `Map` con payload tx | Ensamblaje de ejecución |

### Hooks Opcionales (Lifecycle)

| Hook | Parámetros | Propósito |
|------|-----------|-----------|
| `on_activate()` | Ninguno | Llamado al activar el cartucho |
| `on_deactivate()` | Ninguno | Llamado al pausar el cartucho |
| `on_new_block(block_number)` | `i64` | Llamado en cada nuevo bloque |

---

## Soporte Multi-Chain

### Diseño Chain-Agnostic

Los cartuchos **NUNCA** hardcodean direcciones, parámetros de gas, o configuraciones específicas de una cadena. En su lugar:

1. **`get_chain_id()`** — El host inyecta el chain_id actual.
2. **`get_reserves(pool_addr)`** — Resuelve automáticamente por chain.
3. **`get_token_meta(addr)`** — Resuelve automáticamente por chain.
4. **`get_pool_index(sym_a, sym_b)`** — Resuelve automáticamente por chain.
5. **`get_base_fee()`** — Gas oracle de la chain actual.
6. **`get_block_number()`** — Bloque actual de la chain.

### target_chains

En `init_strategy()`, el campo `target_chains` controla en qué cadenas se activa el cartucho:

```rhai
fn init_strategy() {
    #{
        // ...
        target_chains: [],        // TODAS las chains (universal)
        // target_chains: [1, 42161],  // Solo Ethereum y Arbitrum
    }
}
```

**Cuando se agrega una nueva chain al sistema**, todos los cartuchos con `target_chains: []` se activan automáticamente en ella sin intervención manual.

### Chains Soportadas (y cualquier futura)

| Chain | ID | Block Time | Notas |
|-------|-----|-----------|-------|
| Ethereum | 1 | ~12s | L1 principal |
| Arbitrum One | 42161 | ~0.25s | L2 Optimistic |
| Base | 8453 | ~2s | L2 OP Stack |
| Polygon PoS | 137 | ~2s | Sidechain |
| BSC | 56 | ~3s | Alt-L1 |
| Avalanche C-Chain | 43114 | ~2s | Alt-L1 |
| Optimism | 10 | ~2s | L2 OP Stack |
| zkSync Era | 324 | ~1s | L2 ZK |
| Linea | 59144 | ~3s | L2 ZK |
| Scroll | 534352 | ~3s | L2 ZK |
| *Cualquier EVM futura* | *N* | *Variable* | *Auto-soportada* |

---

## Host Bindings (Periféricos)

### Infraestructura (Redis/RPC)

| Binding | Latencia | Descripción |
|---------|----------|-------------|
| `get_reserves(pool_addr)` | <1ms | Reservas del pool desde Redis cache |
| `get_token_meta(token_addr)` | <1ms | Metadata del token (symbol, decimals) |
| `get_pool_index(sym_a, sym_b)` | <1ms | Lista de pools para un par de tokens |
| `simulate_swap(amount, path)` | 5-50ms | Simulación de swap (cached) |

### Estado de Chain

| Binding | Latencia | Descripción |
|---------|----------|-------------|
| `get_base_fee()` | <1ms | Base fee actual en gwei |
| `get_block_number()` | <1ms | Número de bloque actual |
| `get_timestamp()` | <1μs | Unix timestamp actual |
| `get_chain_id()` | <1μs | Chain ID del host |

### Telemetría

| Binding | Latencia | Descripción |
|---------|----------|-------------|
| `log_quantum(level, msg)` | fire&forget | Log a Redis telemetry channel |
| `emit_signal(type, data)` | fire&forget | Señal a Arteria PubSub |

### Matemáticas (Pure, sin I/O)

| Binding | Descripción |
|---------|-------------|
| `math_sqrt(x)` | Raíz cuadrada |
| `math_abs(x)` | Valor absoluto |
| `math_min(a, b)` | Mínimo |
| `math_max(a, b)` | Máximo |
| `math_pow(base, exp)` | Potencia |
| `math_log(x)` | Logaritmo natural |
| `math_exp(x)` | Exponencial |
| `to_wei(amount, decimals)` | Conversión a wei |
| `from_wei(wei_str, decimals)` | Conversión desde wei |

---

## Sandboxing y Seguridad

### Límites de Ejecución

| Parámetro | Valor | Propósito |
|-----------|-------|-----------|
| `max_operations` | 1,000,000 | Previene loops infinitos |
| `max_call_stack_depth` | 64 | Previene recursión infinita |
| `max_string_size` | 65,536 bytes | Previene bombas de memoria |
| `max_array_size` | 4,096 elementos | Previene bombas de memoria |
| `max_map_size` | 1,024 entradas | Previene bombas de memoria |
| `max_modules` | 0 | Sin imports externos |
| `eval()` | Deshabilitado | Sin ejecución dinámica |

### Modelo de Seguridad

1. **Sin acceso a filesystem** — Rhai no tiene I/O de archivos.
2. **Sin acceso a red** — Solo a través de host bindings controlados.
3. **Sin acceso a procesos** — No puede ejecutar comandos.
4. **Read-only por defecto** — Host bindings solo leen datos.
5. **Rate limiting** — Simulaciones RPC limitadas por cartucho.
6. **Fail-safe** — Un cartucho que falla NUNCA crashea el host.
7. **Auto-pause** — Cartuchos con errores consecutivos se pausan.

---

## Flujo de Inyección End-to-End

### 1. Operador escribe cartucho en Strategy Forge UI

```rhai
fn init_strategy() {
    #{ name: "Mi Estrategia", version: "1.0.0", ... }
}
fn evaluate_opportunity(pool_data) { ... }
fn build_payload(opportunity) { ... }
```

### 2. API Server recibe y persiste

```
POST /api/v1/cartridges
{
  "slug": "mi_estrategia",
  "source_code": "...",
  "target_chains": []  // Todas las chains
}
```

### 3. Redis PubSub notifica a todos los nodos

```json
{
  "cartridge_id": "mi_estrategia",
  "event_type": "inject",
  "content_hash": "sha256...",
  "chain_id": 0,
  "timestamp": "2026-05-31T12:00:00Z"
}
```

### 4. CartridgeSubscriber en cada searcher-rs

```
1. Recibe evento PubSub
2. Verifica dedup (content_hash)
3. Fetch source de Redis: arbx:cartridge:source:mi_estrategia
4. Compila AST con Rhai Engine
5. Valida contrato (3 funciones requeridas)
6. Ejecuta init_strategy() → extrae metadata
7. Almacena en HashMap<String, CompiledCartridge>
8. Publica ACK en arbx:cartridge:ack
```

### 5. Evaluación en el hot-path

```
Para cada tx pendiente en mempool:
  Para cada cartucho activo:
    Si target_chains vacío O contiene chain_id actual:
      resultado = evaluate_opportunity(pool_data)
      Si resultado.is_opportunity:
        payload = build_payload(resultado)
        Emitir a pipeline de ejecución
```

---

## Estructura de Archivos

```
backend/searcher-rs/
├── src/
│   ├── cartridge/
│   │   ├── mod.rs              # Módulo principal + docs
│   │   ├── types.rs            # Tipos compartidos
│   │   ├── contract.rs         # Validación del contrato universal
│   │   ├── runner.rs           # Motor Rhai + registro de cartuchos
│   │   ├── host_bindings.rs    # Funciones nativas expuestas a Rhai
│   │   └── subscriber.rs       # Redis PubSub hot-reload listener
│   ├── cartridge_loader.rs     # Carga desde filesystem (dev/boot)
│   └── lib.rs                  # (módulos declarados)
├── cartridges/
│   ├── dex_arb.rhai            # Cartucho maestro: DEX arbitrage
│   ├── triangular_arb.rhai     # Cartucho: arbitraje triangular
│   └── liquidation.rhai        # Cartucho: liquidaciones
└── tests/
    └── cartridge_e2e_test.rs   # Tests end-to-end

backend/api-server/src/routes/
└── cartridge-forge.ts          # API REST para gestión de cartuchos

database/migrations/
└── 090_cartridge_registry.sql  # Schema de persistencia
```

---

## API REST — Cartridge Forge

| Método | Ruta | Descripción |
|--------|------|-------------|
| `POST` | `/api/v1/cartridges` | Inyectar nuevo cartucho |
| `PUT` | `/api/v1/cartridges/:slug` | Actualizar cartucho existente |
| `DELETE` | `/api/v1/cartridges/:slug` | Archivar y remover cartucho |
| `GET` | `/api/v1/cartridges` | Listar cartuchos (filtro por chain/state) |
| `GET` | `/api/v1/cartridges/:slug` | Detalle de un cartucho |
| `POST` | `/api/v1/cartridges/:slug/pause` | Pausar cartucho |
| `POST` | `/api/v1/cartridges/:slug/resume` | Resumir cartucho |
| `POST` | `/api/v1/cartridges/:slug/test` | Dry-run de evaluación |

### Filtrado por Chain

```
GET /api/v1/cartridges?chain_id=42161
```

Retorna solo cartuchos que:
- Tienen `target_chains` vacío (universales), O
- Incluyen `42161` en su `target_chains`

---

## Cómo Crear un Nuevo Cartucho

### Template Mínimo

```rhai
fn init_strategy() {
    #{
        name: "Mi Estrategia",
        version: "1.0.0",
        author: "mi_equipo",
        description: "Descripción de lo que hace",
        category: "custom",
        target_chains: [],  // [] = todas las chains
        min_eval_interval_ms: 100
    }
}

fn evaluate_opportunity(pool_data) {
    let chain_id = get_chain_id();
    // ... lógica de detección ...
    #{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "none"
    }
}

fn build_payload(opportunity) {
    #{
        target_contract: "EXECUTOR",
        calldata: "0x...",
        value_wei: "0",
        gas_limit: 300000,
        max_priority_fee_gwei: get_base_fee() * 1.5,
        deadline_ts: get_timestamp() + 30
    }
}
```

### Mejores Prácticas

1. **Nunca hardcodear direcciones** — Usar `get_pool_index()` y `get_token_meta()`.
2. **Siempre verificar datos** — Los host bindings pueden retornar `()` (null).
3. **Gas-aware** — Siempre comparar profit contra costo de gas.
4. **Logging** — Usar `log_quantum()` para debugging.
5. **Confidence scoring** — Retornar confianza entre 0.0 y 1.0.
6. **Urgency levels** — "immediate", "next_block", "monitor".
7. **Fail gracefully** — Retornar `is_opportunity: false` ante datos faltantes.

---

## Testing

### Compilación y Validación (sin Redis)

```bash
cd backend/searcher-rs
cargo test --test cartridge_e2e_test -- --nocapture
```

### Tests Incluidos

| Test | Qué Valida |
|------|------------|
| `test_valid_cartridge_compiles_and_validates` | Compilación de dex_arb.rhai |
| `test_triangular_arb_compiles` | Compilación de triangular_arb.rhai |
| `test_liquidation_compiles` | Compilación de liquidation.rhai |
| `test_init_strategy_returns_valid_metadata` | Metadata correcta |
| `test_evaluate_opportunity_no_pools` | Manejo graceful sin pools |
| `test_infinite_loop_terminated` | Protección contra loops |
| `test_memory_bomb_prevented` | Límite de arrays |
| `test_string_bomb_prevented` | Límite de strings |
| `test_multi_chain_agnostic` | Funciona en 10 chains distintas |
| `test_missing_function_detected` | Validación de contrato |
| `test_wrong_arity_detected` | Validación de aridad |
| `test_content_hash_dedup` | Deduplicación por hash |

---

## Métricas y Observabilidad

### Prometheus Metrics (planificadas)

| Métrica | Tipo | Descripción |
|---------|------|-------------|
| `cartridge_evaluations_total` | Counter | Total de evaluaciones por cartucho |
| `cartridge_opportunities_total` | Counter | Oportunidades detectadas |
| `cartridge_errors_total` | Counter | Errores de ejecución |
| `cartridge_eval_duration_ms` | Histogram | Latencia de evaluación |
| `cartridge_active_count` | Gauge | Cartuchos activos |

### Redis Telemetry Channels

| Canal | Contenido |
|-------|-----------|
| `arbx:cartridge:telemetry` | Logs de `log_quantum()` |
| `arbx:cartridge:signals` | Señales de `emit_signal()` |
| `arbx:cartridge:ack` | ACKs de carga/error |
| `arbx:cartridge:injection` | Eventos de inyección |

---

## Roadmap

- [x] Motor Rhai sandboxeado con host bindings
- [x] Contrato Universal validado en compilación
- [x] Hot-reload via Redis PubSub
- [x] Cartuchos maestros (DEX arb, Triangular, Liquidation)
- [x] Soporte multi-chain universal
- [x] API REST completa (Cartridge Forge)
- [x] Persistencia PostgreSQL con audit log
- [x] Tests end-to-end
- [ ] Integración con orchestrator hot-path
- [ ] Strategy Forge UI (editor + deploy)
- [ ] Prometheus metrics per-cartridge
- [ ] V3 concentrated liquidity host binding
- [ ] Flashloan routing host binding
- [ ] Cartridge marketplace (compartir entre operadores)
