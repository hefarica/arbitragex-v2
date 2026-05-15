# C10 Fase 0 — Auditoría VPS Wiring (READ-ONLY)

> Si no puedes medirlo, no está cableado. Si no puedes revertirlo, no
> está listo. Si no puedes trazar entrada y salida, no es HFT: es un
> dashboard.
> — Operador

## 1. Qué es C10

C10 es la arquitectura CI/CD + observabilidad HFT-grade del ecosistema
Ω-S5++. Su propósito es garantizar Mirror Fidelity entre el repositorio
canónico en GitHub y la topología desplegada en el VPS arbx, con
trazabilidad determinista, reversibilidad sub-segundo y observabilidad
de extremo a extremo.

C10 se descompone en fases incrementales:

- **Fase 0** — Auditoría read-only del estado actual (este PR).
- **Fase 1** — Blindaje de metadata Git en VPS (reparación dirigida,
  bajo autorización explícita del operador).
- **Fase 2** — Suite de 8 workflows productivos (deploy, rollback,
  smoke, secrets-audit, etc.).
- **Fase 3** — Matrices de validación cruzada multi-servicio.
- **Fase 4** — Métricas SLO/SLI y dashboards de Topological Yield.

## 2. Por qué Fase 0 es read-only

Auditorías informales previas detectaron drift entre la topología
canónica del repo y el estado real del VPS:

- Remote `origin` apuntando a `/opt/git/arbitragex-v2.git` (destino
  inexistente).
- HEAD del VPS desincronizado respecto a `origin/main`.
- Workflows untracked en el árbol de trabajo VPS.
- Working tree sucio con stashes operador no auditados, que ya
  provocaron un build con código viejo en una iteración anterior.

Estos hallazgos requieren **diagnóstico antes que reparación**.
Cualquier mutación ciega (un `git reset --hard`, un `docker build`
prematuro) puede destruir trabajo operador o dejar el servicio en
estado inconsistente. Fase 0 audita; Fase 1 repara con plan firmado.

## 3. Cómo correr la auditoría

Manual desde tu terminal (recomendado para la primera ejecución):

```bash
gh workflow run audit-vps-wiring.yml --ref main
```

Para correr referenciando un SHA específico (campo informativo, Fase 0
no sincroniza):

```bash
gh workflow run audit-vps-wiring.yml --ref main -f target_sha=<sha>
```

El workflow corre en `ubuntu-latest` y conecta al VPS arbx mediante
SSH usando el secret `VPS_SSH_KEY`. No requiere runners self-hosted.

> Nota: el cron `'0 8 * * *'` está deshabilitado por defecto. Tras
> validar el primer reporte manualmente, descoméntalo para habilitar
> auditoría diaria a las 08:00 UTC.

## 4. Cómo leer el reporte

Cada ejecución produce dos artefactos descargables:

- `audits/AUDIT-<short_sha>-<utc_timestamp>.json` — JSON estructurado
  con las 7 capas y el bloque de análisis de riesgo.
- `audits/AUDIT-<short_sha>-<utc_timestamp>.md` — Reporte markdown
  legible.

Ambos archivos se publican como artifact `audit-fase0-<run_number>`
(retención 30 días). Adicionalmente, el reporte markdown se renderiza
en el `Summary` del run de GitHub Actions — visible sin descargar
nada.

Para descargar:

```bash
gh run list --workflow=audit-vps-wiring.yml --limit 5
gh run download <run-id>
```

## 5. Glosario de las 7 capas

| Capa | Nombre | Qué mide |
|---|---|---|
| A | GitHub (runner) | Default branch, HEAD, workflows declarados, secret names, environments, branch protection |
| B | VPS Git metadata | HEAD, branch, remote URL, stashes, refs locales y remotos |
| C | VPS Filesystem | Permisos y tamaño de `.env`, listado de `docker/`, workflows presentes en VPS, espacio disco |
| D | Docker | Containers activos, imágenes, tags backup (`pre-*`, `backup`, `rollback`), compose file usado |
| E | Health endpoints | Curl read-only a edge (`8787`), api-server (`8080`), frontend (`3000`), prometheus (`9090`) |
| F | Routing | Configuración nginx (`server_name`, `proxy_pass`, `location`) si el container existe |
| G | Análisis de riesgo | Booleanos derivados: remote roto, HEAD desincronizado, working tree sucio, stashes presentes, permisos `.env` |

Todas las capas son **read-only**. El workflow no ejecuta ningún
comando que mute estado en el VPS: prohibido `git pull/fetch/push/
reset/checkout`, prohibido `docker pull/build/up/stop/rm`, prohibido
`rsync/scp/sudo`, prohibida cualquier escritura en
`/opt/arbitragex-v2/**`. El único `docker exec` permitido es
`nginx -T`, que es lectura pura de configuración.

## 6. Redacciones de seguridad

- Jamás se ejecuta `cat .env` ni `env | grep`.
- Lista de secrets se obtiene por nombre únicamente (`gh secret list
  --json name`), nunca por valor.
- Tokens largos (≥32 caracteres alfanuméricos) en cualquier salida
  incidental se redactan con `sed 's/[A-Za-z0-9_\-]\{32,\}/***REDACTED***/g'`.
- Llave SSH se escribe en `~/.ssh/deploy_key` (perm 600) y se elimina
  en un step `if: always()`.

## 7. Qué hacer si el veredicto es BLOCKED

El reporte termina con un **veredicto**: READY para Fase 1 (SI) o no
(NO). Si el veredicto es NO, la sección "Bloqueos detectados" lista
cada hallazgo con su acción recomendada.

Procedimiento estándar:

1. Descarga el artifact JSON + Markdown.
2. Revisa la tabla principal y compárala con la columna "Acción
   recomendada".
3. Si los bloqueos son los esperados (drift conocido) → autorizar Fase
   1 con un PR posterior que repare la metadata Git del VPS.
4. Si aparecen bloqueos no esperados (containers caídos,
   health 5xx, backup tags faltantes) → no autorizar Fase 1;
   investigar primero.

**Nunca** mergees Fase 1 sin antes haber leído el reporte Fase 0
completo. La frase del operador no es decorativa: medir antes de
mover.

## 8. Roadmap

```
Fase 0 (este PR)      → audit-vps-wiring.yml (read-only)
        ↓
Fase 1 (PR futuro)    → metadata-fix.yml (reparación dirigida con dry-run + autorización)
        ↓
Fase 2 (PR futuro)    → deploy/rollback/metrics-smoke/secrets-audit (8 workflows productivos)
        ↓
Fase 3 (PR futuro)    → matrices de validación cruzada (multi-servicio, multi-entorno)
        ↓
Fase 4 (PR futuro)    → métricas SLO/SLI + dashboards (Topological Yield, Spectral Gap, Decoherencia)
```

Cada fase requiere autorización explícita del operador y reporte
Fase 0 aprobado como precondición.

## 9. Invariantes operador

- **Operador Parametrization:** todas las decisiones de mutación
  requieren firma humana del operador (Hector Fabio Riascos Castro).
- **Crucible Sovereignty:** cap $0.00 USD — ninguna operación
  productiva interactúa con valor monetario hasta autorización
  explícita.
- **Ghost Protocol:** `ExecutionSigner.balance ≡ 0` durante toda la
  vida del repositorio.
- **Mirror Fidelity:** el repo es la fuente única de verdad; el VPS
  debe converger al repo, jamás al revés.
- **Stochastic Convergence:** la varianza del estado VPS vs repo debe
  ser monótona no-creciente entre auditorías sucesivas.
