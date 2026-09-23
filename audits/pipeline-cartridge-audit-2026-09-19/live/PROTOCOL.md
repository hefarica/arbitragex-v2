# PROTOCOLO DE RESILIENCIA — CHECKPOINTS EN TIEMPO REAL POR AGENTE

> Orden del operador 2026-09-19: "genera estrategias de reporte por cada agente en tiempo
> real para que por la razón que sea se caiga la red, no se pierda la información y se
> reanude en donde murieron los agentes". PERMANENTE para toda la flota Hermes.

## Capa 1 — Checkpoint incremental del agente (charter)

Todo charter de run incluye este párrafo obligatorio (PROTOCOL.md anexado):

> ANTES de tu primera herramienta: crea tu archivo de progreso
> `audits/pipeline-cartridge-audit-2026-09-19/live/<RUN-ID>-PROGRESS.md` con secciones
> `## Hecho`, `## En curso`, `## Pendiente`, `## Hallazgos` (viñetas cortas con file:line).
> TRAS CADA paso mayor (query, lectura clave, cálculo, veredicto parcial): actualiza el
> archivo ANTES de continuar (append a Hecho + Hallazgos). Tu entregable final también se
> escribe incrementalmente por secciones, no al final. Si mueres a mitad, TODO lo aprendido
> ya está en disco. Al reanudar (charter de re-dispada): LEE tu PROGRESS y CONTINÚA desde
> `## En curso` — no repitas lo ya listado en `## Hecho`.

## Capa 2 — Espejo local del orquestador (a prueba de wipe del gateway)

`mirror_hermes.ps1`/monitor del orquestador: cada 90s, por cada run vivo, hace
`GET /v1/runs/{id}` y guarda en `live/<RUN-ID>-snapshot.json` los campos
status/last_event/error + los últimos ~2KB de output acumulado. Los runs del gateway viven
en memoria de proceso (un restart de VS Code/ACP los WIPea); el espejo local es la única
copia sobreviviente. Regla: tras cualquier muerte en cadena, los snapshots deciden qué
re-disparar y con qué contexto.

## Capa 3 — Reanudación (re-dispada con contexto)

1. Muerte (503/timeout/wipe): NO re-despachar en caliente — circuit-breaker v1.2
   (oleadas ≤4, sonda de salud primero).
2. Re-dispada = charter original + `<RUN-ID>-PROGRESS.md` + snapshot JSON como contexto,
   con la orden explícita de continuar desde `## En curso`.
3. El nuevo run usa SU PROPIO nuevo RUN-ID como archivo de progreso pero COPIA/EXTIENDE
   el anterior (sección `## Heredado de <RUN-ID viejo>`), preservando la cadena de custodia.

## Registro de runs (llena el orquestador)

| RUN-ID | Charter | Snapshot | PROGRESS | Estado |
|---|---|---|---|---|
| run_e8df465f0fad4211882c72c6b11d6997 | gang3 PC-03/PC-10 | live/run_e8df465f-snapshot.json | (post-mortem) | running |
| run_1644dfb7318c44c187bb03c73bb1b097 | leaderboard+censo #7/#8 | live/run_1644dfb7-snapshot.json | (post-mortem) | running |
| run_8423b346d1c24388aa9866e04c6a5398 | funnel forense #1 | live/run_8423b346-snapshot.json | (post-mortem) | running |
| run_8ca2e7db108b482abebf779096589b31 | rutas exóticas PC-08 | live/run_8ca2e7db-snapshot.json | (post-mortem) | running |
| run_09335973f0ad43a8951571b4d99656e9 | vectores math #2 | live/run_09335973-snapshot.json | (post-mortem) | running |
| run_599dbe197e2e4e829502535eab04ad59 | PC-03 funnel audit (re-dispada tras 3×503, probe aceptada 01:5x) | live/run_599dbe19-snapshot.json | no creado (murió antes de la primera herramienta) | **failed @ 300s: "HTTP 503: dual gateway queue timeout"** — 503 sistémico CONFIRMADO; suspensión anti-amplificación re-activada; PC-04 NO despachado ese ciclo (payload listo en live/pc04_redispatch.json para el próximo) |
| run_26b6453ad0564010b810c2c1fc1d5ead | PC-03 funnel audit (ciclo 2, orden del operador "intenta nuevamente") | live/run_26b6453a-snapshot.json | no creado (murió antes de la primera herramienta) | **failed @ 302s: "HTTP 503: dual gateway queue timeout"** — MISMO patrón del ciclo 1 (muerte ~300s exactos = timeout de cola fijo del gateway). Suspensión anti-amplificación RE-ACTIVADA: no se re-dispara PC-03 ni PC-04 este ciclo; payloads listos (pc03_redispatch.json / pc04_redispatch.json). Peer -89 confirma gateway saturado (su run 6322fccc también afectado) y tampoco despacha. |
