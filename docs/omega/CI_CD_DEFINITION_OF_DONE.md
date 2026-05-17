# CI/CD Definition of Done

> **Doctrina permanente**: Ningún cambio está terminado si no existe una ruta CI/CD completa, verificable y documentada.
> **Fecha de vigencia**: 2026-05-17
> **Autoridad**: Operador Lead R8

---

## REGLA MADRE

Ningún cambio está terminado por existir localmente.
Ningún cambio está terminado por compilar localmente.
Ningún cambio está terminado por hacer commit.
Ningún cambio está terminado por hacer push.
Ningún cambio está terminado por abrir PR.
Ningún cambio está terminado porque algunos checks pasen.
Ningún cambio está terminado si hay checks expected, pending, running, skipped, failed o review required.
Ningún cambio está terminado si no pasó por la ruta CI/CD correspondiente.

**El remoto manda.**

---

## RUTA OBLIGATORIA PARA TODO CAMBIO

1. Crear o usar branch correcto.
2. Hacer cambio mínimo.
3. Commit atómico.
4. Push al remoto.
5. Abrir o actualizar PR.
6. Verificar HEAD remoto.
7. Ejecutar workflows existentes.
8. Esperar todos los required checks.
9. Descargar logs/artifacts si falla.
10. Corregir causa raíz.
11. Repetir hasta verde.
12. Resolver review required.
13. Merge sin bypass.
14. Verificar main.
15. Ejecutar deploy si aplica.
16. Validar VPS si aplica.
17. Reportar evidencia final.

No se permite saltar pasos.

---

## ESTADOS PERMITIDOS DE REPORTE

| Estado | Condición |
|--------|-----------|
| `GREEN_FINAL` | Todos los checks SUCCESS, cero pending/expected/running/failure, review resuelto, merge/deploy validado si aplica. |
| `WAITING_CHECKS` | Hay checks pending, expected, queued o running. |
| `FAILED_CHECKS` | Hay failure, error, cancelled o timeout. |
| `BLOCKED_BY_REVIEW_REQUIRED` | Checks verdes, pero falta approval válido. |
| `BLOCKED_BY_BRANCH_PROTECTION` | La protección de rama impide merge por regla activa. |
| `BLOCKED_BY_MISSING_AUTH` | No hay gh/auth. |
| `BLOCKED_BY_INFRA` | Falla VPS, SSH, Docker, secrets, healthcheck o runtime. |
| `BLOCKED_BY_SECRET` | Falta secret o valor real. |

**Nunca usar**: "terminado", "listo", "cerrado", "completo", "verde", "sin bloqueos" — si no corresponde exactamente a `GREEN_FINAL`.

---

## COMANDOS OBLIGATORIOS POR PR

Antes de reportar estado:

```bash
gh pr view <PR> --repo hefarica/arbitragex-v2 \
  --json headRefOid,mergeStateStatus,mergeable,reviewDecision,statusCheckRollup

gh pr checks <PR> --repo hefarica/arbitragex-v2

gh run list --repo hefarica/arbitragex-v2 --branch <BRANCH> --limit 20
```

El reporte debe incluir:

```
Repo:
Branch:
PR:
HEAD remoto:
Checks SUCCESS:
Checks EXPECTED:
Checks PENDING:
Checks RUNNING:
Checks FAILURE:
reviewDecision:
mergeStateStatus:
Estado final:
Siguiente acción:
```

---

## WORKFLOWS EXISTENTES: USAR, NO RECREAR

No recrear workflows ya establecidos. Usar los workflows existentes:

- `e2e` / `playwright`
- `deploy-vps.yml`
- `hardened-vps-deploy.yml`
- `security.yml`
- `rust.yml`
- `typescript.yml`
- `frontend-build.yml`
- `unit-tests.yml`
- `no-hardcode.yml`
- `foundry.yml`
- `dockerfile-audit.yml`
- `omega8` grep/PII gates

Si un workflow falla: leer logs, descargar artifacts y corregir causa raíz.

