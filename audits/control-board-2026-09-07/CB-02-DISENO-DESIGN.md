# CB-02-DISENO — Reporte de diseño (ecc:code-architect · Gang Omniscience · 2026-09-07)

**Estado:** DESIGN COMPLETO (kind: design — cero ejecución, cero git, cero ediciones a código
de producción; los archivos bajo claim quedaron intactos como protección para el apply).
**Entregable:** `audits/control-board-2026-09-07/CB-02-DISENO.md` (este archivo es solo el reporte).

## Qué se decidió (las 7 decisiones del charter)

1. **Claves Redis clase A (conjunto EXACTO, D1):** `arbx:killswitch` (existente, viva — se
   reusa, no se duplica) + **UNA nueva**: `arbx:controlboard:route_scanner` (run/halt del
   worker RU-3 ya spawn-eado) + soporte: hb `arbx:controlboard:route_scanner:hb` (SETEX 75s,
   lado VERIFICADO), hash `arbx:controlboard:approved` (registro de aprobación CB-04), publish
   `arbx:config:boot_census` (lado DECLARED clase B), canal opcional
   `arbx:controlboard:changes`. Convención = clon exacto del patrón kill-switch (poll
   TTL-cacheado 1s, fail-safe clave-ausente=comportamiento desplegado, pub/sub opcional,
   lectura cero-alloc bool de cache).
2. **Clase B (D2):** los **53 CanonicalKnobs TODOS quedan boot-time** (constraints: §37
   Nivel-1 route-discovery congelado; invariantes cruzados suma=1.0; declarativos-only
   §34.1; knobs gated sin consumidor; PUT booleano por contrato) — el board los muestra como
   UNA fila agregada enlazada a `/api/v1/config/canonical-knobs`. Idem gates de workers
   (orchestrator/cartridge mode §34.2, mempool, native engines, pool enum, scoring, macro
   gate, SIM_BACKEND, WS purge, CSP, archivers). La ÚNICA promoción A = run-gate del
   route_scanner (el algoritmo §37 no se toca; solo se omite invocarlo).
3. **Wiring Rust file:line (D3):** NUEVO `shared-rs/src/control_board.rs`
   (RuntimeToggleClient read-only, mirror killswitch.rs — killswitch.rs queda INTACTO) ·
   `main.rs:361-384` +publish boot census · `route_scanner_worker.rs:786-854` (client en
   spawn, default=boot mode), `:702-721` (poll antes de `scan_block`:710 + SETEX hb/bloque +
   halt con debug! y counter).
4. **API GET/PUT (D4):** NUEVO `api-server/src/routes/control-board.ts` + montaje index.ts
   junto a service-control (antes de mountStubs). GET ensambla declared(approved→census) vs
   verified(hb/live-key/self-env/census) + audit metadata (DISTINCT ON sobre audit_log);
   Redis caído → 503; PG caído → columnas audit null honestas. PUT: razón OBLIGATORIA
   server-side (zod min3/max500), registry-check (404), denylist clase B/C (403 con cita),
   no-spawned (409), **writeAudit PRIMERO**, Redis SET+PUBLISH+HSET idempotente, fallo Redis
   → audit compensatorio + 503 "audited, NOT applied", 200 devuelve snapshot fresco (contrato).
   Edge: CERO parches (index.ts:1594-1595 ya construido).
5. **DDL (D5):** `121_control_board_audit_logs.sql` **NO se crea — premisa refutada** (ver
   hallazgo 1 abajo): la tabla `audit_log` (SINGULAR) existe (migración 011, particiones 019,
   PII 053/055/070) con writeAudit vivo; duplicarla violaría §37 P-∅. CB-05 mapea 1:1.
6. **Denylist §34.3 + soberanía (D6):** 403 server-side a todo PUT no-clase-A (curl directo
   incluido); filas C (live_exec_terminus, relays_submit, signer_capital) con candado siempre;
   único writer = sesión admin-token (V-AT-1 cookie→adminProxy→x-arbx-admin-token→
   requireAdminToken safeTokenEqual); sin camino service-token; default-deny y
   MainnetRefused INTOCABLES.
7. **Hooks CB-04 (D7):** hash approved + campos declared/verified en snapshot + vocabulario
   de acciones audit (`control_board.toggle/.toggle_failed` + reservados
   `.drift_detected/.reverted`) + boot census para env-drift; revert = re-SET approved→clave.

## Hallazgos clave (verificados read-only)

