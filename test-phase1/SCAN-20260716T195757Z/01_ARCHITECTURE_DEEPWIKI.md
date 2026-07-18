# Architecture DeepWiki

## Control

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| HOST:repo-parity | PARITY | UNKNOWN |  |  |  | Local/reference SHA and VPS SHA | Compares deployed source identity | Deployment parity verdict |
| EXT:source-control | EXTERNAL | VERIFIED |  |  |  | Repository reference | Provides source-of-truth commit | Git SHA and history |

## Data/Security

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| redis | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| postgres | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| vault | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| anvil | CONTAINER | UNKNOWN |  |  |  |  |  |  |

## Experience/API

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| api-server | CONTAINER | UNKNOWN |  |  | redis |  |  |  |
| edge | CONTAINER | UNKNOWN |  |  | api-server |  |  |  |
| selector-api | CONTAINER | UNKNOWN |  |  | postgres, redis |  |  |  |
| frontend | CONTAINER | UNKNOWN |  |  | edge |  |  |  |

## Infrastructure

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| HOST:docker-engine | HOST_SERVICE | UNKNOWN |  |  |  | Compose definitions and images | Runs containers | Container runtime |
| VPS:host | HOST | UNKNOWN |  |  |  | SSH read-only inspection | Hosts repository and containers | Linux runtime |
| HOST:compose-contract | COMPOSE | VERIFIED | docker/compose.prod.yml |  |  | Compose YAML and environment references | Defines services, networks, volumes and health checks | Runtime topology |

## Observability

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| minio | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| prometheus | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| loki | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| thanos-sidecar | CONTAINER | UNKNOWN |  |  | prometheus, minio |  |  |  |
| thanos-store | CONTAINER | UNKNOWN |  |  | minio |  |  |  |
| alertmanager | CONTAINER | UNKNOWN |  |  |  |  |  |  |
| grafana | CONTAINER | UNKNOWN |  |  | prometheus |  |  |  |
| promtail | CONTAINER | UNKNOWN |  |  | loki |  |  |  |
| thanos-query | CONTAINER | UNKNOWN |  |  | thanos-sidecar, thanos-store |  |  |  |

## Processing

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| relays-client | CONTAINER | UNKNOWN | backend | backend | redis |  |  |  |
| searcher-rs | CONTAINER | UNKNOWN | backend | backend | redis, postgres |  |  |  |
| sim-ctl | CONTAINER | UNKNOWN | backend | backend | postgres, redis |  |  |  |
| recon | CONTAINER | UNKNOWN | backend | backend | postgres |  |  |  |
| token-enricher | CONTAINER | UNKNOWN | backend | backend | postgres, redis |  |  |  |

## Repository

