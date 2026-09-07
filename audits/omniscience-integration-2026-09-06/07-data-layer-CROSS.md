# N7 — Data-Layer CROSS-EXAMINATION (ronda 2)

- **Agente:** verificador N7 "data-layer" (PostgreSQL + Redis)
- **Ventana de verificación cross:** 2026-09-06 23:45 → 23:49 UTC (todo read-only; 0/5 requests públicos; 5 ssh + 4 psql batch + 3 redis batch + 1 curl interno)
- **Veredicto ronda 1:** DEGRADED — **se mantiene DEGRADED** (bomba de disco estructural intacta; el margen transitorio mejoró)
- **Base:** `07-data-layer.md` (ronda 1) + medición fresca post-redeploy de #544

---

## 0. Evento del round-table que cambia el tablero

**El VPS quedó sincronizado a GitHub main EN VIVO durante la auditoría.** A las
`23:45:12Z` mi ssh midió `/opt/arbitragex-v2 HEAD = 9ac06d2d` (== `origin/main`),
con rolling-recreate en curso (api-server "Up 7 seconds", redis/postgres en
recreación a esa hora exacta, todos healthy a las `23:46:01Z`). Timeline completo:

```
22:30Z merge #545 · 22:58Z redeploy flota (d4d3ff63... previo)
23:01Z merge #543 · 23:27Z merge #544 (GitHub main → 9ac06d2d)
23:33Z redeploy flota (evidenciado por 08-monitoring)
23:45Z NUEVA ola de recreate (mi ssh: HEAD ya 9ac06d2d) → 23:46Z flota datos healthy
```

Esto corrige en vivo los DRIFTs de deploy reportados por 01-frontend y 08-monitoring
(ver §2.1) y confirma la cronología de merges encadenados de 08-monitoring (D7).

**Incidente data-plane durante el churn (refuerza R2 de 08-monitoring):** a las
`23:45:12Z` el `docker exec redis` falló con *"container is not running"* y
postgres/redis no aparecían en `docker ps` — la capa de datos estuvo **caída
decenas de segundos** durante la recreación. Cada merge a main = ventana de
indisponibilidad del data-plane. Re-verificado a `23:46:01Z`: 4/4 contenedores de
datos healthy (disciplina R9: un instante ≠ ausencia; la ventana sí existió).

---

## 1. CONFIRMACIONES (mi evidencia respalda a los demás)

### 1.1 A 05-simulator-family — sus 4 cifras PG son exactas (y una mía era estimada)
Remeición exacta post-redeploy (`23:46-23:48Z`):

| Su claim | Mi medición fresca | Veredicto |
|---|---|---|
| simulations 1.000.718 filas, `passed=f` 100% | `passed=false n=1000962`; **grupo `passed=true` ausente del GROUP BY** (0 filas) | CONFIRMADO (0 aprobadas en la historia; +244 filas en ~40 min = flujo vivo) |
| `XLEN arbx:opps:simulated = 0` | `0` (23:46Z) | CONFIRMADO |
| paper_trade_runs 598.878 filas, labels=0, sim_attempts max=0 | exacto: `n=598878 labeled=0 max_attempts=0` | CONFIRMADO — **su COUNT exacto es el correcto; mi "670K" de ronda 1 era `n_live_tup` (estimado inflado: hoy 669.578 vs 598.878 exacto). Corrijo mi ronda 1.** |
| congelada desde 2026-09-01 16:32 | `MAX(created_at) = 2026-09-01 16:32:06.855275+00` | CONFIRMADO con precisión de milisegundos |

### 1.2 A 03-api-ws — cable muerto hot:* y productor WS, verificados desde la capa de datos
```
XLEN arbx:hot:detected = 0 · XLEN arbx:hot:simulated = 0            (23:46Z)
XINFO GROUPS arbx:hot:detected → ws-emitter-g0: 62 consumers, last-delivered-id 0-0, lag 0
  → el grupo NUNCA leyó una sola entrada (stream eternamente vacío): cable muerto CONFIRMADO
Trigger PG: trg_notify_opportunity enabled='O' (+ trg_notify_opportunity_update, trg_opp_updated_at)
  → el productor del evento 'new_opportunity' que ustedes trazaron EXISTE y está habilitado en PG
```

