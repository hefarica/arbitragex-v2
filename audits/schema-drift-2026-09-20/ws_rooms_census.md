# WO-D5 — Censo de rooms/channels WS del api-server (2026-09-20)

> Auditoría de destinos de datos, parte 2: WebSocket del api-server.
> Método: enumerar cada room suscribible (handler `subscribe:*`) y verificar
> (a) productor (`io.to(room).emit` o puente Redis→WS) y (b) consumidor
> (hooks/sockets del frontend). Todo veredicto con evidencia archivo:línea
> (líneas pre-fix, commit fb547bb6 las desplaza ~2).

## Matriz room → handler → productor → consumidor

| Room | Handler (`socket.on`) | Productor | Consumidor frontend | Veredicto |
|---|---|---|---|---|
| `opportunities` | websocket.ts:410 | `new_opportunity` emit websocket.ts:498 (PG LISTEN opportunities_channel) | useOpportunitiesStream (WS en vivo) | ✅ VIVO ambos extremos |
| `convergence` | websocket.ts:423 | `convergence_signal` websocket.ts:531 `broadcastConvergenceSignal` (Redis arbx:signals:convergence) | panel convergencia | ✅ VIVO |
| `telemetry` | websocket.ts:431 (CARTRIDGE_TELEMETRY_ROOM) | `telemetry` websocket.ts:759 `broadcastCartridgeTelemetry` (Redis arbx:cartridge:telemetry) | Strategy Forge UI | ✅ VIVO |
| `route_discovery` | websocket.ts:440 (ROUTE_DISCOVERY_TELEMETRY_ROOM) | `route_discovery_telemetry` websocket.ts:1192 (Redis arbx:route_discovery:telemetry) | Route Discovery panel | ✅ VIVO |
| `runtime_ack` | websocket.ts:462 (per-socket flag + ack callback ROOM-AUTH-01) | `runtime_ack` broadcast websocket.ts:600 `broadcastRuntimeAck` (POST /api/system/runtime-ack) | useActionState 12-estados | ✅ VIVO |
| `prices` | prices-stream.ts:104 (`subscribe:prices`, registrado index.ts:1787) | delta-streaming torre on-chain (programa precios soberanos) | usePricesStream.ts | ✅ VIVO — **falso positivo inicial descartado** (el handler vive en otro archivo, no en websocket.ts) |
| `metrics` | websocket.ts:415 | **NINGUNO** — cero `io.to('metrics').emit` en todo api-server | **NINGUNO** — useGatesStatus.ts usa HTTP polling, no WS | ❌ **WIRE MUERTO** → removido en PR #616 |

## Detalle del wire muerto `metrics`

- Handler: `socket.on('subscribe:metrics')` → `socket.join('metrics')` (websocket.ts:415-418, pre-fix).
- Cero productores: grep `to\(['"]metrics['"]\)` en todo el repo → 0 hits en emisión.
- Cero consumidores: grep frontend `subscribe:metrics` → 0; "metrics" en hooks solo aparece en useGatesStatus.ts que hace HTTP polling.
- AsyncAPI YA lo documentaba como DORMANT ("clients may join, but the server emits nothing to it") — apis/asyncapi.yaml:82-90 (pre-fix).
- Referencias totales al momento del fix: websocket.ts:415 (handler), websocket.ts:261 (comentario auth), websocket-rooms.test.ts:54+122 (harness réplica), apis/asyncapi.yaml:82 (contrato), tests/e2e/omega-audit.spec.ts:220 (e2e auditando el canal muerto — pasaba vacío), docs/auditoria/OMEGA_AUDITORIA_HOLONOMICA.md:191 (histórico, se conserva).

## Fix aplicado (PR #616, branch `fix/ws-metrics-dead-room`, commit fb547bb6)

1. Handler `subscribe:metrics` removido de websocket.ts + comentario con la razón (WO-D5, R10).
2. Entrada `subscribe:metrics` eliminada de apis/asyncapi.yaml (el canal deja de existir en el contrato).
3. Harness de websocket-rooms.test.ts actualizado (ya no replica el room muerto; 4/4 pass).
4. e2e omega-audit.spec.ts test 05 repuntado al room VIVO `telemetry` con la forma real CartridgeTelemetry (loose: cartridge_id/level/message) — antes auditar un canal que jamás emitía era un pass vacío.
5. Comentario de auth (websocket.ts:261) actualizado para citar rooms vivos.

## Regla

R10 E2E-COMPUTE GUARD: un canal suscribible sin productor NI consumidor no se
ofrece — se remueve o se le pone productor real. Jamás queda "dormant" como
superficie de API muerta (RULE 00 aplicado a contratos de transporte).
