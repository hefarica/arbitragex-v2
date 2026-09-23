# 🔬 AUDITORÍA TÉCNICA COMPLETA CON GRAFOS DE PROCESAMIENTO MATEMÁTICO
## ArbitrageX v2 - Estado al 20 Sep 2026, 07:00 UTC

> Añadida a tareas por orden del operador ("agrega esto a tus tareas", 2026-09-20).
> Fuente: auditoría externa (sesión peer); claims marcados ⚠️ requieren verificación
> propia antes de cierre (doctrina: aserciones de agentes ≠ facts).

---

## 📊 GRAFO 1: Flujo Matemático de Datos

```mermaid
flowchart TB
    subgraph INPUT["📥 INPUT: Estado On-Chain"]
        S0["slot0.sqrtPriceX96<br/>slot0.tick<br/>slot0.observationIndex"]
        LIQ["liquidity (uint128)"]
        BLOCK["block.number<br/>block.timestamp"]
    end

    subgraph FINGERPRINT["🔐 CAPA 1: State Fingerprint"]
        HASH1["Keccak256<br/>(sqrtPrice + tick + idx)<br/>→ slot0_hash[32]"]
        HASH2["Keccak256<br/>(liquidity)<br/>→ liq_hash[32]"]
        COMPARE["¿hash_actual ≠ hash_previo?"]
        DECISION{"Cambio<br/>significativo?"}
    end

    subgraph BATCH["📦 CAPA 2: QuoteWindow"]
        ACC["Acumular requests<br/>Ventana: 50ms"]
        MULTI["multicall3.aggregate<br/>(~28 pools/call)"]
        RPC["eth_call único<br/>por batch"]
    end

    subgraph CACHE["💾 CAPA 3: Dirty-Seed Cache"]
        NEG["Neg-cache: 10s<br/>(rechazo rápido)"]
        POS["Pos-cache: TTL dinámico<br/>basado en volatilidad"]
        HIT["Cache hit ratio<br/>objetivo: >90%"]
    end

    subgraph OUTPUT["📤 OUTPUT: Push Selectivo"]
        WS["WebSocket Snap<br/>Solo si cambió"]
        REST["/api/opportunities<br/>Polling legacy"]
        EDGE["Cloudflare Edge<br/>CDN cache 30s"]
    end

    S0 --> HASH1
    LIQ --> HASH2
    BLOCK --> COMPARE
    HASH1 --> COMPARE
    HASH2 --> COMPARE
    COMPARE --> DECISION

    DECISION -->|Sí| BATCH
    DECISION -->|No| CACHE

    BATCH --> RPC
    RPC --> CACHE
    CACHE --> HIT

    HIT -->|Cambio real| WS
    HIT -->|Request explícito| REST
    WS --> EDGE
    REST --> EDGE
```

**Invariante Matemática:** ∀ p ∈ Pools, Δt ≤ 50ms ⇒ multicall(p₁...pₙ)

**Dedup Ratio Esperado:** eth_calls_antes / eth_calls_después = 279/10 ≈ 28x

---

## 📊 GRAFO 2: Arquitectura de Servicios y Dependencias

```mermaid
flowchart TB
    subgraph EXTERNAL["🌐 Externo"]
        RPC["RPC Providers<br/>drpc/publicnode/0xrpc/blockpi<br/>(+ alchemy PAYG)"]
        RELAYS["MEV Relays<br/>flashbots/mevblocker<br/>(solo bundles)"]
        CF["Cloudflare<br/>edge-arbx.ape-tv.net"]
    end

    subgraph CORE["⚡ Core Hot-Path (Rust)"]
        SEARCHER["searcher-rs<br/>detector_id: 6 engines"]
        MATH["math-engine<br/>31 operators"]
        SIM["sim-ctl<br/>REVM fork"]
        RELAYS_CLIENT["relays-client<br/>bundle submitter"]
    end

    subgraph CONTROL["🎛️ Control Plane (TS)"]
        API["api-server:8080<br/>WebSocket directo"]
        SELECTOR["selector-api:3002"]
        RECON["recon:3004"]
    end

    subgraph DATA["💾 Data Layer"]
        PG["PostgreSQL<br/>121 migraciones"]
        REDIS["Redis 7.2<br/>AOF ON (post-fix)"]
        MINIO["MinIO<br/>Thanos blocks"]
    end

    subgraph FRONT["🖥️ Frontend"]
        NEXT["Next.js 14<br/>App Router"]
        PANEL["/opportunities<br/>página única"]
    end

    RPC --> SEARCHER
    SEARCHER --> MATH
    MATH --> SIM
    SIM --> RELAYS_CLIENT
    RELAYS_CLIENT --> RELAYS

    SEARCHER --> REDIS
    SEARCHER --> PG
    API --> REDIS
    API --> PG

    API --> NEXT
    NEXT --> CF
    CF --> API

    SELECTOR --> SEARCHER
    RECON --> PG
```

---

## 📊 GRAFO 3: Máquina de Estados de Deployment

