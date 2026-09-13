# /goal — CONTROL BOARD MAESTRO (orden del operador 2026-09-07)

> "Ya estoy harto que hayan muchas cosas pero que se apaguen y no me dé cuenta, o que estén
> implementadas pero no cableadas. Necesito que sean todas las más posibles de este tipo de
> implementaciones reales en cuanto al arbitraje, visibles para mí y que las pueda activar.
> Wireadas, conectadas, cableadas end-to-end y bindadas para que nada las desconecte sin mi
> aprobación." — Operador, 2026-09-07

**Contexto vivo que origina la orden:** RU-3 (route_scanner multihop) llevaba meses construido
y anclado — jamás encendido (`ARBX_ROUTE_SCANNER_MODE` ausente). Descubierto al investigar por
qué el feed no muestra hops 2-6.

## Kanban

| WO | Ítem | Acción | Estado |
|---|---|---|---|
| CB-01 | **Inventario exhaustivo** de TODOS los módulos/switches reales de procesamiento (searcher workers, orchestrator/cartridge/discovery/scanner modes, native engines, sim backend, WS subsystems, purge/retention knobs, CSP gate, service control, latency series, kill-switch…) — cada uno con: nombre, qué hace, fuente de verdad de su estado HOY (env Redis metric log), clasificación (A=runtime-pollable / B=boot-time env / C=operator-gated §34.3 = LOCKED sin toggle), y estado actual verificado en VPS | Censo read-only con evidencia | PENDIENTE |
| CB-02 | **Control plane runtime**: para clase A, claves Redis de runtime-config que los workers ya pollan o se les añade poll (patrón kill-switch/canonical_knobs) + API `GET/PUT /api/v1/control-board` (x-arbx-admin-token) con auditoría en audit_logs | Diseño + apply | **NO-OP honesto (CB-02-RUST-APPLY)**: el apply llegó SIN diseño (CB-01 censo + CB-02-DISENO ausentes) y se negó a inventar trabajo (RULE 00/P-∅ — correcto). **NOTA orquestador: CB-01 censo y CB-02-DISENO son PRERREQUISITOS — la mesa debe despacharlos ANTES de reintentar el apply** |
| CB-03 | **Página frontend `/control`**: tablero con LED fluor verde(encendido)/rojo(apagado) por módulo, toggle con confirmación + razón, badges de clase (A live / B requiere-restart / C LOCKED§34.3), estado verificado vs declarado (drift alert si difieren), timestamps, link a telemetría viva | Diseño + apply + tests | PENDIENTE |
| CB-04 | **Drift-guard**: el board es el registro de aprobación del operador; si un módulo cambia de estado por fuera del board (env drift, deploy, ediciones), alerta visible (banner + endpoint de diff) | Diseño + apply | PENDIENTE |
| CB-05 | **Clase B (boot-time env)**: el board muestra estado + permite "proponer cambio" que genera el diff exacto (env + restart) para el pipeline de deploy aprobado por el operador — NUNCA auto-aplica | Diseño | PENDIENTE |
| CB-06 | **Verificación browser completa** (yo orquestador + personas): cada LED refleja la realidad (prender/apagar un módulo A y ver el cambio propagar), 0 regressions, capturas | E2E | PENDIENTE |

## Reglas duras
- §34.3: capital/broadcast/live-flip = **Categoría C: LED gris-candado, SIN toggle, con texto de autorización** — jamás toggleable desde UI.
- RULE 00: estado "desconocido" se muestra como DESCONOCIDO, jamás apagado-verdadero falso.
- Toda escritura: audit_logs con quién/cuándo/por qué (razón obligatoria en el toggle).
- NO-GIT hasta gates; PRs per-WO; browser-verify end-to-end de cada toggle.

## EXTENSIÓN (orden del operador, 2026-09-07 — misma sesión)

1. **SEMILLA confirmada por el operador** (buscar MÁS allán de esta lista — exhaustivo):
   ARBX_ROUTE_SCANNER_MODE (RU-3) · ARBX_ROUTE_DISCOVERY_MODE (NO-ACTIVE pin — mostrar el pin
   honestamente como "arquitectura radar, sin modo activo por diseño") · ARBX_ORCHESTRATOR_MODE ·
   ARBX_CARTRIDGE_MODE · ARBX_CSP_ENFORCE · SIM_BACKEND · Kill-switch (ya tiene página — el board
   lo ENLAZA, no lo duplica) · ARBX_NATIVE_ENGINES · ARBX_WS_CONSUMER_PURGE* · terminus paper/live
   (Categoría C LOCKED §34.3) · ARBX_SERVICE_CONTROL · knobs de retention · y TODO lo que el
   censo CB-01 encuentre (math registry, rate limits, watchlist, pool_sync interval, hot-path
   knobs ARBX_KNOB_*, alertmanager/webhooks, etc.).
2. **Pestañas clasificadas**: el board organiza los módulos por pestañas (Detección/Discovery,
   Evaluación/Matemática, Ejecución/Terminus (C-locked), Infra/Observabilidad, Seguridad,
   Frontend). Cada LED con estado vivo + fuente.
3. **SOBERANÍA ("ni Dios mueva si no es mi deseo")**: sólo el operador (sesión admin-token)
   puede cambiar un interruptor. Drift-guard reforzado: cualquier cambio de estado de módulo por
   fuera del board (env drift, deploy, script, agente) = **alerta audible en el board + registro
   en audit_logs + diff visible**; para clase A (runtime Redis) el board **revierte** al último
   valor aprobado por el operador (el board es el registro de aprobación). Nadie más escribe.
4. **Badge discreto de defaults** (instructivo del operador): panel plegable con la tabla
   "postura de máximo potencial" — valores default recomendados por módulo para robustez
   máxima + potencial máximo, con nota honesta por módulo (qué gana/pierde cada valor). El
   operador la lee desde el propio board.

