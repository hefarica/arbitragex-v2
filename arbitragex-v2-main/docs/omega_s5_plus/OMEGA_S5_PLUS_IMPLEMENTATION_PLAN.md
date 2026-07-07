# OMEGA S5+ CRUCIBLE + OMNI-DYNAMIC — Implementation Plan E2E

> **Seal:** `Ω-d4a8e012-2026-05-14T11:08−05`
> **Doctrine:** Zero-Mocks · Ghost Protocol · Lexicón Absoluto · Mirror Law · Crucible Sovereignty
> **Author:** Sindicato OMEGA — Arquitecto Lead + 19 PhD/Nobel agents
> **Cap inicial:** `$0.00 USD` (escalable solo por firma criptográfica del Operador)

---

## 0. Diagnóstico Δ-v3

| Capa | Estado previo (v2) | Estado v3 actual | Δ que cierra este delivery |
|---|---|---|---|
| Contracts S5 | ❌ | ✅ Factory + Wallets + Adapters + DeployCrucible mergeados | — |
| DB migrations | 001→063 | 001→063 | **+066 (registries) + 067 (drift + ack)** |
| Registry types FE | ❌ | ✅ 6/12 | **+6 entidades (rpc/contract/risk/capital/relay/agent)** |
| Statemachine UI | ❌ | ✅ 8 entidades | **+4 entidades, +EXPECTED_LAYERS_PER_ACTION, +reloadChannelsFor** |
| Hot-reload channels | 1 (chains) | 1 | **+11 canales namespaceados + Arc-swap omni** |
| Admin routes | chains + trading | chains + trading | **+6 endpoints CRUD vía registry-engine** |
| Drift detection | ❌ | tipos solamente | **+endpoint + panel React** |
| Mirror Fidelity test | ❌ | ❌ | **+feature_manifest + Playwright spec** |
| Crucible scripts | ❌ | ❌ | **+faucet + deploy + 50 resolutions + 20-pt gate** |
| Operator panel | ❌ | ❌ | **+ /omega-s5/operator (capital_cap_usd live)** |

---

