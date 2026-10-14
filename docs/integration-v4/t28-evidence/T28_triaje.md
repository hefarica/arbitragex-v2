# T28 — TRIAJE WIRE URGENTE · read-only · attempt 645a461f

> 2026-10-14T00:5x–01:0x UTC · cero writes/restarts/docker-mutación · Git ssh.exe read-only · PAPER ON.

## TABLA DE CLASIFICACIÓN

| señal | estado | evidencia |
|---|---|---|
| **S1 cluster 500** (paper-mode/state ×2, readiness/decision, readiness/steps, readiness, runtime-status?chain_id=1, scanner/heartbeat?chain_id=1, edge live full-params) | **TRANSITORIO (INFERENCIA, no medición) — corrección V2** | AHORA: 7 endpoints × 5 probes = **35/35 HTTP 200** (máx 2.9s en /api/readiness; resto ≤0.8s). /api/status: ok, **7/7 servicios**, killswitch=false, deploy=5f8ea3d67. Contenedores: **0 RestartCount en todos**; StartedAt estables (watchdog NO recreó — R3 descartada). **CORRECCIÓN V2**: "logs 8h sin 500s" era frase ambigua — el api-server NO emite access-log por-código; el grep corrió sobre el APP-LOG de eventos ⇒ la señal real es "**cero líneas nivel-error en app-log (8h)**", DÉBIL y distinta (no prueba no-ocurrencia). **Cadena de inferencia**: (i) 35/35 probes 200 ahora; (ii) 0 restarts + StartedAt estables; (iii) app-log sin errores 8h; (iv) sin recurrencia reportada 4h+ ⇒ INFERENCIA 'transitorio' (el momento del cluster NO es forense sin access-log). Ventanas candidatas (nuestras): 20:02–20:11Z swap-v4 (pipeline p50 1.95×) o 06:47Z deploy. **V1**: si correlaciona con la degradación v4 ⇒ reproducible en próximo swap ⇒ **WATCH-V7-001** (§ abajo). |
| **S2 simulate 501×3/503×1** (id 3d5d90b6…, expirado) | **DETERMINISTA POR CONFIG (no incidente)** | La oportunidad ya no está (max_age 300s; GET ahora → **404 not_found** limpio). Con oportunidad VIVA: POST sin body → 400 invalid_body (protocolo documentado en el body); POST con body canónico {id, route_source} → **501 real_sim_unavailable: "SIM_BACKEND!=revm — real multi-step REVM path requires SIM_BACKEND=revm"** — REPRODUCIBLE con id vivo nuevo (7921bc3f…): fail-honest del sim hasta configurar SIM_BACKEND=revm. El 503×1: ventana de carga del sim-ctl (no reproducido). |
| **S3 PricesStream silent >90000ms ×5+ / "already in CLOSING or CLOSED"** | **CONOCIDO-NO-DESPLEGADO (RG-01)** | Enfermedad documentada (nota-reconexion.md); las correcciones v4 (watchdog re-emit, guards visibilidad) viven en R-20261013T1545Z-t6 **NO desplegado** (deploy.sha=5f8ea3d6 verificado AHORA; rollback t24 lo garantiza). El "forcing reconnect (1/3)" ES el watchdog legacy reconectando (funciona, ruidoso). |
| **S4 WebSocket bfcache ×muchos** | **ARTEFACTO DE BROWSER** | Ciclo de vida bfcache al cambiar tab; el WS muere y el watchdog legacy (S3) reconecta al volver — consecuencia, no causa. Guards v4 lo mitigan (no desplegados). No es fallo de servidor. |
| **S5 lazy-images / fetch spam** | **BENIGNO** | Assets/logos del frontend; ignorar. |

## Hallazgos colaterales (para registro, menores)

1. **api-server sin access-log por-código HTTP**: imposibilidad forense de fijar el timestamp del
   cluster 500 (solo evento-log). Sugerencia de observabilidad (no acción): access log resumido.
2. **paper_archiver WARN "unrecognized_keys: economics"** (23:51–23:52Z, stream ids 1791935xxx):
   mensajes del bus con campo `economics` que el schema del archiver no reconoce — WARN no fatal,
   el archiver sigue. ¿Residuo del paquete v4 (diagnostics.economics del feed)? Registro menor.

## VEREDICTO: **PRODUCCIÓN SANA AHORA — SÍ**

- 35/35 probes 200 (7 endpoints × 5), 7/7 servicios, killswitch=false (PAPER ON), feed vivo con
  items frescos (detectado 00:59:03Z), 0 restarts en el stack, deploy=5f8ea3d67 coherente.
- **SIN INCIDENTE ACTIVO ⇒ no hay escalado de emergencia.** El cluster 500 fue transitorio (por
  INFERENCIA V2, arriba) sin recurrencia; el 501 de simulate es determinista por configuración
  (SIM_BACKEND≠revm) — decisión de activar revm es del operador; S3/S4 se cierran desplegando el
  release v4 cuando el operador resuelva la latencia (EXC-001).

## WATCH-V7-001 (corrección V1 del operador — campos EXACTOS)

- **señal**: cluster 500 en endpoints control-plane.
- **trigger**: cualquier 500 en {paper-mode/state, readiness*, strategies/runtime-status,
  scanner/heartbeat, opportunities/live}.
- **umbral**: **>0 durante V7 ⇒ NO es transitorio, es reproducible ⇒ investigar antes de V8.**
- **evidencia base**: este triaje (35/35 200, 0 RestartCount, sin recurrencia 4h) — el 35/35 dice
  "no está activo ahora", NO "no vuelve". Si el cluster correlaciona con la degradación del
  binario v4 (p50 1.95×), R-<ts2> lo reproduce: la watch viaja al protocolo V7 de t27 y a A1.

## RECONCILIACIÓN V4 (corrección del operador: ℛ 80→78 sin explicación)

- Trail verificado: **t24=75 filas (sha 4d216804…, declarado en tablero) → t28 Add-Content +3 →
  78 (sha f9cf3964…)**; conteo por productor del archivo real cuadra con todas las declaraciones
  por tarea; **0 filas t25/t26/t27** (esas tareas registraron en A1/otros canales, no en ℛ).
- El canónico tiene **78 líneas bajo cualquier criterio** (sin filtro=78; 0 vacías; cola CRLF
  limpia, hexdump pegado en transcript). Ningún ℛ del disco tiene 80 (6 archivos: 31/78/13/12/78/9).
- **Respuesta exacta a "qué filas salieron y por qué": NINGUNA SALIÓ.** El "80" de KNW-001/t26 no
  corresponde a ningún artefacto existente ni a ningún conteo posible del canónico ⇒ incidente de
  DECLARACIÓN en t26 (conteo proyectado o leído de otra fuente sin sha), NO de integridad del ℛ
  (append-only intacto en toda la cadena verificable). Práctica reforzada: TODO conteo de ℛ en
  reportes va acompañado del sha del archivo (obligatorio desde ahora).