1. **REFUTACIÓN a CB-05 (§6.1/hallazgo 3):** "audit_logs no existe" fue un miss de nombre
   PLURAL. `audit_log` existe: migración 011 + 019 particionada (**3 particiones vivas en VPS,
   confirmado psql HOY**) + writeAudit api-server/src/index.ts:362-383 (usado por killswitch,
   blacklist, circuit breakers, service-control). Impacto: cero migración necesaria; el flujo
   CB-05 funciona igual (solo cambia el nombre de tabla destino).
2. **Confirmación a CB-02-RUST-APPLY:** knobs 100% boot-time (from_env 1 vez, main.rs:361);
   su §3 fue el mapa correcto — este diseño es su desbloqueante directo (§3-§4 del entregable
   = insumo del re-despacho del apply).
3. **Contrato cliente encaja SIN cambios de tipos**; se ordena UN parche acotado aditivo:
   `isBoardToggleable()` pura en ControlBoardLed.tsx (filas A-link externas no deben pintar
   toggle — isToggleable las marcaría y el PUT las rechazaría).
4. **Semántica invertida documentada:** killswitch on = ARMADO = detención (name/description
   del registry lo explicitan — el LED dice la verdad de la clave, RULE 00).
5. Registry v1 = 27 filas con cita file:line por entrada (DISENO §8); pendientes CB-01
   declarados sin fila (RULE 00).

## Gates de este diseño (criterios de aprobación del apply)

GATE-CB02-1 contrato (Zod+400/403/404/409) · GATE-CB02-2 fail-safe Rust (ausente→default,
TTL 1 GET/s, hb expirada→DESCONOCIDO) · GATE-CB02-3 integridad (cargo check/clippy/fmt,
tsc, vitest, XLEN delta=0) · GATE-CB02-4 E2E CB-06 (toggle off → halted ≤2 bloques → LED rojo
sin restart + fila audit con razón). Invariantes INV-CB02-1..7 en el entregable §11.

## Reconciliación con el apply TS paralelo (§15 del entregable — LEER ANTES DEL PR)

`CB-02-API-APPLY.md` (ecc:typescript-reviewer) aplicó la mitad TS ANTES de este diseño
(declarado honestamente en su §0) y dejó merge-points explícitos. Adjudicación R1-R10:

- **R1 (CRÍTICA — refutación con teeth):** su migración 121 `audit_logs` NUEVA **NO se crea**
  y su `UPDATE audit_logs SET payload…` es **imposible por diseño** (011:21 REVOKE UPDATE,
  DELETE FROM arbx_rw sobre `audit_log`; y viola append-only aunque la tabla fuera nueva).
  Retarget: columna-por-columna a `audit_log` existente + **INSERT compensatorio**
  `control_board.toggle_failed` en lugar del UPDATE. Es el único retarget que toca queries.
- **R3:** GET adopta su arquitectura census (`arbx:config:control_board`, CB-01 publica;
  ausente → snapshot vacío), PERO `verified_*` de clase A se computa LIVE (hb/clave) — un
  census publicado-por-herramienta se stalea y el LED verificado debe ser vivo.
- **ADOPTADO del apply:** valor `"true"/"false"` estricto (R2), B→409 y terminus-denylist
  403 antes del check de clase (R4), pool-null→503 primero y INSERT-falla⇒sin-Redis (R8),
  reason min1/max1000 (R9), no-op re-assert auditado.
- **Decisión de diseño que estrecha el alcance:** `killswitch` pasa a fila **A-LINK** (NO
  board-toggleable — R5): su clave es JSON KillSwitchState, incompatible con el formato del
  board, y GOAL :37-38 manda enlazar, no duplicar. **Conjunto board-toggleable clase A v1 =
  exactamente `route_scanner_multihop`** + los que el census CB-01 añada en el namespace
  `arbx:controlboard:` con consumidor citado. Parche §9 refinado a prefijo
  `redis:arbx:controlboard:` (R6). Pub/sub deferido (R7).
- **Estado de mitades:** TS = aplicada con retargets R1/R3 (acotados; 22 tests se ajustan);
  **Rust = PENDIENTE íntegro** (spec §3: RuntimeToggleClient string-based + wiring
  route_scanner_worker :786-854/:702-721 + hb + boot census) — re-despachar;
  CB-01 = PENDIENTE (su census debe satisfacer el contrato de contenido §8 + R3).

## Presupuesto dominio público

0 de 5 requests HTTP manuales (solo repo local + ssh `arbx` read-only: psql SELECT, redis-cli
GET/SCAN — nada mutado, §33). Cero ediciones a archivos de producción (los 2 archivos del
apply paralelo son suyos, intactos; mis 5 archivos bajo claim intactos — verificado
`git status`).
