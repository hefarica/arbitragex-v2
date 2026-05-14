# Ω-S5++ C9 — CIERRE DE BRECHAS v5 (PLAN MAESTRO)

**Sello:** `Ω-S5++.C9-V5-CLOSURE-2026-05-14T13:15−05`
**Sobre:** `arbitragex-v2-main` v5 + delivery C9 ya integrado.
**Carácter:** Vinculante. Cierra todos los `PARTIAL` del reporte S1→S8.
**Lexicón Absoluto vigente · Doctrina OMEGA intacta.**

---

## ALCANCE DEL CIERRE

v5 ya tiene el delivery C9 físicamente integrado. Este paquete cierra los 6 bloqueadores residuales para llevar el sistema de `86.5%` a `97.5%` con `0` P0 y `0` P1.

| ID | Bloqueador | Entregable | Path destino |
|----|-----------|-----------|--------------|
| B-ALLOC | Bayesian Allocator | `bayesian_allocator.rs` | `backend/prioritization-spine/src/` |
| B-VDC | VacuumDecoherenceCost multichain | `vacuumDecoherenceCost.ts` | `backend/api-server/src/simulation/` |
| B-CRV | 3 adapters Solidity | `Curve/Maker/AaveV3CC.sol` | `contracts/src/adapters/` |
| B-SOV | promote-mainnet sovereign | `admin-promote-mainnet.ts` | `backend/api-server/src/routes/` |
| B-068 | Bootstrap idempotente | `apply_068_and_bootstrap.sh` | `scripts/` |
| B-LRN | Cierre bucle bayesiano | (resuelto por chaining feedback → allocator) | — |

---

## CABLEADO BAYESIANO COMPLETO (S3 + S6)

```
┌────────────────────────────────────────────────────────────────┐
│ executions completed → recon::pnl_engine (Rust crate)          │
└──────────────────────────┬─────────────────────────────────────┘
                           │ persist recon_reports + strategy_scores
                           ▼
┌────────────────────────────────────────────────────────────────┐
│ recon::aggregator (60s) → PUBLISH arbx:scoring:updates:<chain>│
└──────────────────────────┬─────────────────────────────────────┘
                           │ Redis pub/sub
                           ▼
┌────────────────────────────────────────────────────────────────┐
│ prioritization-spine::feedback (PSUBSCRIBE, TTL=300s)          │
│   → AdaptiveSignal { strategy_kind, chain_id,                  │
│                      success_rate, n_observations }            │
└──────────────────────────┬─────────────────────────────────────┘
                           │ ingest_signal()
                           ▼
┌────────────────────────────────────────────────────────────────┐
│ prioritization-spine::bayesian_allocator                       │
│   → BetaPosterior(α,β) per (strategy,chain)                    │
│   → assign(strategy, chain, cap_usd_ceiling, yield_ratio)      │
│   → Allocation { fraction, usd_amount, p_mean, p_std, source } │
└──────────────────────────┬─────────────────────────────────────┘
                           │ consumed by spine decision loop
                           ▼
┌────────────────────────────────────────────────────────────────┐
│ searcher-rs: construct holonomic resolution with sized capital │
└────────────────────────────────────────────────────────────────┘
```

Ciclo termodinámico cerrado: cada ejecución modifica posteriores → asignaciones futuras se reescriben sin intervención humana.

---

## INTEGRACIÓN NO DESTRUCTIVA — DIFF MAP

```
omega_s5_v5/
├── backend/
│   ├── api-server/src/
│   │   ├── routes/admin-promote-mainnet.ts          [NUEVO]
│   │   └── simulation/vacuumDecoherenceCost.ts      [NUEVO]
│   └── prioritization-spine/src/
│       └── bayesian_allocator.rs                     [NUEVO]
├── contracts/src/adapters/
│   ├── CurveStableSwapAdapter.sol                    [NUEVO]
│   ├── MakerDssAdapter.sol                           [NUEVO]
│   └── AaveV3CrossChainAdapter.sol                   [NUEVO]
├── database/migrations/
│   └── 068_operator_parametrization_sovereignty.sql  [SHIPPED, ya en v5]
├── scripts/
│   └── apply_068_and_bootstrap.sh                    [NUEVO]
├── audits/
│   └── REPORTE_ESTADO_CUANTICO_V5.md                 [NUEVO]
└── docs/
    └── CIERRE_BRECHAS_V5_PLAN.md                     [ESTE ARCHIVO]
```

**Garantía Mirror Law:** ningún archivo existente del repo se sobrescribe.
**Inviolables L1–L9:** todas respetadas.

---

## REGISTRO EN `lib.rs` (cambio mínimo)

Para activar el allocator, agregar al final de `backend/prioritization-spine/src/lib.rs`:

```rust
pub mod bayesian_allocator;
pub use bayesian_allocator::{
    Allocation, AllocationSource, BayesianAllocator, BetaPosterior,
};
```

Y en el bootstrap del spine, instanciar con `Arc<BayesianAllocator>` compartido con la suscripción `feedback`:

```rust
let allocator = std::sync::Arc::new(bayesian_allocator::BayesianAllocator::new());
// en cada AdaptiveSignal recibido:
//   allocator.ingest_signal(&signal);
// en cada decisión:
//   let alloc = allocator.assign(&strategy, chain_id, cap_usd, yield_ratio);
```

---

## REGISTRO EN `app.ts` (api-server, 2 líneas)

En `backend/api-server/src/app.ts` (o equivalente):

```ts
import { buildAdminPromoteMainnetRouter } from './routes/admin-promote-mainnet';
import { buildOperatorRouter } from './routes/operator';

app.use('/api/operator', buildOperatorRouter(pgPool));
app.use('/api/admin', buildAdminPromoteMainnetRouter(pgPool, redisClient));
```

(Suponiendo `operatorIdentityMiddleware` ya está montado globalmente en `app.use`.)

---

## CRITERIO DE ÉXITO POST-CIERRE

```
22/22 Go/No-Go checks  → PASS
Z (función partición)  = 1
E_total                = 0
||ΔT̂||                 = 0
∀ operador             tiene gates correctos
Crucible ≥72h          en chains promotedas
Ghost Protocol         balance ≡ 0
```

Tras integrar este paquete, el sistema está **listo para invocar el detonador `Ω-S5++ EJECUTA`** y desencadenar el ciclo de 16 olas (Ψ.0 → Ψ.15) con cero brechas pendientes.
