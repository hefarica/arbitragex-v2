# ArbitrageX v2 — Productivo Full

Plataforma MEV/arbitraje institucional con separación canónica de capas.
**Estado actual: Sprint 1 (Foundations) implementado, Sprints 2–8 pendientes.**

## Estructura

- `backend/` — Rust workspace + TS workspace (dual). Hot-path en Rust; control-plane en TS.
  - `shared-rs/` — crate compartida: config, kill-switch, metrics, tracing, health.
  - `searcher-rs/` — detector (S1: esqueleto, S2: mempool real).
  - `sim-ctl/` — simulación (S1: 501, S4: Anvil/fork real).
  - `relays-client/` — ejecución privada (S1: 501, S5: Flashbots).
  - `recon/` — reconciliación PnL (S1: lee DB real).
  - `selector-api/` — scoring multi-factor (TS).
  - `api-server/` — gateway + `/admin/killswitch` + `/status` (TS).
- `shared-ts/` — paquete npm compartido con contratos + loaders.
- `edge/`
  - `worker/` — Cloudflare Worker canónico (`wrangler.toml`).
  - `dev-local/` — Express shim dev-only.
- `frontend/` — Next.js 14 App Router. Solo consume el edge.
- `database/migrations/` — 11 migraciones versionadas (9 tablas + roles + control).
- `configs/schemas/` — **fuente de verdad** JSON Schemas.
- `monitoring/` — Prometheus + Grafana + Alertmanager + Loki + Promtail.
- `automation/scripts/` — bootstrap, health-check, smoke-test, migrate, validate-config, rollback, seed-dev.
- `docs/` — spec, SOP, API contracts, trust policy, canonical sources, roadmap, risk policy.

## Reglas obligatorias

1. **Sin mocks, sin hardcodes**. Endpoints que requieren infra externa no configurada responden `501 {requires:[…], sprint:"SN"}`.
2. **Simulación antes de ejecución** (pipeline `Detect → Validate → Simulate → Select → Fund → Execute → Recon → Learn`).
3. **Secretos fuera del repo**. Ver `configs/secrets.policy.md`.
4. **Edge = único endpoint público**. Hot-path nunca expuesto.
5. **Kill-switch global** siempre consultado antes de actions críticas.
6. **Honestidad de estado**: `[OK] [PARCIAL] [PENDIENTE] [BLOQUEADO]` en cada deliverable.

## Inicio rápido (local dev)

```bash
cp .env.example .env             # EDITAR antes de ejecutar
./automation/scripts/bootstrap.sh          # build + up + migrate + health
./automation/scripts/smoke-test.sh         # verificar contratos incluyendo 501
```

Servicios locales:

| Servicio       | URL                       |
|----------------|---------------------------|
| Frontend       | http://localhost:5173     |
| Edge           | http://localhost:8787     |
| api-server     | http://localhost:8080     |
| selector-api   | http://localhost:3002     |
| sim-ctl        | http://localhost:3003     |
| recon          | http://localhost:3004     |
| relays-client  | http://localhost:3005     |
| searcher-rs    | http://localhost:9001     |
| Prometheus     | http://localhost:9090     |
| Grafana        | http://localhost:3000     |
| Alertmanager   | http://localhost:9093     |
| Loki           | http://localhost:3100     |

## Documentación clave

- `docs/superpowers/specs/2026-04-20-sprint1-foundations-design.md` — Spec S1.
- `docs/API_CONTRACTS.md` — Contratos HTTP por servicio.
- `docs/CANONICAL_SOURCES.md` — Repos canónicos por capa.
- `docs/TRUST_POLICY.md` — Verificado / Narrativo / No disponible.
- `docs/SOP_ENTERPRISE.md` — SOP + backup/restore + runbooks.
- `docs/ROADMAP_FASES.md` — 8 sprints y qué credenciales requiere cada uno.
- `configs/secrets.policy.md` — Policy de secretos.

## Roadmap

`S1 Foundations (actual) → S2 Detection → S3 Selector+Risk → S4 Sim → S5 Exec → S6 Recon+Learn → S7 Edge+Frontend → S8 Obs+E2E+Gov`.

## Disclaimer

Este repo **no** garantiza rentabilidad. No autoriza operación con capital real. El paso a real requiere el cierre completo de S8 con evidencia verificable.
