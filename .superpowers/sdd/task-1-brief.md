# Task Brief: Redis Hot Path Schema Design

## Context

Este es el Task 1 del Plan Maestro OMEGA para implementar un pipeline de detección/simulación/ejecución con latencia end-to-end <100ms.

## Goal

Crear documentación técnica que defina el esquema de Redis Streams y Keys para el hot path de baja latencia.

## Files

- Create: `docs/redis-schema/hot-path-v2.md`

## Interfaces

- Produces: Definición de streams y keys para pipeline <100ms
- No consume interfaces previos (primer task)

## Steps (exactos)

### Step 1: Documentar streams requeridos

Crear archivo con la siguiente estructura documentada:

```markdown
## Redis Streams (Hot Path v2)

### arbx:hot:detected (Stream)
- XADD por searcher-rs al detectar oportunidad
- Fields: id, chain_id, strategy_kind, token_path[], amounts[], detected_at_ms
- MAXLEN ~10000
- Consumer Groups: paper-executor-g0, ws-emitter-g0

### arbx:hot:simulated (Stream)
- XADD por searcher-rs post-REVM (solo passed)
- Fields: id, sim_result (JSON), net_profit_wei, gas_used, trace_hash
- MAXLEN ~5000

### arbx:hot:paper_executed (Stream)
- XADD por api-server paper archiver
- Fields: id, execution_time_ms, paper_pnl_usd, status
- MAXLEN ~1000

### Keys (TTL corto)
- arbx:hot:opp:{id} (Hash, TTL 300s) - Datos completos
- arbx:hot:sim:{id} (Hash, TTL 300s) - Resultado simulación
- arbx:metrics:throughput:detected (String, TTL 60s) - Contador para métricas
```

### Step 2: Verificar sintaxis

Run: `cat docs/redis-schema/hot-path-v2.md | head -30`
Expected: Documento markdown válido con estructura clara

### Step 3: Commit

```bash
git add docs/redis-schema/hot-path-v2.md
git commit -m "docs(redis): define hot path schema v2 for <100ms pipeline"
```

## Acceptance Criteria

- [ ] Archivo `docs/redis-schema/hot-path-v2.md` creado
- [ ] Documenta los 3 streams: arbx:hot:detected, arbx:hot:simulated, arbx:hot:paper_executed
- [ ] Documenta las 3 keys con TTL
- [ ] Commiteado con mensaje convencional

## Out of Scope

- Implementación de código Rust/TypeScript
- Tests funcionales
- Modificaciones a archivos existentes
