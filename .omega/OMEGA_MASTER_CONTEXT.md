# OMEGA MASTER CONTEXT

## 1. Etapas Post-Scaffold
0. Verificar paquete (Hashes).
1. Entender documentos.
2. Auditar repo.
3. Sincronizar Codex.
4. Aplicación diff-first.
5. Quemar placeholders.
6. Validación local canónica.
7. Validar DB/migraciones.
8. CI/CD GitHub Actions.
9. Mapear VPS.
10. Cerrar GAPs P0.
11. Deploy controlado.
12. Validación navegador.
13. Reportes (Diátaxis, ADRs).
14. Merge Readiness.

## 2. GAPs P0
1. Vault init en VPS.
2. Grafana deploy.
3. DR drill dry-run.
4. MkDocs publicar.
5. PR pendiente con review.
6. ADRs accepted.
7. Performance budget p99 < 500ms.
8. Paper Shadow 14 días P&L verde.

## 3. Stack mínimo canónico
- Node 20/22 + Rust 1.78
- PostgreSQL 16 + Redis 7
- Next.js 14 App Router, shadcn/ui
- Playwright para E2E
- GitHub Actions para CI/CD
- Docker Compose, Nginx, Let's Encrypt

## 4. Criterio Unificado de TERMINADO
- GitHub Actions GREEN.
- Cero Mocks. R8 Fail-Honest.
- VPS documentado.
- Deploy validado.

## 5. Herramientas obligatorias
- sha256sum para hash verification.
- gh CLI para PR view/checks.
- pnpm / npm run para local checks.
- cargo para Rust.
- Playwright.
- Docker compose.