| ID | Type | Status | Expected | Actual | Requires | Consumes | Process | Produces |
|---|---|---|---|---|---|---|---|---|
| FILE:docker/compose.prod.yml | FILE | VERIFIED | docker/compose.prod.yml | docker/compose.prod.yml |  |  |  |  |
| FILE:.devcontainer/Dockerfile | FILE | VERIFIED | .devcontainer/Dockerfile | .devcontainer/Dockerfile |  |  |  |  |
| FILE:.devcontainer/docker-compose.yml | FILE | VERIFIED | .devcontainer/docker-compose.yml | .devcontainer/docker-compose.yml |  |  |  |  |
| FILE:.github/workflows/action-a-plus-v2.yml | FILE | VERIFIED | .github/workflows/action-a-plus-v2.yml | .github/workflows/action-a-plus-v2.yml |  |  |  |  |
| FILE:.github/workflows/action-a-plus.yml | FILE | VERIFIED | .github/workflows/action-a-plus.yml | .github/workflows/action-a-plus.yml |  |  |  |  |
| FILE:.github/workflows/audit-vps-wiring.yml | FILE | VERIFIED | .github/workflows/audit-vps-wiring.yml | .github/workflows/audit-vps-wiring.yml |  |  |  |  |
| FILE:.github/workflows/audit-wiring.yml | FILE | VERIFIED | .github/workflows/audit-wiring.yml | .github/workflows/audit-wiring.yml |  |  |  |  |
| FILE:.github/workflows/audit.yml | FILE | VERIFIED | .github/workflows/audit.yml | .github/workflows/audit.yml |  |  |  |  |
| FILE:.github/workflows/auto-deploy-vps.yml | FILE | VERIFIED | .github/workflows/auto-deploy-vps.yml | .github/workflows/auto-deploy-vps.yml |  |  |  |  |
| FILE:.github/workflows/c10-f1-recovery-step14-only.yml | FILE | VERIFIED | .github/workflows/c10-f1-recovery-step14-only.yml | .github/workflows/c10-f1-recovery-step14-only.yml |  |  |  |  |
| FILE:.github/workflows/cartridge-integration-deploy.yml | FILE | VERIFIED | .github/workflows/cartridge-integration-deploy.yml | .github/workflows/cartridge-integration-deploy.yml |  |  |  |  |
| FILE:.github/workflows/ci.yml | FILE | VERIFIED | .github/workflows/ci.yml | .github/workflows/ci.yml |  |  |  |  |
| FILE:.github/workflows/codeql.yml | FILE | VERIFIED | .github/workflows/codeql.yml | .github/workflows/codeql.yml |  |  |  |  |
| FILE:.github/workflows/deploy-edge-only-v2.yml | FILE | VERIFIED | .github/workflows/deploy-edge-only-v2.yml | .github/workflows/deploy-edge-only-v2.yml |  |  |  |  |
| FILE:.github/workflows/deploy-edge-only.yml | FILE | VERIFIED | .github/workflows/deploy-edge-only.yml | .github/workflows/deploy-edge-only.yml |  |  |  |  |
| FILE:.github/workflows/deploy-frontend.yml | FILE | VERIFIED | .github/workflows/deploy-frontend.yml | .github/workflows/deploy-frontend.yml |  |  |  |  |
| FILE:.github/workflows/deploy-vps.yml | FILE | VERIFIED | .github/workflows/deploy-vps.yml | .github/workflows/deploy-vps.yml |  |  |  |  |
| FILE:.github/workflows/deploy.yml | FILE | VERIFIED | .github/workflows/deploy.yml | .github/workflows/deploy.yml |  |  |  |  |
| FILE:.github/workflows/diag-cookie-emission.yml | FILE | VERIFIED | .github/workflows/diag-cookie-emission.yml | .github/workflows/diag-cookie-emission.yml |  |  |  |  |
| FILE:.github/workflows/docker-build.yml | FILE | VERIFIED | .github/workflows/docker-build.yml | .github/workflows/docker-build.yml |  |  |  |  |
| FILE:.github/workflows/dockerfile-audit.yml | FILE | VERIFIED | .github/workflows/dockerfile-audit.yml | .github/workflows/dockerfile-audit.yml |  |  |  |  |
| FILE:.github/workflows/e2e.yml | FILE | VERIFIED | .github/workflows/e2e.yml | .github/workflows/e2e.yml |  |  |  |  |
| FILE:.github/workflows/ethics-guard.yml | FILE | VERIFIED | .github/workflows/ethics-guard.yml | .github/workflows/ethics-guard.yml |  |  |  |  |
| FILE:.github/workflows/foundry.yml | FILE | VERIFIED | .github/workflows/foundry.yml | .github/workflows/foundry.yml |  |  |  |  |
| FILE:.github/workflows/frontend-build.yml | FILE | VERIFIED | .github/workflows/frontend-build.yml | .github/workflows/frontend-build.yml |  |  |  |  |
| FILE:.github/workflows/hardened-vps-audit.yml | FILE | VERIFIED | .github/workflows/hardened-vps-audit.yml | .github/workflows/hardened-vps-audit.yml |  |  |  |  |
| FILE:.github/workflows/hardened-vps-baseline.yml | FILE | VERIFIED | .github/workflows/hardened-vps-baseline.yml | .github/workflows/hardened-vps-baseline.yml |  |  |  |  |
| FILE:.github/workflows/hardened-vps-deploy.yml | FILE | VERIFIED | .github/workflows/hardened-vps-deploy.yml | .github/workflows/hardened-vps-deploy.yml |  |  |  |  |
| FILE:.github/workflows/integration-tests.yml | FILE | VERIFIED | .github/workflows/integration-tests.yml | .github/workflows/integration-tests.yml |  |  |  |  |
| FILE:.github/workflows/m5-sepolia-validation.yml | FILE | VERIFIED | .github/workflows/m5-sepolia-validation.yml | .github/workflows/m5-sepolia-validation.yml |  |  |  |  |
| FILE:.github/workflows/monitoring-config.yml | FILE | VERIFIED | .github/workflows/monitoring-config.yml | .github/workflows/monitoring-config.yml |  |  |  |  |
| FILE:.github/workflows/no-hardcode.yml | FILE | VERIFIED | .github/workflows/no-hardcode.yml | .github/workflows/no-hardcode.yml |  |  |  |  |
| FILE:.github/workflows/omega8-m3-grep-gates.yml | FILE | VERIFIED | .github/workflows/omega8-m3-grep-gates.yml | .github/workflows/omega8-m3-grep-gates.yml |  |  |  |  |
| FILE:.github/workflows/omega8-pii-gates.yml | FILE | VERIFIED | .github/workflows/omega8-pii-gates.yml | .github/workflows/omega8-pii-gates.yml |  |  |  |  |
| FILE:.github/workflows/ops-live-testnet.yml | FILE | VERIFIED | .github/workflows/ops-live-testnet.yml | .github/workflows/ops-live-testnet.yml |  |  |  |  |
| FILE:.github/workflows/ops-paper-mode.yml | FILE | VERIFIED | .github/workflows/ops-paper-mode.yml | .github/workflows/ops-paper-mode.yml |  |  |  |  |
| FILE:.github/workflows/probe-admin-session.yml | FILE | VERIFIED | .github/workflows/probe-admin-session.yml | .github/workflows/probe-admin-session.yml |  |  |  |  |
| FILE:.github/workflows/probe-cookies-deep.yml | FILE | VERIFIED | .github/workflows/probe-cookies-deep.yml | .github/workflows/probe-cookies-deep.yml |  |  |  |  |
| FILE:.github/workflows/rust.yml | FILE | VERIFIED | .github/workflows/rust.yml | .github/workflows/rust.yml |  |  |  |  |
| FILE:.github/workflows/security.yml | FILE | VERIFIED | .github/workflows/security.yml | .github/workflows/security.yml |  |  |  |  |
| FILE:.github/workflows/semiotic.yml | FILE | VERIFIED | .github/workflows/semiotic.yml | .github/workflows/semiotic.yml |  |  |  |  |
| FILE:.github/workflows/spec-drift-gate.yml | FILE | VERIFIED | .github/workflows/spec-drift-gate.yml | .github/workflows/spec-drift-gate.yml |  |  |  |  |
| FILE:.github/workflows/sync-vps-metadata.yml | FILE | VERIFIED | .github/workflows/sync-vps-metadata.yml | .github/workflows/sync-vps-metadata.yml |  |  |  |  |
| FILE:.github/workflows/trailblazing-5182-validation.yml | FILE | VERIFIED | .github/workflows/trailblazing-5182-validation.yml | .github/workflows/trailblazing-5182-validation.yml |  |  |  |  |
| FILE:.github/workflows/typescript.yml | FILE | VERIFIED | .github/workflows/typescript.yml | .github/workflows/typescript.yml |  |  |  |  |
| FILE:.github/workflows/unit-tests.yml | FILE | VERIFIED | .github/workflows/unit-tests.yml | .github/workflows/unit-tests.yml |  |  |  |  |
| FILE:.github/workflows/verify-admin-session-wiring.yml | FILE | VERIFIED | .github/workflows/verify-admin-session-wiring.yml | .github/workflows/verify-admin-session-wiring.yml |  |  |  |  |
| FILE:backend/Cargo.lock | FILE | VERIFIED | backend/Cargo.lock | backend/Cargo.lock |  |  |  |  |
| FILE:backend/Cargo.toml | FILE | VERIFIED | backend/Cargo.toml | backend/Cargo.toml |  |  |  |  |
| FILE:backend/api-server/Dockerfile | FILE | VERIFIED | backend/api-server/Dockerfile | backend/api-server/Dockerfile |  |  |  |  |
| FILE:backend/api-server/package.json | FILE | VERIFIED | backend/api-server/package.json | backend/api-server/package.json |  |  |  |  |
| FILE:backend/api-server/src/index.ts | FILE | VERIFIED | backend/api-server/src/index.ts | backend/api-server/src/index.ts |  |  |  |  |
| FILE:backend/math-engine/Cargo.toml | FILE | VERIFIED | backend/math-engine/Cargo.toml | backend/math-engine/Cargo.toml |  |  |  |  |
| FILE:backend/mcp-sim-engine/Cargo.toml | FILE | VERIFIED | backend/mcp-sim-engine/Cargo.toml | backend/mcp-sim-engine/Cargo.toml |  |  |  |  |
| FILE:backend/prioritization-spine/Cargo.toml | FILE | VERIFIED | backend/prioritization-spine/Cargo.toml | backend/prioritization-spine/Cargo.toml |  |  |  |  |
| FILE:backend/recon/Cargo.toml | FILE | VERIFIED | backend/recon/Cargo.toml | backend/recon/Cargo.toml |  |  |  |  |
| FILE:backend/recon/Dockerfile | FILE | VERIFIED | backend/recon/Dockerfile | backend/recon/Dockerfile |  |  |  |  |
| FILE:backend/relays-client/Cargo.toml | FILE | VERIFIED | backend/relays-client/Cargo.toml | backend/relays-client/Cargo.toml |  |  |  |  |
| FILE:backend/relays-client/Dockerfile | FILE | VERIFIED | backend/relays-client/Dockerfile | backend/relays-client/Dockerfile |  |  |  |  |
| FILE:backend/searcher-rs/Cargo.toml | FILE | VERIFIED | backend/searcher-rs/Cargo.toml | backend/searcher-rs/Cargo.toml |  |  |  |  |
| FILE:backend/searcher-rs/Dockerfile | FILE | VERIFIED | backend/searcher-rs/Dockerfile | backend/searcher-rs/Dockerfile |  |  |  |  |
| FILE:backend/sed-core/Cargo.toml | FILE | VERIFIED | backend/sed-core/Cargo.toml | backend/sed-core/Cargo.toml |  |  |  |  |
| FILE:backend/selector-api/Dockerfile | FILE | VERIFIED | backend/selector-api/Dockerfile | backend/selector-api/Dockerfile |  |  |  |  |
| FILE:backend/selector-api/package.json | FILE | VERIFIED | backend/selector-api/package.json | backend/selector-api/package.json |  |  |  |  |
| FILE:backend/semiotic-bridge/Cargo.toml | FILE | VERIFIED | backend/semiotic-bridge/Cargo.toml | backend/semiotic-bridge/Cargo.toml |  |  |  |  |
| FILE:backend/shared-rs/Cargo.toml | FILE | VERIFIED | backend/shared-rs/Cargo.toml | backend/shared-rs/Cargo.toml |  |  |  |  |
| FILE:backend/sim-core/Cargo.toml | FILE | VERIFIED | backend/sim-core/Cargo.toml | backend/sim-core/Cargo.toml |  |  |  |  |
| FILE:backend/sim-ctl/Cargo.toml | FILE | VERIFIED | backend/sim-ctl/Cargo.toml | backend/sim-ctl/Cargo.toml |  |  |  |  |
| FILE:backend/sim-ctl/Dockerfile | FILE | VERIFIED | backend/sim-ctl/Dockerfile | backend/sim-ctl/Dockerfile |  |  |  |  |
| FILE:backend/simulator-v2/Cargo.toml | FILE | VERIFIED | backend/simulator-v2/Cargo.toml | backend/simulator-v2/Cargo.toml |  |  |  |  |
| FILE:backend/token-enricher/Cargo.toml | FILE | VERIFIED | backend/token-enricher/Cargo.toml | backend/token-enricher/Cargo.toml |  |  |  |  |
| FILE:backend/token-enricher/Dockerfile | FILE | VERIFIED | backend/token-enricher/Dockerfile | backend/token-enricher/Dockerfile |  |  |  |  |
| FILE:configs/app.toml | FILE | VERIFIED | configs/app.toml | configs/app.toml |  |  |  |  |
| FILE:contracts/foundry.toml | FILE | VERIFIED | contracts/foundry.toml | contracts/foundry.toml |  |  |  |  |
| FILE:contracts/lib/forge-std/foundry.toml | FILE | VERIFIED | contracts/lib/forge-std/foundry.toml | contracts/lib/forge-std/foundry.toml |  |  |  |  |
| FILE:contracts/lib/forge-std/package.json | FILE | VERIFIED | contracts/lib/forge-std/package.json | contracts/lib/forge-std/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/contracts/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/contracts/package.json | contracts/lib/openzeppelin-contracts-upgradeable/contracts/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/foundry.toml | contracts/lib/openzeppelin-contracts-upgradeable/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/foundry.toml | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/lib/ds-test/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/lib/ds-test/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/lib/ds-test/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/forge-std/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/foundry.toml | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/foundry.toml | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/lib/ds-test/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/lib/ds-test/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/lib/ds-test/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/lib/forge-std/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/scripts/solhint-custom/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/scripts/solhint-custom/package.json | contracts/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/scripts/solhint-custom/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/package.json | contracts/lib/openzeppelin-contracts-upgradeable/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts-upgradeable/scripts/solhint-custom/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts-upgradeable/scripts/solhint-custom/package.json | contracts/lib/openzeppelin-contracts-upgradeable/scripts/solhint-custom/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/contracts/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/contracts/package.json | contracts/lib/openzeppelin-contracts/contracts/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/foundry.toml | contracts/lib/openzeppelin-contracts/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/lib/forge-std/foundry.toml | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/lib/forge-std/foundry.toml | contracts/lib/openzeppelin-contracts/lib/forge-std/foundry.toml |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/lib/forge-std/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/lib/forge-std/package.json | contracts/lib/openzeppelin-contracts/lib/forge-std/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/package.json | contracts/lib/openzeppelin-contracts/package.json |  |  |  |  |
| FILE:contracts/lib/openzeppelin-contracts/scripts/solhint-custom/package.json | FILE | VERIFIED | contracts/lib/openzeppelin-contracts/scripts/solhint-custom/package.json | contracts/lib/openzeppelin-contracts/scripts/solhint-custom/package.json |  |  |  |  |
| FILE:database/run_migrations.sh | FILE | VERIFIED | database/run_migrations.sh | database/run_migrations.sh |  |  |  |  |
| FILE:docker-compose.edge.yml | FILE | VERIFIED | docker-compose.edge.yml | docker-compose.edge.yml |  |  |  |  |
| FILE:docker-compose.yml | FILE | VERIFIED | docker-compose.yml | docker-compose.yml |  |  |  |  |
| FILE:docker/compose.dev.yml | FILE | VERIFIED | docker/compose.dev.yml | docker/compose.dev.yml |  |  |  |  |
| FILE:docker/compose.hotpath-test.yml | FILE | VERIFIED | docker/compose.hotpath-test.yml | docker/compose.hotpath-test.yml |  |  |  |  |
| FILE:docker/compose.loopback.override.yml | FILE | VERIFIED | docker/compose.loopback.override.yml | docker/compose.loopback.override.yml |  |  |  |  |
| FILE:docker/compose.noports.override.yml | FILE | VERIFIED | docker/compose.noports.override.yml | docker/compose.noports.override.yml |  |  |  |  |
| FILE:docker/compose.staging.override.yml | FILE | VERIFIED | docker/compose.staging.override.yml | docker/compose.staging.override.yml |  |  |  |  |
| FILE:docs/blueprints/enterprise_package/docker-compose.yml | FILE | VERIFIED | docs/blueprints/enterprise_package/docker-compose.yml | docs/blueprints/enterprise_package/docker-compose.yml |  |  |  |  |
| FILE:docs/contracts/foundry.toml | FILE | VERIFIED | docs/contracts/foundry.toml | docs/contracts/foundry.toml |  |  |  |  |
| FILE:edge/dev-local/Dockerfile | FILE | VERIFIED | edge/dev-local/Dockerfile | edge/dev-local/Dockerfile |  |  |  |  |
| FILE:edge/dev-local/package.json | FILE | VERIFIED | edge/dev-local/package.json | edge/dev-local/package.json |  |  |  |  |
| FILE:edge/worker/package.json | FILE | VERIFIED | edge/worker/package.json | edge/worker/package.json |  |  |  |  |
| FILE:frontend/Dockerfile | FILE | VERIFIED | frontend/Dockerfile | frontend/Dockerfile |  |  |  |  |
| FILE:frontend/app/page.tsx | FILE | VERIFIED | frontend/app/page.tsx | frontend/app/page.tsx |  |  |  |  |
| FILE:frontend/package.json | FILE | VERIFIED | frontend/package.json | frontend/package.json |  |  |  |  |
| FILE:package.json | FILE | VERIFIED | package.json | package.json |  |  |  |  |
| FILE:shared-ts/package.json | FILE | VERIFIED | shared-ts/package.json | shared-ts/package.json |  |  |  |  |
| FILE:tests/e2e/package.json | FILE | VERIFIED | tests/e2e/package.json | tests/e2e/package.json |  |  |  |  |