Si un workflow no corre: investigar triggers, branch filters, paths, workflow disabled o required_status_checks desactualizado.

No inventar otro workflow paralelo para evadir el existente.

---

## PROHIBICIONES ABSOLUTAS

- Reportar terminado con checks pendientes.
- Reportar terminado con checks expected.
- Reportar terminado con review required.
- Usar `--admin`.
- Hacer bypass.
- Force push sin autorización.
- Desactivar branch protection para ocultar fallos.
- Quitar required checks para pasar.
- Usar `continue-on-error` para maquillar.
- Borrar tests.
- Usar skip global.
- Usar mocks para falsificar salud.
- Usar `.first()` para esconder strict mode.
- Fabricar UP falso.
- Ocultar DOWN real.
- Tocar secrets sin causa explícita.
- Cambiar `VPS_SSH_USER` sin validar llave correspondiente.
- Hacer deploy desde PR.
- Declarar éxito local.

---

## SI HAY CHECKS EXPECTED

Si aparece `Expected — Waiting for status to be reported`, el PR NO está listo.

Acciones:

```bash
gh pr checks <PR> --repo hefarica/arbitragex-v2 --watch --interval 30
```

Si no arranca:

```bash
gh workflow list --repo hefarica/arbitragex-v2
gh run list --repo hefarica/arbitragex-v2 --branch <BRANCH> --limit 20
gh pr view <PR> --repo hefarica/arbitragex-v2 --json statusCheckRollup
```

No mergear. No deployar. No cerrar.

---

## SI HAY CHECK FALLIDO

Descargar evidencia:

```bash
RUN_ID=$(gh run list --repo hefarica/arbitragex-v2 --branch <BRANCH> --limit 1 --json databaseId --jq '.[0].databaseId')
mkdir -p ./ci-artifacts
gh run view "$RUN_ID" --repo hefarica/arbitragex-v2 --log-failed > ./ci-artifacts/failed.log
gh run download "$RUN_ID" --repo hefarica/arbitragex-v2 -D ./ci-artifacts
```

Buscar causa raíz:

```bash
grep -Ei "error|failed|failure|timeout|strict mode|Application error" ./ci-artifacts/failed.log | head -150
```

Luego: corregir → commit mínimo → push → esperar checks → repetir.

No hacer rerun infinito si el fallo es de código.

---

## SI HAY REVIEW REQUIRED

Permitido:

1. Solicitar review real de colaborador.
2. Solicitar autorización explícita del operador para excepción temporal.
3. Documentar before/after si se toca branch protection.

Prohibido: `--admin`, bypass, quitar checks, dejar branch protection relajada.

---

## RUTA OBLIGATORIA PARA DEPLOY

Deploy **solo desde main**.

1. PR verde.
2. Review resuelto.
3. Merge a main.
4. Verificar main.
5. Ejecutar `deploy-vps.yml`.
6. Verificar run.
7. Validar VPS.

Validación VPS:

```bash
curl -fsS http://127.0.0.1:8080/health
curl -fsS http://127.0.0.1:8787/health
curl -fsS http://127.0.0.1:5173
curl -fsS http://127.0.0.1:3000/api/health
stat -c "%U:%G %a %n" /run/secrets/arbx/grafana_admin_password
```

---

## REGLA PARA RUNTIME SECRETS

Antes de cualquier `docker compose up` en producción:

```bash
if [ "$(whoami)" = "root" ]; then
  python3 /usr/local/sbin/arbx-materialize-runtime-secrets.py
else
  sudo /usr/bin/python3 /usr/local/sbin/arbx-materialize-runtime-secrets.py
fi
```

Motivo: `/run` es volátil y se borra en reinicios.

No usar `chmod 644` para secrets. No imprimir `.env`. No imprimir `/run/secrets`.

---

## FILOSOFÍA

Ningún agente puede volver a decir "terminado" si no existe una ruta CI/CD completa y verde.

Cada cambio debe dejar una ruta.
Cada ruta debe dejar evidencia.
Cada evidencia debe venir del remoto.

**El remoto manda.**