### 1.3 A 08-monitoring — genealogía D1 confirmada y brecha cerrada
Su corrección del hallazgo semilla (#543/#545/#544) coincide con mi línea temporal
(mi ronda 1 midió origin/main=d4d3ff63 a las ~23:20Z — ANTES del merge #544 de
23:27Z; hoy ls-remote = 9ac06d2d y VPS = 9ac06d2d). El drift VPS↔main que
reportaron **ya no existe** a las 23:45Z.

### 1.4 A 02-edge — persistencia Redis sana (cierre de una deuda de memoria)
`aof_enabled=1 · aof_last_write_status=ok · aof_current_size=270.5MB · loading:0`
— la corrupción AOF del 2026-09-04 no dejó secuelas. DBSIZE 3.801→3.214 tras el
restart (claves TTL expiradas — buckets rl:*/cache del edge consistentes con su
rate-limit Redis-backed).

### 1.5 Charter del 04-searcher (que NO entregó reporte): cubierto desde mi superficie
`04-searcher-pipeline.md` quedó **EN_CURSO sin deliverable** (solo el plan). Su
charter R7 queda cubierto por mi medición: `opportunities` LAG **53 s**,
`57.927/24h`, total 12.804.965; probe interno `127.0.0.1:8787/api/opportunities/live`
= `{"count":50,...}` con items reales → **pipeline detección→PG→edge VIVO y
fresco** post-redeploy. El round-table no tiene ese hueco abierto.

---

## 2. DESAFÍOS Y REFINAMIENTOS (con contra-evidencia)

### 2.1 A 01-frontend-web — su DRIFT principal ya está RESUELTO en vivo
- **Su claim:** "Live domain es 1 PR detrás de main... P1: sincronizar VPS a
  GitHub main (pull + rebuild)".
- **Contra-evidencia:** `ssh 23:45:12Z → git -C /opt/arbitragex-v2 rev-parse HEAD
  = 9ac06d2dc70594dd8eac904aea027613a22a1940` == GitHub main; recreate de flota
  en curso a esa hora (sus propias capturas fueron pre-23:45Z).
- **Resolución:** su medición fue correcta PARA SU VENTANA, pero el reporte final
  al operador no debe dejarlo como acción pendiente — la sincronización ya ocurrió
  (operador/auto-deploy). Falta su re-verificación del marcador público
  (`data-slot=go-no-go-signoff-card`) para cerrarlo formalmente (pregunta §4).
  Sus otros 4 findings (CSP report-only, falta de buildId, main local stale
  28d48cdd — que yo también documenté en ronda 1, reflojo `git ls-remote` hoy =
  9ac06d2d) siguen vigentes.

### 2.2 A 03-api-ws — su propuesta XS de higiene es de alcance insuficiente
- **Su claim:** "60 consumers huérfanos en ws-emitter-g0 → XGROUP DELCONSUMER/
  XAUTOCLAIM en shutdown del api-server" (solo ese grupo).
- **Contra-evidencia (XINFO GROUPS 23:46Z):** el patrón es **sistémico en 4
  grupos y 3 servicios**:

| Stream | Grupo | Consumers | Origen |
|---|---|---|---|
| arbx:opps:detected | paper-archiver-g0 | **63** (+2 esta noche) | api-server |
| arbx:opps:detected | selector-g0 | **59** (+2) | api-server/svc |
| arbx:opps:validated | sim-ctl-g0 | **51** (+2) | sim-ctl |
| arbx:hot:detected | ws-emitter-g0 | **62** (+2) | api-server |

  = **235 consumers, ~230 huérfanos**, +1/boot cada uno (61→63 y 57→59 y 49→51 y
  60→62 con los DOS restarts de esta noche — crecimiento medido, no estimado).
  La propuesta correcta es un mecanismo genérico (DELCONSUMER en shutdown +
  GC idle>24h) para TODOS los grupos, no un patch puntual de ws-emitter-g0.
  El diagnóstico de ustedes es correcto; el alcance, no.

### 2.3 A 05-simulator-family — su P0 (flip revm) tiene una dependencia data-layer no evaluada
- **Su claim:** "el flip SIM_BACKEND=revm corta D-1/D-2/D-5 de raíz y desbloquea
  labels" (P0).
- **Contra-evidencia / matiz:** nadie ha ejercitado JAMÁS el consumo del stream de
  salida en producción (`XLEN arbx:opps:simulated=0` histórico; el único writer
  es su consumer.rs con XADD MAXLEN 10.000). Mientras tanto, el stream hermano
  `arbx:opps:detected` YA pierde entradas por trim-antes-de-consumo:
  `lag − length = 10.548 − 10.001 = 547` entradas (23:46Z), **creció de 488 a 547
  (+59) en 21 minutos** — los 2 restarts de esa ventana no lo explican (el stream
  retiene ~6 h de headroom; una pausa de 2 min no puede comer 59 entradas): hay
  déficit estructural de consumo ~14% del flujo. Traducción: el día que el flip
  produzca `passed=t` a volumen, el stream de salida puede repetir el patrón y
  **perder silenciosamente labels recién desbloqueados**. El flip debe llegar con:
  (a) consumer group verificado sobre `simulated`, (b) alerta `lag > length` en
  todos los streams `arbx:opps:*`, (c) post-flip: primer `passed=t` + `XLEN>0` +
  lag<length sostenido. Esto NO difiere el flip: lo blindá.
- Nota: hoy el hueco del detected es **inerte en la práctica** — ver §2.5.

### 2.4 A 08-monitoring — su "78% / 33G libres" y mi "80% / 30G": reconciliados
Serie medida: **80%/30G (23:20Z, ronda 1) → 78%/33G (ustedes) → 76%/35G (23:45Z,
cross)**. Explicación por `docker system df`: Images 35.28→28.18 GB (cleanup de
imágenes viejas tras los builds) — el deploy LIMPIÓ. No es discrepancia, es
deriva temporal; ambos reportamos lo mismo con timestamp distinto. **El hecho
estructural no se movió un milímetro**: `route_discovery_outcomes` 86,490,723
filas / ~30M/día vs cap de purge 20M/día → **la corrida de mañana 04:17Z es la
PRIMERA en la que expira ~30M > cap 20M**; desde ahí, +10M filas/día netas
(~+5 GB/día). Con 35G libres: ENOSPC ≈ 09-13/14. Y `pg_database_size` ya midió
60 GB (era 59 GB hace 28 min — fase de inserción activa). Además su R2 (churn)
se agrava con mi dato: el data-plane estuvo caído decenas de segundos en la
recreación de 23:45Z (§0).

### 2.5 Autocrítica a MI ronda 1 — el "hueco del ledger paper" hoy es inerte (R3 degradado a latente)
Mi ronda 1 planteó R3 (hueco silencioso en ledger paper) como riesgo activo. La
medida fresca lo matiza: `opportunity_observations` está **AL DÍA** — 131.898
filas en 24h, `MAX(observed_at)=23:48:14Z` (segundos antes de mi query) — y
`paper_trade_runs` está congelada **por diseño** (100% del flujo es rejected →
`skip_rejected`; 0 labels que archivar). Con 0 viables en el sistema, el lag-gap
no está costando datos hoy. **Se vuelve crítico únicamente cuando el pipeline
produzca viables** (post-flip revm / post-WO-06). Declaro el gap abierto que
queda: no identifiqué el writer exacto de `opportunity_observations` (¿paper-
archiver vía Redis o searcher directo a PG?) — pregunta directa §4, porque si el
writer es el archiver, la cobertura 131.9K/24h CONTRADIRÍA el déficit de consumo
y el lag sería artefacto de contador (XAUTOCLAIM no incrementa `entries-read`);
si el writer es otro, el hueco es real y latente.

---

## 3. Riesgos actualizados de MI superficie (delta vs ronda 1)

| # | Riesgo | Estado |
|---|---|---|
| R1 | ENOSPC ~09-13/14 (purge cap 20M < 30M/día; primera corrida deficitaria HOY-mañana 04:17Z) | **INTACTO (P0)** — margen transitorio +5GB por cleanup de imágenes |
| R2 | WAL-burst sin pacing (CHECKPOINT multi-statement roto, `|| true`) | INTACTO (P1) — se agrava si se intenta "alcanzar" el backlog |
| R3 | Hueco archiver (lag 547 > length) | **DEGRADADO a latente-condicional** (hoy inerte; crítico post-flip) — ver §2.5 |
| R4 | Bloat: `pool_reserves` dead=1.679.560 (autovacuum stale 09-05), VACUUM manual roto | INTACTO (P2) |
| R5 | Deploy churn = data-plane down (redis/postgres no disponibles en la ventana de recreate) | **NUEVO** (P1, comparte owner con R2 de 08-monitoring) |

---

## 4. Preguntas directas

1. **A 05-simulator-family:** ¿Existe HOY un consumer group creado sobre
   `arbx:opps:simulated` y quién es el lector canónico del stream de salida
   (¿paper executor? ¿drift_tracker?)? Si el flip llena el stream MAXLEN 10.000
   sin lector verificado, se repite el patrón lag>length que ya le costó 547
   entradas al `detected`. Propongo gate post-flip: lag<length sostenido 24 h.
2. **A 03-api-ws:** ¿`paper-archiver-g0` es el writer de `opportunity_observations`
   (131.898 filas/24h, frescas al segundo) o ese ledger lo escribe searcher-rs
   directo a PG? Es la pieza que falta para cerrar si el lag de Redis es pérdida
   real o artefacto de contador (XAUTOCLAIM no incrementa `entries-read`).
3. **A 01-frontend-web:** a las 23:45Z el VPS ya sirve `9ac06d2d` (mi ssh) —
   ¿re-verificaron `data-slot=go-no-go-signoff-card` en el dominio público
   post-redeploy? Si no, su DRIFT #1 está ya resuelto y el reporte al operador
   debe decirlo (yo no gasté requests públicos para no duplicar: 0/5).
4. **A 08-monitoring-fleet:** ¿existe ya una regla `node_filesystem`/disco en
   `alerts.rules.yml` (24 reglas en main)? Mi P1 de alerta 75/85% no debe
   duplicarla — pueden confirmarlo desde su superficie Prometheus (yo no verifiqué
   las reglas: hueco declarado).

---

## 5. Propuestas refinadas (con dependencias aprendidas del round-table)

| # | Qué | Por qué / novedad cross | P | Effort | Gate |
|---|---|---|---|---|---|
| 1 | `ARBX_RETENTION_MAX_ROWS=40000000` (o cron 2×/día) | Cap 20M < 30M/día → HOY 04:17Z es la última corrida que alcanza; desde mañana +10M filas/día. Margen 35G (cleanup de imágenes compró ~1 día) | **P0** | 1 línea | OPERADOR |
| 2 | `docker builder prune -af --keep-storage 5GB` + cron semanal con `-a` | Build cache medido **20.65 GB con ACTIVE=0 (100% reclaimable)** — +20 GB de aire inmediato | **P0** | 1 cmd | OPERADOR |
| 3 | Fix VACUUM/CHECKPOINT multi-statement en `scripts/pg_retention.sh` | `retention.vacuum` falla TODAS las corridas desde 09-04; pool_reserves 1.68M dead | P1 | ~10 líneas | PR con ID (P-∅) + CI |
| 4 | **Gate data-layer para el flip revm** (dependencia de la P0 de 05-sim): consumer verificado en `arbx:opps:simulated` + alerta `lag>length` en `arbx:opps:*` + identificar writer de observations | El stream de salida jamás se ejercitó; su hermano `detected` ya pierde ~14% del flujo por trim-antes-de-consumo | **P1 (bloqueante del flip)** | S | arbx-simulation-mandatory + operador |
| 5 | Alerta disco 75/85% → Alertmanager (fusionar con la pregunta §4-4 a monitoring para no duplicar regla) | 09-04 se llenó sin aviso; hoy 76% transitorio | P1 | 1 regla | PR + promtool CI (#545) |
| 6 | Investigación hueco archiver: conteo ledger-vs-stream + artefacto XAUTOCLAIM sí/no | R3 degradado a latente: hoy inerte (100% rejected), crítico post-flip | P2 | 2-4 h | RULE 00: medir antes de cambiar |
| 7 | DELCONSUMER/GC genérico idle>24h para TODOS los grupos (235 huérfanos: 63+59+51+62) | Generaliza la XS de 03-api-ws (su alcance era 1 de 4 grupos) | P2 | XS-S | PR + verificación RO redis |
| 8 | Ventana RDO 2d→1d si 30M/día es el nuevo normal | Steady state RDO 42→21 GB; 99.82% es telemetría de rechazo; trazabilidad fina vive en rollup 5m | P2 | 1 celda | OPERADOR (trazabilidad) + PR |

---

## 6. Comandos usados (todos read-only; presupuesto público 0/5)

```
ssh arbx: date -u; git -C /opt/arbitragex-v2 rev-parse HEAD; df -h /; docker ps [-a]
ssh arbx: docker exec redis redis-cli XLEN {detected,validated,simulated,hot:detected,hot:simulated}
         XINFO GROUPS {detected,validated,hot:detected}; XINFO STREAM detected; DBSIZE; INFO persistence
ssh arbx: docker exec postgres psql -At -c [columns paper_trade_runs/opportunity_observations;
         triggers opportunities; simulations GROUP BY passed; paper_trade_runs n/labeled/max_attempts/MAX(created_at);
         pg_stat_user_tables live/dead/last_autovacuum; opportunity_observations 24h/MAX;
         opportunities 24h/lag/total; pg_database_size]
ssh arbx: docker system df; tail -3 /var/log/arbx-pg-retention.log; curl -s 127.0.0.1:8787/api/opportunities/live
local:   git rev-parse/branch/status/ls-remote (read-only)
```

Cero escrituras al sistema. Único archivo escrito: este cross + el JSON estructurado.
