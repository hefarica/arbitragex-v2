# CB-05 — Reporte de diseño (devops-platform · Gang Omniscience · 2026-09-07)

**Estado:** DESIGN COMPLETO (kind: design — cero ejecución, cero git).
**Entregable:** `audits/control-board-2026-09-07/CB-05-PROPUESTA-B.md` (este archivo es solo el reporte).

## Qué se diseñó

Flujo clase B (boot-time env) en tres pasos: board muestra estado verificado + botón
«Proponer cambio» → backend genera diff EXACTO y lo persiste como `propuesta_pendiente`
en `audit_logs` → operador aplica por el pipeline de deploy existente. NUNCA auto-aplica,
NUNCA escribe VPS, NUNCA genera toggle clase C §34.3 (denylist server-side §7 del
entregable).

Componentes del diseño (secciones del entregable):
- §1 Inventario clase B derivado de fuentes primarias (var→servicio→valores válidos con
  file:line del parse real) + estado VPS verificado HOY.
- §2 Invariantes INV-B1..B7 (solo-texto; diff contra estado vivo o nada; denylist §34.3
  server-side; NEXT_PUBLIC_* ⇒ rebuild RULE 03 + CSP RULE 04; auditoría previa; cierre de
  drift con evidencia; DESCONOCIDO ≠ off-falso).
- §3 Máquina de estados: pendiente→aprobada→aplicada|descartada (sin estado auto-aplicada).
- §4 Generador de diff server-side + registry declarativo en backend (no-hardcode;
  valores enum del parse real; el sistema valida, jamás sugiere valores).
- §5 Matriz de comandos compose R3 exactos (Caso A recreate vs Caso B rebuild --no-cache;
  prohibidos restart-para-env / up sin --env-file).
- §6 Formato del bloque auditable: contrato de fila `audit_logs` + payload JSON + bloque
  textual que el operador copia al pipeline (diff, servicios, comandos, verificación,
  rollback).
- §7 Denylist §34.3/secretos con rechazo backend 403.
- §8 Tres ejemplos reales: E1 `ARBX_ROUTE_SCANNER_MODE` (RU-3 — ya aplicado manualmente
  hoy 12:13Z, presentado como caso canónico), E2 `ARBX_SCORING_HARD_GATE` (dormido
  verificado HOY), E3 `NEXT_PUBLIC_WS_URL` (camino rebuild RULE 03/04; nuevo valor =
  input del operador, RULE 00).
- §9 Bordes con CB-01/02/03/04/06 · §10 evidencia verificada · §11 fuera de alcance.

## Hallazgos clave (verificados read-only)

1. **Inputs CB-01/CB-02 aún no existen** en `audits/control-board-2026-09-07/` (solo
   GOAL-WORKORDERS.md). Mitigación: inventario derivado de fuentes primarias + VPS;
   reconciliar cuando aterricen (regla de precedencia A>B en §9).
2. **`ARBX_ROUTE_SCANNER_MODE=on` YA está aplicado** (.env:136 + container + log
   `route_scanner.mode mode=on dispatch_path=orchestrator` 2026-09-07T12:13:15Z; searcher
   recreado 12:13:00Z vs flota 6h). El flujo manual que CB-05 audita acaba de ocurrir.
3. **`audit_logs` NO existe** (information_schema vacío en PG; migraciones 094–120 no la
   crean). CB-02 es dueño de la tabla; CB-05 declara el contrato de la fila (§6.1).
4. Sin drift hoy entre `.env` y env del contenedor searcher-rs en las claves relevadas.
5. Flota 25/25 healthy (docker ps 12:40Z).

## Gates de este diseño (criterios de aprobación)

- GATE-CB05-1: los 3 ejemplos producen diffs byte-exactos contra el `.env` vivo
  (old verificado o declarado ausente con default fail-safe).
- GATE-CB05-2: ningún ejemplo/endpoint toca denylist §34.3 ni secretos.
- GATE-CB05-3: el payload cubre quién/cuándo/razón/diff/comandos/verificación/rollback.
- GATE-CB05-4: no existe ningún camino de ejecución board→VPS (solo INSERT audit_logs).

## Presupuesto dominio público

0 de 5 requests HTTP manuales usados (solo ssh read-only + lectura de repo local).
