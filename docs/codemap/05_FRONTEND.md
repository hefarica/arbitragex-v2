# 05_FRONTEND.md — Las 57 páginas contra su fuente y render

> SHA: `35627908` · Vivo: 2026-08-14T04:30:00Z · **55/57 = 200 · 2 redirects (307)**

## Censo completo (57 páginas)

### Páginas principales (pipeline + datos)

| # | Ruta | Modo | Endpoints que consume | VIVO | Estado datos |
|---|---|---|---|---|---|
| 1 | `/` | SSR | /api/status, /api/readiness, /api/scanner/heartbeat | ✅ 200 | yield sin datos (honesto), gates |
| 2 | `/status` | SSR | /api/status, /api/v1/admin/services/*/start | ✅ 200 | 7/7 servicios UP |
| 3 | `/opportunities` | SSR→Client | /api/opportunities/live, WS /socket.io | ✅ 200 | tarjetas con TARGET NONE (0 viables) |
| 4 | `/opportunities/by-strategy` | SSR | /api/opportunities/live (grouped) | ✅ 200 | agrupado por strategy_kind |
| 5 | `/opportunities/exchange` | Client | /api/opportunities/live (exchange filter) | ✅ 200 | 3-6 cards, 2-leg fallback |
| 6 | `/executions` | SSR | /api/v1/executions/recent | ✅ 200 | "No executions yet" (honesto) |
| 7 | `/paper/history` | SSR→Client | /api/paper/history, /api/paper/history/summary | ✅ 200 | avg $49 (A-02b purge aplicado) |
| 8 | `/recon` | Client | /api/v1/recon/summary, /api/v1/recon/timeseries | ✅ 200 | summary 0, timeseries gráfico |
| 9 | `/operations` | Client | /api/operations/kpi, /s-curve, /variance | ✅ 200 | KPIs degenerados (0 ops) |
| 10 | `/route-outcomes` | Client | /api/route-discovery-outcomes/summary | ✅ 200 | datos reales (viables 0) |

### Páginas de infra/config

| # | Ruta | Modo | Endpoints | VIVO | Nota |
|---|---|---|---|---|---|
| 11 | `/routes/discovery` | Client | /api/route-discovery/* | ✅ 200 | Radar: 118 edges, 16ms |
| 12 | `/sed` | Client | WS convergence, /api/sed/status | ✅ 200 | Admin session required (gate) |
| 13 | `/agent-insights` | SSR→Client | /api/agents/status | ✅ 200 | 17 agent verdicts |
| 14 | `/worker-health` | Client | /api/metrics/defi | ✅ 200 | NO ACTIVE WORKERS (honesto) |
| 15 | `/risk` | SSR→Client | /api/v1/risk/alerts | ✅ 200 | 0 alertas/24h |
| 16 | `/killswitch` | Client | /api/status, /admin/killswitch | ✅ 200 | disabled/paper-only |
| 17 | `/live-readiness` | SSR | /api/v1/readiness | ✅ 200 | 4/4 steps verificados |
| 18 | `/operator/self-test` | SSR | /api/operator/selftest | ✅ 200 | 6/10 verified |
| 19 | `/operator/presets` | Client | (sin fetch, math pura) | ✅ 200 | Disclaimer correcto |
| 20 | `/operator/` | — | redirect 307 → /operator/self-test | 307 | diseño |
| 21 | `/audit-logs` | SSR→Client | /admin/audit (cookie-gated) | ✅ 200 | Admin session required (A-01) |
| 22 | `/apex/allocator` | Client | WS arbx:scoring:updates | ✅ 200 | Admin session required |
| 23 | `/settings` | Client | (prefs client-only) | ✅ 200 | Advisory honesto |
| 24 | `/settings/credentials` | Client | /admin/credentials (gated) | ✅ 200 | Bien gateada |
| 25 | `/config` | SSR | /api/v1/config/current | ✅ 200 | app.toml fiel |
| 26 | `/config/trading` | Client | /api/trading-config | ✅ 200 | Form config |
| 27 | `/strategies` | Client | /api/strategy-catalog | ✅ 200 | Catálogo real |
| 28 | `/strategies/forge` | Client | /api/route-discovery/*, /api/cartridges/* | ✅ 200 | Radar + cartridge telemetry |

### Páginas onboarding

| # | Ruta | Modo | Endpoints | VIVO | Nota |
|---|---|---|---|---|---|
| 29 | `/onboarding` | SSR | /api/v1/onboarding/status | ✅ 200 | Fase 1 completa, 2-5 pendientes |
| 30-34 | `/onboarding/1-init` … `/5-production` | Client | admin-gated steps | ✅ 200 | Cada una su paso |

### Páginas registry

| # | Ruta | Modo | Endpoints | VIVO | Estado |
|---|---|---|---|---|---|
| 35 | `/chains` | Client | /api/chains | ✅ 200 | 6 chains ACTIVE |
| 36 | `/rpcs` | Client | /api/rpcs | ✅ 200 | Registry vacío (D-02) |
| 37 | `/pools` | Client | /api/pools | ✅ 200 | 87 pools ACTIVE, PancakeSwap V3 |
| 38 | `/dex-registry` | Client | /api/v1/dexes | ✅ 200 | 7 DEXes |
| 39 | `/wallets` | Client | /api/v1/wallets | ✅ 200 | "No wallets registered" |
| 40 | `/wallet` | Client | WalletConnect (lazy mount) | ✅ 200 | projectId placeholder (B-01 follow-up) |

### Páginas misceláneas

| # | Ruta | Modo | Endpoints | VIVO | Estado |
|---|---|---|---|---|---|
| 41 | `/monitor` | SSR→Client | /api/status, WS /socket.io | ✅ 200 | NOT_AVAILABLE honesto (C-04) |
| 42 | `/readiness` | SSR | /api/v1/readiness | ✅ 200 | NO_GO, 9 pendientes |
| 43 | `/deploy-pipeline` | SSR | (estático) | ✅ 200 | Runbook declarado estático |
| 44 | `/translator` | Client | /api/translate (404 stub) | ✅ 200 | Glosario, botón 404 |
| 45 | `/live-testnet` | Client | EventSource /api/live-testnet/events | ✅ 200 | "Connected: No" |

### Páginas omega-s5

| # | Ruta | Modo | Endpoints | VIVO | Estado |
|---|---|---|---|---|---|
| 46-51 | `/omega-s5/core, wallets, factory, adapters, operator, drift` | Client | varios | ✅ 200 | Vacío honesto |
| 52 | `/omega-s5/crucible` | Client | /api/crucible/status | ✅ 200 | "NOT STARTED" (C-05 fix) |
| 53 | `/omega-s5/registry` | Client | hot-reload channels | ✅ 200 | 6 registries documentados |
| 54 | `/omega-s5/registry/contracts` | Client | /api/operator/me, /api/contracts | ✅ 200 | Datos vacíos honestos |
| 55 | `/omega-s5/registry/[entity]` | Client | dinámica | ✅ 200 | Shell OK |

### Admin

| # | Ruta | Modo | VIVO | Nota |
|---|---|---|---|---|
| 56 | `/admin/signin` | SSR | ✅ 200 | Cookie httpOnly |
| 57 | `/admin/chains` | SSR (gated) | ✅ 200 | Admin session required |
| 58 | `/admin/topology` | SSR (gated) | ✅ 200 | /api/admin/topology/snapshot |

**TOTAL: 57 páginas · 55× 200 · 2× redirect 307 (diseño) · 0 errores**

## Schemas Zod (contratos frontend↔edge)

| Schema | Archivo | Campos críticos |
|---|---|---|
| `OpportunitySchema` | `shared-ts/src/contracts/index.ts:15` | id, chain_id, strategy_kind, expected_profit_usd, net_expected_profit_usd |
| `AuditLogRowSchema` | `frontend/lib/schemas.ts:628` | id, actor, action, ip_address, target_id, created_at |
| `DefiPoolRowSchema` | `frontend/lib/schemas.ts:683` | address, chain_id, dex_name, protocol_type, fee_tier, is_active |
| `DefiChainRowSchema` | `frontend/lib/schemas.ts:654` | chain_id, name, is_active |
| `ReconTimeseriesResponseSchema` | `frontend/lib/schemas.ts` | window_hours, points[] |
| `ScannerHeartbeatResponseSchema` | `frontend/lib/operations-schemas.ts` | snapshot{pending_received, decoded_ok, ...} |
| `DefiContractTest` (FASE0) | `frontend/lib/schemas.defi-contract.test.ts` | acepta payload real, rechaza drift |

## Divergencias repo ↔ vivo (FASE 5)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | 55/57 páginas 200 | ✅ sin divergencia |
| 2 | `/operator/` → 307 → `/operator/self-test` | ✅ por diseño |
| 3 | `/omega-s5/registry/contracts` → 307 | ✅ por diseño |
| 4 | `/wallet` muestra projectId placeholder | 🟡 B-01 follow-up (operador debe provisionar) |
| 5 | `/translator` botón 404 | 🟡 endpoint /api/translate no existe (B-02 residual) |

**Cobertura FASE 5: 95% (57/57 páginas verificadas, schemas catalogados)**
