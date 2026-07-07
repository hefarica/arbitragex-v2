# EXECUTION PLAN

## Instrucción Recibida
ACTÚA COMO OMEGA POST-SCAFFOLD MATERIALIZER + CI/CD + VPS EXECUTION TEAM AGENT. Tomar el paquete OMEGA post-scaffold completo, verificarlo, entenderlo, aplicarlo al repo real hefarica/arbitragex-v2 sin destruir nada, cerrar todos los GAPs P0, ejecutar CI/CD real en GitHub Actions, mapear/validar VPS, desplegar controladamente y dejar evidencia ejecutable de cada paso.

## Alcance
Fases 0 a 14 de la guía OMEGA post-scaffold, asegurando que todos los GAPs P0 y el CI/CD estén resueltos antes del merge final.

## Fuera de Alcance
Cualquier refactor masivo no requerido para cumplir con la doctrina. Cambios destructivos o despliegues sin validación CI previa. Modificaciones que debiliten la seguridad o testing.

## Archivos Candidatos
- Archivos en `.github/workflows/`
- Código de aplicación (`frontend/`, `backend/`, `contracts/`, `crates/`) que necesiten resolverse según los comentarios Codex.
- Documentación (`docs/`, `mkdocs.yml`)
- Scripts (`scripts/`)
- Infraestructura (`infra/`)

## Comandos Candidatos
- `gh pr checks`
- `npm run typecheck`
- `npm run lint`
- `cargo clippy`
- `docker compose config`
- `gh run list`

## Validaciones
1. GitHub Actions GREEN (13 required checks).
2. Comentarios Codex resueltos.
3. VPS documentado sin modificaciones destructivas.
4. Novedades registradas si hay bloqueos.

## Riesgos
- Fallos en PR 103 por el rebase.
- Falta de infraestructura en el VPS que bloquee la ejecución de los scripts de deploy.
- Problemas con integraciones E2E o tests.

## Criterio de Éxito
- Todo el workflow ejecutado paso a paso con su documentación respectiva en la carpeta `.omega/`.
- CI/CD en GitHub en verde.
- Novedades explicadas si hay bloqueos y esperando decisión humana.
