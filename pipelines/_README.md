# CI/CD Pipelines (`pipelines/`)

This directory contains the GitHub Actions workflows and deployment
automation for the ArbitrageX V2 system.

## Workflows

```
pipelines/
  .github/
    workflows/
      ci.yaml              # PR checks: lint, test, build
      build-images.yaml    # Build and push Docker images
      deploy-staging.yaml  # Deploy to staging
      deploy-prod.yaml     # Deploy to production
      e2e.yaml             # Playwright E2E tests
      security-scan.yaml   # Dependency and container scanning
      dr-drill.yaml        # Monthly DR drill automation
  scripts/
    deploy.sh              # Deployment helper
    rollback.sh            # Rollback script
    promote.sh             # Staging → production promotion
```

## CI Pipeline (`ci.yaml`)

Triggered on every pull request:

1. Lint (`cargo clippy`, `eslint`, `solhint`)
2. Unit tests (`cargo test`, `vitest`, `forge test`)
3. Build artifacts
4. Security scan (`cargo audit`, `trivy`)

## Deployment Pipeline

```
PR merge → CI pass → Docker build → Push to registry
                                      ↓
                          [manual] Deploy staging
                                      ↓
                          [manual] Deploy production
```

## Required Secrets

| Secret | Used By | Description |
|--------|---------|-------------|
| `GITHUB_TOKEN` | All workflows | GitHub-provided |
| `DOCKER_REGISTRY_TOKEN` | Build | GHCR push access |
| `KUBECONFIG` | Deploy | K8s cluster access |
| `VAULT_TOKEN` | Deploy | Vault secret access |

## Rollback

```bash
# Rollback to previous image tag
./scripts/rollback.sh api v0.1.0-previous
```

## Conventions

- Image tags use `v<semver>` format.
- Staging deploys are automatic on `main` merge.
- Production requires manual approval.
- All deploys require green E2E tests.