```mermaid
stateDiagram-v2
    [*] --> LOCAL_DEV: Desarrollo
    LOCAL_DEV --> GIT_PUSH: Commit/PR
    GIT_PUSH --> CI_CHECKS: GitHub Actions
    CI_CHECKS --> VPS_DEPLOY: Merge main
    VPS_DEPLOY --> HEALTH_CHECK: Deploy canónico

    HEALTH_CHECK --> LIVE: Health PASS
    HEALTH_CHECK --> ROLLBACK: Health FAIL

    ROLLBACK --> VPS_DEPLOY: Retry

    LIVE --> PAPER_MODE: ARBX_CARTRIDGE_MODE=shadow
    LIVE --> LIVE_MAINNET: A.6+A.7+A.9 PASS

    PAPER_MODE --> LIVE_MAINNET: Sign-off formal

    state CI_CHECKS {
        [*] --> RUST_TEST
        RUST_TEST --> TS_TEST
        TS_TEST --> E2E_TEST
        E2E_TEST --> SECURITY_AUDIT
        SECURITY_AUDIT --> [*]
    }

    state VPS_DEPLOY {
        [*] --> BACKUP_ENV
        BACKUP_ENV --> RUN_MIGRATIONS
        RUN_MIGRATIONS --> RESTART_SERVICES
        RESTART_SERVICES --> VERIFY_HEALTH
        VERIFY_HEALTH --> [*]
    }
```

---

## 🚨 DEUDAS TÉCNICAS CRÍTICAS

### DT-1: Flipper Deployment — RESUELTO ✅
- `run_migrations.sh` usaba `ARBX_RW_PW` vs `ARBX_RW_PASSWORD`.
- Fix: PR #595 + #596 — script acepta ambos nombres, sources `.env` cuando vars unset.

### DT-2: Disco VPS 100% — MITIGADO ⚠️
- Causa: `route_discovery_outcomes` = 37GB (76M filas en 2 días).
- Fix aplicado: PR #591 ventana cruda 2d → 1d (~18GB); cron 4:17 AM diario.
- Pendiente: alerta disco >80% antes de PostgreSQL PANIC.

### DT-3: Batching RPC (B1/F1) — PENDIENTE 🔴 (P0)
- 78% del funnel rechazado por `v3_quote_unavailable` (69.930/30min).
- Root cause: 1 eth_call por pool (279 pools) → ~280 calls/tick; públicos 429 →
  breakers abiertos → neg-cache 2s → bucle 36.6/s.
- Ver detalle: `06-V3QUOTE-ROOT-CAUSE-2.md` (misma carpeta).
- Solución: `QuoteWindow` 50ms + `multicall3.aggregate` (~10 calls/tick, ~28x menos).
- El kernel `v3_quote_exact_in_multicall` YA soporta batches; caller pasa `vec![1]`
  (v3_quote_provider.rs:257).
- Requiere PR con tests de equivalencia unary vs batch.

### DT-4: Alchemy PAYG — PENDIENTE (operador) 🟡
- Free tier agotado (429 hasta en `eth_blockNumber`).
- Costo estimado $50-100/mes. Con F1+F2 quizá innecesario.

### F2 (config): sanear `RPC_HTTP_1`
- Quitar 1rpc (410 Gone), flashbots/mevblocker del pool del quoter (son relays MEV,
  no RPC de eth_call — 403 permanente → reabren breakers eternamente).
- Mantener drpc/publicnode/0xrpc/blockpi/llama.

### F4 (observabilidad): exportar `arbx_rpc_provider_state` para TODOS los
proveedores declarados (hoy 4/9 invisibles) + alerta si >50% Open.

---

## ⚔️ CONFLICTOS DE DIRECTIVAS

- **CD-1 §34.3 vs código**: resuelto documentalmente (§34.5, 2026-09-17) —
  default-deny por env, no restricción de código.
- **CD-2 Lexicón**: doctrina interna en CLAUDE.md; términos estándar en código público.
- **CD-3 Zero Mocks vs testing**: Fail-Honest (None ≠ Some(0.0)); Anvil fork = simulación
  controlada permitida.
- **CD-4 Local-first vs VPS real**: main tree para compilar (AppControl), worktrees solo
  review.

---

## 🎯 MATRIZ DE DECISIÓN

| Problema | Solución inmediata | Solución raíz | Prioridad |
|---|---|---|---|
| 78% v3_quote_unavailable | F2: sanear RPC_HTTP_1 (.env VPS) | F1: batching multicall | P0 |
| Alchemy 429 | — | PAYG ($50-100/mes) — operador | P1 |
| Disco 100% | Mitigado (2d→1d) | Alerta + auto-cleanup | P1 |
| Cache hit <90% | Neg-cache 2s→10s | Dirty-seeding optimizado | P2 |

---

## ⚠️ Claims que requieren verificación propia antes de cierre

- `/api/status` sha `5fa9c8e0` deployado a las 06:45Z — verificar con
  `ssh arbx git -C /opt/arbitragex-v2 rev-parse HEAD`.
- "cargo test 1305 passed / vitest 84 passed" — re-ejecutar en el SHA vigente.
- Dedup ratio 28x — medir post-F1 con `arbx_v3_quote_total`.

**Veredicto X10 (fuente):** sistema operativo estable post-flipper; deuda crítica
restante = F1 batching (~40 líneas + tests de equivalencia) para bajar 78%→<10%.