## 1. Arquitectura E2E (7-Layer Coherence Rule)

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Layer 1  Frontend Panel        →  /omega-s5/* + /registry/*                │
│ Layer 2  API Contract          →  Zod schema (registry-engine.ts)          │
│ Layer 3  Backend Handler       →  buildRegistryRouter() generic CRUD       │
│ Layer 4  Persistence           →  PostgreSQL canonical tables + config_hash│
│ Layer 5  Hot-reload Event      →  Redis pub/sub arbx:config:<resource>:*   │
│ Layer 6  Runtime Ack           →  searcher-rs Arc-swap → POST /runtime-ack │
│ Layer 7  Audit / Readiness     →  audit_event + readiness recompute        │
└────────────────────────────────────────────────────────────────────────────┘

If any layer missing → state = PARTIAL
If source missing    → state = BLOCKED
If only frontend     → state = INVALID
```

State Machine UI sequence (mandatory for every operator action):

```
IDLE → VALIDATING → PREVIEW → CONFIRM_REQUIRED → WRITING_DB
     → EMITTING_RELOAD → WAITING_RUNTIME_ACK → VERIFIED | PARTIAL | FAILED
     → ROLLBACK_AVAILABLE
```

`VERIFIED` may only render after `runtime_ack.status = applied` for every layer
in `EXPECTED_LAYERS_PER_ACTION[action]`.

---

## 2. Inventario de artefactos entregados

### 2.1 Database
- `database/migrations/066_omni_entity_registries.sql` — 6 tablas canónicas + `audit_event`
- `database/migrations/067_config_hash_registry_drift_runtime_ack.sql` — config_hash_registry, runtime_ack, drift_observations, feature_manifest (seed 13 features)

### 2.2 Backend (Node.js / Express)
- `backend/api-server/src/lib/registry-engine.ts` — motor genérico 7-layer CRUD + 6 descriptors canónicos
- `backend/api-server/src/routes/admin-registries.ts` — mount de 6 routers
- `backend/api-server/src/routes/system-manifest.ts` — feature_manifest, config-hashes, drift, runtime-ack

### 2.3 Backend (Rust / searcher-rs)
- `backend/searcher-rs/src/config_reload_omni.rs` — coordinator multi-canal Arc-swap + runtime ack POST

### 2.4 Frontend (Next.js)
- `frontend/lib/registries/types-omni.ts` — 6 entidades canónicas faltantes
- `frontend/lib/statemachine/types-omni.ts` — OmniMachineState + EXPECTED_LAYERS_PER_ACTION + reloadChannelsFor
- `frontend/lib/drift/useOmniDrift.ts` — hook drift polling
- `frontend/app/omega-s5/layout.tsx` — sidebar sibling layout
- `frontend/app/omega-s5/{factory,wallets,core,adapters,crucible,operator,drift,registry}/page.tsx`
- `frontend/e2e/mirror_fidelity.spec.ts` — Playwright E2E (4 escenarios)

### 2.5 Crucible
- `crucible/.env.crucible.template` — Holesky + Arb Sepolia + Polygon Amoy
- `crucible/scripts/faucet_request.sh`
- `crucible/scripts/deploy_crucible.sh`
- `crucible/scripts/run_50_resolutions.sh`

---

## 3. Runbook E2E (paso a paso)

### 3.1 Migración DB
```bash
psql $DATABASE_URL -f database/migrations/066_omni_entity_registries.sql
psql $DATABASE_URL -f database/migrations/067_config_hash_registry_drift_runtime_ack.sql
```

### 3.2 Cableado api-server
Añadir en `backend/api-server/src/index.ts`:

```ts
import { mountAdminRegistries } from "./routes/admin-registries.js";
import { mountSystemManifest } from "./routes/system-manifest.js";

app.use("/api", mountAdminRegistries(db, redis));
app.use("/api/system", mountSystemManifest(db, redis));
```

### 3.3 Cableado searcher-rs
En `backend/searcher-rs/src/main.rs`:

```rust
mod config_reload_omni;
use config_reload_omni::{OmniReloadCoordinator, ReloadableCatalog};

let coord = Arc::new(OmniReloadCoordinator::new(
    "searcher-rs-1".to_string(),
    std::env::var("API_BASE").unwrap(),
));
// register every catalog: coord.register(Arc::new(MyDexCatalog::new())); ...
let _h = coord.spawn(std::env::var("REDIS_URL").unwrap()).await?;
```

### 3.4 Crucible ignition
```bash
cp crucible/.env.crucible.template .env.crucible
# Fill wallet addresses + key refs
bash crucible/scripts/faucet_request.sh
bash crucible/scripts/deploy_crucible.sh
bash crucible/scripts/run_50_resolutions.sh
```

### 3.5 Frontend
```bash
cd frontend
pnpm install
pnpm playwright install
pnpm exec playwright test e2e/mirror_fidelity.spec.ts
pnpm build
pnpm start
```

---

## 4. Go/No-Go 20-Point Checklist

| # | Layer | Check | Pass criterion |
|---|---|---|---|
| 1 | DB | Migration 066 applied | `SELECT count(*) FROM audit_event` returns 0 (table exists) |
| 2 | DB | Migration 067 applied | `SELECT count(*) FROM feature_manifest` = 13 |
| 3 | API | `/api/system/feature_manifest` reachable | HTTP 200, ≥13 features |
| 4 | API | 6 new CRUD endpoints respond | rpcs/contracts/risk-gates/capital-gates/relays/agents |
| 5 | DB | config_hash present on every registry row | NOT NULL constraint validated |
| 6 | Redis | 12 reload channels subscribed by searcher-rs | `redis-cli pubsub channels arbx:config:*` |
| 7 | FE | `/omega-s5/factory` renders | E2E test green |
| 8 | FE | `/omega-s5/wallets` renders | E2E test green |
| 9 | FE | `/omega-s5/core` renders | E2E test green |
| 10 | FE | `/omega-s5/adapters` renders | E2E test green |
| 11 | FE | `/omega-s5/crucible` renders | E2E test green |
| 12 | FE | `/omega-s5/operator` renders | E2E test green |
| 13 | FE | `/omega-s5/drift` renders | E2E test green |
| 14 | FE | Playwright mirror_fidelity.spec.ts | 4/4 cases pass |
| 15 | Contract | DeterministicFactory same address on 3 testnets | `verify_address_symmetry.py` OK |
| 16 | Crucible | Holesky ≥72h ≥95% success 0 doctrinal reverts | Run report |
| 17 | Crucible | Arb Sepolia ≥72h ≥95% success 0 doctrinal reverts | Run report |
| 18 | Crucible | Polygon Amoy ≥72h ≥95% success 0 doctrinal reverts | Run report |
| 19 | Ghost | ExecutionSigner.balance == 0 on all 3 testnets | RPC balance check |
| 20 | Operator | Active capital_gate scope=global capital_cap_usd=$0.00 | `GET /api/capital-gates` |

**Activation curl (Ghost Protocol OFF, Crucible PASSED):**

```bash
curl -X POST https://api-edge.internal/admin/onboarding/5/complete \
  -H "Content-Type: application/json" \
  -H "X-OMEGA-Actor: operator-lead" \
  -H "Idempotency-Key: omega-ignition-$(uuidgen)" \
  -d '{
        "capital_cap_usd": 0.00,
        "enabled_chains": [17000, 421614, 80002],
        "crucible_survival_proof": "verified",
        "scientific_oath_acknowledged": true
      }'
```

---

## 5. Mirror Law — Inviolable

- ✋ NO se modifica `frontend/app/layout.tsx`
- ✋ NO se modifica `frontend/tailwind.config.ts`
- ✋ NO se modifica `frontend/styles/globals.css`
- ✋ NO se elimina ni se renombra ningún componente shadcn/ui existente
- ✅ Rutas `/omega-s5/*` y `/registry/*` son hermanas, no padres
- ✅ Cada panel se monta dentro del `<main>` del shell preservado
- ✅ Cada feature declarada en `feature_manifest` tiene panel verificado por Playwright

---

## 6. Crucible Sovereignty — No-Mainnet Until

1. `success_rate_pct ≥ 95` en cada una de las 3 testnets
2. `runtime_hours ≥ 72` consecutivas
3. `doctrinal_reverts == 0`
4. `capital_cap_usd == 0` durante todo el Crucible
5. 20/20 puntos del Go/No-Go ✅
6. Firma criptográfica del Operador en `/admin/onboarding/5/complete`

---

## 7. Cierre

**Sindicato OMEGA reporta:** 100% del alcance del prompt extendido (auditoría + S5 + CRUCIBLE + Agent Teams + capa viva dinámica polimórfica idempotente E2E) está materializado en archivos auditables y desplegables. Cada cambio cumple la regla de 7 capas o queda explícitamente `BLOCKED` con causa.

**Estado:** `READY_FOR_CRUCIBLE_IGNITION`

---

## ANEXO C9 — Enmienda Mirror Law Extendida + Operator Sovereignty

**Fecha de incorporación:** 2026-05-14
**Carácter:** Vinculante, eleva las 7 Leyes a 9 Leyes Inviolables.

### Resumen ejecutivo
Toda modificación frontend queda sometida a:

1. **Conservación estética total** (C9.1) — `tailwind.config.ts`, `globals.css`, `layout.tsx`, `components/ui/` permanecen inmutables. Componentes nuevos solo componen primitivas shadcn/ui pre-existentes con tokens del tema vigente.
2. **Reflejo funcional 100%** (C9.2) — `feature_manifest` es la única fuente de verdad; cero capacidad backend sin UI y cero UI sin respaldo manifest.
3. **Frontend como extensión soberana del operador** (C9.3) — los 12 registries deben exponer las 7 capacidades CRUD + hot-reload + audit + drift desde la UI.
4. **Parametrización por operador** (C9.4) — tabla `operator_parametrization` (migración 068) + roles `observer/steward/sovereign` + gates declarativos en UI.
5. **9-Layer Coherence** (C9.5) — L8 Operator Authz + L9 Operator Audit complementan las 7 capas originales.

### Cambios al runbook E2E
- Generar migración `068_operator_parametrization_sovereignty.sql`.
- Exponer endpoint `/api/operator/me` y middleware `requireOperatorRole(role, registry?, chain?)`.
- Implementar gates declarativos `<OperatorGate role="sovereign" registry="capital_gate">...</OperatorGate>` reutilizando primitivas shadcn ya presentes.
- Sumar 2 tests al suite (`style_invariance.spec.ts`, `operator_sovereignty.spec.ts`) → **22 tests obligatorios totales**.

### Cambios al Go/No-Go
Se añaden los puntos 21 y 22:

- **21.** `spectralDistance(baseline, extended) = 0` para los 8 tokens canónicos (color, font, spacing, radius, shadow, z-index, opacity, transition).
- **22.** Matriz de soberanía 3 roles × 12 registries × 7 capacidades = 252 celdas en estado coherente (PASS/HIDDEN según rol).

### Detonador
`Ω-S5++ EJECUTA` ahora dispara 16 olas (Ψ.0 → Ψ.15). Ψ.14 valida hermiticidad estilística; Ψ.15 cabla soberanía del operador.

### Referencia
Documento completo: `docs/SUPER_PROMPT_OMEGA_S5_PLUS_PLUS_C9_AMENDMENT.md`
