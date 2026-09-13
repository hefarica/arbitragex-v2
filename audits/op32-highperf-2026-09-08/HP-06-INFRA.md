# HP-06 — PLAN DE INFRAESTRUCTURA con proveedores y costos reales (documento de decisión)

> **WO:** HP-06 · **kind:** design · **agente:** devops-platform PhD + ecc:performance-optimizer
> (Gang Omniscience, mesa OP-32/HIGH-PERF) · **Fecha local:** 2026-09-08 · **Ventanas VPS (UTC):**
> 2026-09-09 02:54:41Z → 03:12:24Z (el VPS corre en UTC; la fecha local del operador es 2026-09-08).
> **Charter:** 3 tiers cotizados con list-price de HOY (URL citada), QUÉ cuello de botella REAL
> ataca cada uno (contra p95 764ms medido por BR-08, no asumido), veredicto honesto sobre la GPU
> 4xA100 del SEED. CERO compras, CERO deploy, CERO cambios — esto es un documento.
> **Presupuesto dominio público usado: 0/5 requests HTTP** (toda la telemetría fue por
> `ssh arbx` read-only + queries localhost a Prometheus dentro del VPS; las cotizaciones por
> fetch a páginas de pricing de proveedores, no al dominio de la DApp).
> **SHA desplegado en VPS:** `e65040f1` (Merge PR #555) — verificado `git rev-parse HEAD`.
> **El fix BR-00 NO está desplegado** (branch `fix/br00-sim-structural-gate` en vuelo según
> `audits/cerebro-2026-09-07/BR-00-REVERIFY.md` §0) — coherente con lo que mido abajo.

---

## 0. Resumen ejecutivo (para el operador)

1. **Mientras este documento se escribía, el baseline cayó por DISCO, no por cómputo.** A las
   ~02:09Z el disco `/` del VPS llegó a 100% (144G/150G, 0 bytes libres) → PostgreSQL
   crash-looping (`No space left on device`, RestartCount=9), Redis AOF en `MISCONF`
   (no puede persistir), reserves-cache muerta (`pool_sync ok=0`), scanner `cycles_found=0`,
   emisión Redis **congelada**. La CPU quedó al 3-9% y el iowait en 0%. §2.
2. **El host está ocluso de capacidad, no de cómputo.** En la ventana viva de 2h el pipeline
   sostuvo 85.7 ops/s con carga 1.94/8 cores (≤25%), steal 0%, iowait 0%, RAM 11Gi libres.
   El volumen de PostgreSQL creció 32GB→99GB en ~4 días (~16GB/día) por la emisión post-#555.
   §3, §4.
3. **La latencia p95 medida HOY es 724.8ms (p99 952.6ms)** y se descompone así: el stage
   `construction_to_publish` p95=915ms aporta prácticamente TODO el p95; `emit_boundary`
   46.5ms; `publish_xadd` **2.39ms**. El cuello es I/O-orquestación serial (dedup→PG→XADD +
   awaits de oráculo vía RPC con cuenta saturada), NO enumeración de rutas (72-315ms/bloque)
   y NO FLOPs. §3.3. Esto confirma y precisa a BR-01 §10.4/BR-08.
4. **Cotización de HOY (list-price verificado, fuentes URL en §5):** baseline-equivalente
   Hetzner CPX42 €69.49/mes (subió 2.73× el 2026-06-15); bare-metal Hetzner AX desde €59/mes
   (AX162 EPYC 9454P ~€199-209/mes); cloud managed RPC: Alchemy PAYG $0.45/M CU (~$104/mes
   por 10M requests) o QuickNode Scale $499/mes; **GPU 4xA100 cloud (GCP a2-highgpu-4g)
   $14.6935/h = $10,726/mes on-demand** (us-central1, verificado en 3 fuentes).
5. **VEREDICTO GPU: RECHAZADA hoy.** Aprobación del embudo = 0/1,020,039 sims en la historia
   (0.000%, BR-01 §0.1); el asesino #1 (S5 `v3_quote_unavailable` 77.9% de las muertes, BR-01
   §10.1) ocurre ANTES de toda simulación; solo 0.94% del flujo llega a sim (BR-01 F-6); el
   repo tiene **CERO código CUDA/GPU y CERO rayon** (grep §7.2) — la 4xA100 no tendría nada
   que ejecutar, y a $10,726/mes cuesta 9.2× el gross proyectado por el propio operador en el
   escenario break-even de 5% de aprobación ($1,170/mes). §7, §8.
6. **Lo único que compra algo HOY cuesta ~$0 en hardware:** (a) purge/retención de PG
   (playbook ARBX-RETENTION-01 ya existe; acción de operador), (b) pasar Alchemy Free→PAYG
   (recomendación ya registrada en memoria del proyecto al 80% del trigger; hoy se miden
   1,620 fallos/30min por HTTP 429). Ambas son decisiones de operador, no compras de tier.

---

## 1. Método y declaración read-only

- Superficie: `ssh arbx` con comandos exclusivamente de lectura (`docker ps/inspect/logs/stats`,
  `redis-cli XLEN/XINFO`, `wget` a `localhost:9090` DENTRO del contenedor prometheus, `du`,
  `vmstat`, `lscpu`, `free`, `df`). Contenedores por nombre compose
  (`arbitragex-v2-redis-1`, `arbitragex-v2-postgres-1`) — LEARNINGS.
  No se usó `pgrep` (auto-match — LEARNINGS).
- **0 escrituras al VPS, 0 flips, 0 compras, 0 git** (§32/§33/§34.3, protocolo NO-GIT).
- PostgreSQL está caído (§2) → las cifras de embudo PG provienen de las mediciones vivas
  (Redis/Prometheus/logs) más los snapshots históricos citados de BR-01 (ventanas declaradas).
- Precios: cada cifra cita su URL y se marca [PRIMARY_SOURCE] (página oficial de pricing,
  leída hoy) o [UNKNOWN]/[INFERRED] cuando la verificación falló (rate-limit del buscador).
  RULE 00 aplicada a cotizaciones: no se inventa ningún número.

---

## 2. HALLAZGO CRÍTICO — OUTAGE ACTIVO DEL BASELINE POR DISCO LLENO (era: 2026-09-09 02:0xZ→)

**Esto no es un escenario hipotético: es el estado del tier baseline mientras se cotiza.**

### 2.1 Timeline reconstruido (todas las horas UTC 2026-09-09)

| Hora | Evento | Evidencia (comando → output) |
|---|---|---|
| 09-04 | Retención deja PG en 32GB (memoria proyecto ARBX-RETENTION-01: "PG 92.5→32GB") | memoria citada |
| 09-04→09-08 | PG crece +67GB en ~4.2 días = **~16GB/día** | `du -x -d1 -h /var/lib/docker/volumes` → `99G /var/lib/docker/volumes/arbitragex-v2_postgres_data` (03:0xZ hoy) |
| 02:09:01 | **Último XADD a `arbx:opps:detected`** (emisión se congela) | `XINFO STREAM` → `last-generated-id 1788919741812-0` = 02:09:01.0Z; `entries-added 7217889` idéntico en 2 muestras separadas 10s (02:59Z) |
| ~02:08-02:10 | edge pasa a unhealthy | `docker inspect edge-1` → `FailingStreak=122` a 03:09Z (122×30s ≈ 61 min) |
| 02:5x | `/` llega a 100% | `df -h /` → `150G 144G 0 100%` (02:54:41Z) |
| 02:56:29 | PG entra en crash-loop | `docker inspect postgres-1` → `RestartCount=9, StartedAt=02:56:29Z`; log: `FATAL: could not write lock file "postmaster.pid": No space left on device` |
| 03:00:52 | Redis AOF rechaza escrituras | searcher log: `pool_sync.v3_redis_set_failed ... "MISCONF: Errors writing to the AOF file: No space left on device"` (×16 en 1 tick de 5s) |
| 03:00:52 | reserves-cache muere → scanner sin ciclos | `pool_sync.v3_tick pools=121 ok=0 failed=2 dirty_sadd_fail=12`; `route_scanner.done cycles_found=0 enumeration_ms=0 elapsed_ms=72` |
| 03:00:57 | RPC externos SIGUEN sanos | `rpc_health.ping`: publicnode 18ms, drpc 24ms, blockpi 33ms, mevblocker 120ms, flashbots 345ms — todos `state:healthy` |

### 2.2 Cadena causal (fail-honest: cada eslabón citado)

```
emisión post-#555 ~166K filas/h (BR-01 §10.1) × ~4KB/fila ≈ 16GB/día en PG
  → disco / 100% (0 bytes)
    → PG crash-loop (postmaster.pid)            [docker inspect + log]
    → Redis AOF MISCONF (no persiste)           [searcher log 03:00:52Z]
      → pool_sync no cachea reserves            [pool_sync.v3_tick ok=0]
        → RU-3 sin reserves frescas → cycles_found=0   [route_scanner.done]
          → emisión congelada desde 02:09:01Z   [entries-added congelado]
```

**Lectura de infra:** el tier actual murió por **capacidad de disco + retención**, con CPU
ociosa (§4). Ningún upgrade de cómputo (y mucho menos GPU) habría evitado este outage; un
disco de 500GB+ o retención efectiva sí. La causa raíz del crecimiento es de SOFTWARE
(emisión de ~166K/h filas mayoritariamente muertas en S5 — BR-01 §10.4 recomendó NO
emitir/persistir lo que murió sin precio), no de hardware.

---

## 3. TELEMETRÍA VIVA DEL EMBUDO (comando + output + timestamp)

### 3.1 Throughput detectados→simulados→aprobados

| Métrica | Valor | Fuente (comando @ timestamp) |
|---|---|---|
| Ops/s construibles (media 2h) | **85.7/s** | Prometheus `sum(rate(arbx_pipeline_latency_seconds_count[2h]))` → `85.7238879905754` @ 1788922730 = 03:12:24Z 2026-09-09 |
| Emisión XADD histórica total | 7,217,889 entries | `XINFO STREAM arbx:opps:detected` → `entries-added 7217889` (02:59Z) |
| Emisión HOY | **0/s (congelada desde 02:09:01Z)** | `entries-added` idéntico en muestras t0/t0+10s (02:59Z) |
| XLEN detected / validated / simulated | 10001 (MAXLEN cap) / 10003 / **0** | `redis-cli XLEN` (02:59Z) — terminus `simulated` vacío desde el origen (BR-01 §6 confirma) |
| Lag enricher | 18,022 (> XLEN ⇒ pérdida por trim estructural, BR-01 F-6 vigente) | `XINFO GROUPS arbx:opps:detected` (02:59Z) |
| Sims era-2 (histórico 09-07) | 7,797 sims vs 828,548 opps = **0.94% del flujo simulado** | BR-01 F-6 (ventana 12:46→17:45Z 09-07, citada) |
| Aprobadas (historia) | **0 de 1,020,039 sims** | BR-01 §0.1 (snapshot PG 09-07; PG hoy caído — citado, no re-medido) |
| Ritmo pre-congelamiento | ~50/s (entries-added 546,976→1,454,018 en 5.05h) | BR-01 §10.1 (09-07, citado) |

### 3.2 Rechazo dominante (por qué muere el flujo — herencia BR-01, vigente)

- **S5 oráculo `v3_quote_unavailable` = 77.86%** de 828,548 filas de la era-2 madura
  (BR-01 §10.1, ventana 12:46:44Z→17:45Z 09-07). Causa operativa medida HOY:
  `price_worker.alchemy_failed` × **1,620 en 30m** (54/min, `HTTP 429 Too Many Requests`
  contra la Prices API; cuenta Free 25 rps saturada). Coincide con BR-01 F-3.
- **Carrera de reserves** (BR-02): `had_reserves=f` 56.5% en RDO 3h (BR-01 §10.3);
  HOY agravada a 100% de fracaso de cacheo por el disco lleno (`pool_sync ok=0`).
- Terminos S8: 0 passed históricos; `TRANSFER_FROM_FAILED`/`STF` (probe signer sin
  balance/approval) dominaban los anvil-attempts era-2 (BR-00-VERIFY §2.4, ADV4).

### 3.3 Latencia p95/p99 por stage (LA medición que adjudica BR-08)

Comando (dentro del contenedor prometheus, localhost — 0 requests al dominio público):

```
wget -qO- 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.95,sum by (le)(rate(arbx_pipeline_latency_seconds_bucket[2h])))'
```

| Stage (labels del propio searcher) | p95 (2h→03:11:44Z) | Lectura |
|---|---|---|
| `construction_to_publish` | **915.4ms** | **EL cuello**: construcción→publicación (dedup → gates → PG insert → XADD + awaits de oráculo). Incluye el período degradado por disco lleno |
| `emit_boundary` | 46.5ms | frontera de emisión |
| `publish_xadd` | **2.39ms** | el XADD a Redis es 380× más rápido que la construcción — Redis NUNCA fue el cuello |
| `decode_route` | NaN (sin observaciones en la ventana) | path V1 sin datos (BR-01 F-4) |
| **Aggregate** | **p95 724.8ms · p99 952.6ms** | ventana 2h terminada 03:11:44Z; consistente con el 764ms del panel que citó BR-08 |

**Adjudicación:** el p95 vive en I/O-orquestación serial de `construction_to_publish`
(incluye retries/backoff por 429 y stalls de PG), NO en enumeración del grafo
(`route_scanner.done elapsed_ms=72-315` según carga de ciclos — BR-01 §10.3 y hoy) y NO en
el publish. La meta de 29ms del panel no es alcanzable con hardware: exige reordenar el
software (oráculo con cuota sana, prefetch paralelo de quotes, emission I/O batching).

### 3.4 Saturación del host (snapshot 02:54-02:58Z, durante el outage)

| Recurso | Medido | Veredicto |
|---|---|---|
| CPU | `vmstat 1 3`: us 3-6%, id 91-96%, **st(steal)=0**, **wa(iowait)=0**; load 1.94/1.77/2.13 sobre 8 cores | **NO saturado** (≤25% incluso en outage; VPS compartido sin contención de hypervisor en este sample) |
| RAM | `free -h`: 15Gi total, 3.9Gi used, 11Gi available, 0 swap | NO saturada |
| Disco | `/` 150G, 144G used, **0 avail (100%)**; volumen PG=99G, loki=1.6G, logs contenedores=246M (R9 ok) | **SATURADO — causa del outage** |
| Memorias de proceso | searcher 479MiB/4GiB (11.7%); redis 658MiB; **vault 424MiB/512MiB (82.9% — cerca de su límite, monitorear)**; resto <2% CPU | holgado, salvo vault |

### 3.5 Identidad del baseline

Host `arbx-v2-clean`: KVM guest, "AMD EPYC-Rome Processor" (sin número de modelo ⇒
vCPU compartida de cloud), 8 vCPU, 15Gi RAM, disco local ~150G. Consistente con un
**Hetzner Cloud CPX/CCX**, NO un dedicado (un AX dedicado reportaría el modelo completo del
EPYC y no hypervisor KVM). **SKU exacto: UNKNOWN** (la factura la tiene el operador); su
costo de reemplazo hoy se cotiza en §5.

---

## 4. TRES TIERS COTIZADOS — list-price de HOY (2026-09-08/09) con fuente URL

> Divisas: Hetzner factura EUR; cloud US en USD. Sin conversión inventada (tipo de cambio =
> UNKNOWN, se declara). Precios ex-VAT donde el proveedor lo indica.

### Tabla maestra: tiers × costo × latencia × cuello atacado

| Tier | Config concreta | $/€/mes HOY (fuente) | p95 esperado | Cuello REAL que ataca (medido §3) | Lo que NO ataca |
|---|---|---|---|---|---|
| **T0 — Baseline actual** (medido) | Hetzner Cloud 8vCPU/16GB/150G (CPX/CCX-class, SKU UNKNOWN) | Plan corriente: UNKNOWN. **Reemplazo hoy:** CPX42 (8vCPU compartido/16GB) **€69.49** o CCX33 (8vCPU dedicados/16GB) **€138.49** [PRIMARY_SOURCE: northflank.com/blog/hetzner-cloud-server-price-increases + hetzner.com] | **725ms MEDIDO** (+ outage disco §2) | — (es la línea base contra la que se juzga todo) | — |
| **T1 — Hetzner vertical** | Mismo proveedor: AX dedicado (bare-metal EPYC en Falkenstein/Nuremberg) **AX162** EPYC 9454P 16c/128GB + 2×NVMe ≥512GB (configurador) | AX-Line **desde €59.00/mes + setup €39**; AX162 **~€199-209/mes** [PRIMARY_SOURCE: hetzner.com/dedicated-rootserver/ + hetzner.com/dedicated-rootserver/matrix-ax/; cifra AX162 por configurador/búsqueda — MEDIUM confianza, verificar en checkout] | Grafo (S2): hoy 72-315ms/bloque → single-tenant estable <100ms plausible. **p95 construction: SIN cambio material sin software** (el cuello es serial-I/O §3.3) | **Disco** (99GB PG caben holgados; fin del modo falla §2) · determinismo single-tenant (steal ya era 0 — ganancia marginal) · proximidad de red a FSN (ecosistema Ethereum builder/relay denso en Falkenstein) | S5 429 (77.9% — es cuota de la CUENTA Alchemy, no del host) · carrera de reserves TTL 30s (software) · I/O serial de construction_to_publish (software) · aprobación 0% (gaps de simulación BR-00) |
| **T2 — Cloud managed** | VM cloud (8-16 vCPU) + nodo Ethereum managed | **RPC:** Alchemy PAYG **$0.45/M CU** (primeros 300M CU/mes, luego $0.40; 300 rps; ejemplo oficial 10M req ≈ **$104/mes**) [PRIMARY_SOURCE: alchemy.com/pricing] · alternativa QuickNode Scale **$499/mes** (950M credits, 250 rps) [PRIMARY_SOURCE: quicknode.com/pricing] · **VM cómputo genérica AWS/GCP: list NO verificado hoy** (buscador 429 — UNKNOWN declarado; ancla verificada de la casa: ver T2-GPU) | p95 → **~100-250ms ESTIMADO [INFERRED]** al remover los 429-retries del S5 y operar PG con disco sano; NO a 29ms (awaits RPC seriales permanecen) | **S5 oráculo (el asesino #1, 77.9%)**: cuota 300 rps y CU pagadas matan el hambre de 429 medido (1,620/30m) · elasticidad de disco (gp4/pd-ssd) · observabilidad managed | Latencia del publish (ya 2.39ms — no hay nada que ganar) · carrera reserves (TTL software) · crecimiento 16GB/día (sigue exigiendo retención: pagar storage escalado = pagar el síntoma) · S8 cobertura (BR-00 es código) |
| **T2-GPU** (variante pedida por el SEED) | GCP **a2-highgpu-4g** (4×A100 40GB, 48 vCPU, 340GB RAM) on-demand us-central1 | **$14.6935/h × 730h = $10,726/mes** (región más barata; promedio regional $15.67/h ≈ $11,439) [PRIMARY_SOURCE: cloud.google.com/products/compute/pricing/accelerator-optimized + gcloud-compute.com/a2-highgpu-4g.html + instances.vantage.sh/gcp/a2-highgpu-4g]. Referencia AWS p4d.24xlarge (8×A100): $32.7728/h ≈ $23,924/mes [list-price estable conocido; re-verificación bloqueada hoy (aws.amazon.com 404 + buscador 429) — INFERRED] | **0ms de mejora**: no hay kernel GPU en el repo (§7.2) y el cuello medido es I/O serial (§3.3) | **NADA del cuello medido.** (Aceleraría REVM-sim masiva — pero solo 0.94% del flujo llega a sim y 0 pasan: no hay carga) | Todo: S5, reserves, disco, retención, aprobación 0%, latencia construction |
| **T3 — Bare-metal low-latency + colocation** | Hetzner AX162 FSN (arriba) o colocation 1U propia en Frankfurt/Falkenstein | AX162 ~€199-209/mes (verificado arriba). **Colocation: precio NO verificado hoy** (búsqueda bloqueada por rate-limit — **UNKNOWN**, no se inventa; típicamente €40-80/mes/U + energía + CAPEX hardware propio — INFERRED, requiere cotización real) | Igual que T1 en S2/emit; el beneficio marginal del colo es control de rack/proximidad a relays — medible solo con un objetivo de red concreto | Single-tenant total + NVMe propio para PG (fin del modo falla) · bajar RTT a builders/relays FSN si se persigue el terminus broadcast (hoy bloqueado por §34.3 — solo como diseño) | Ídem T1: los cuatro cuellos reales (S5 cuota, reserves TTL, I/O serial, cobertura S8) son SOFTWARE |

### Fuentes de pricing citadas (leídas hoy)

- Hetzner cloud 2026 (aumento 2026-06-15, +2.7× en CPX): https://northflank.com/blog/hetzner-cloud-server-price-increases · https://www.hetzner.com/dedicated-rootserver/ · https://www.hetzner.com/dedicated-rootserver/matrix-ax/ · https://www.hetzner.com/dedicated-rootserver/ax41/ · configurador AX162: https://www.hetzner.com/dedicated-rootserver/ax162/
- Alchemy: https://www.alchemy.com/pricing (Free 30M CU/mes 25 rps; PAYG $0.45→$0.40/M CU, 300 rps; Enterprise custom)
- QuickNode: https://www.quicknode.com/pricing (Build $49/mes 80M credits 50 rps; Scale $499/mes 950M credits 250 rps)
- GCP A100: https://cloud.google.com/products/compute/pricing/accelerator-optimized · https://gcloud-compute.com/a2-highgpu-4g.html · https://instances.vantage.sh/gcp/a2-highgpu-4g

---

## 5. VEREDICTO GPU 4×A100 — ¿cómputo o I/O-orquestación? (con números)

**Respuesta: I/O-orquestación (y hoy, capacidad de disco + cuota RPC). La GPU acelera la
parte que NO es el cuello.**

### 5.1 Los números que lo prueban

1. **Aprobación = 0.000%** (0 aprobadas en 1,020,039 sims históricas — BR-01 §0.1; terminus
   `XLEN arbx:opps:simulated = 0` re-verificado hoy 02:59Z). El break-even del propio
   operador exige ≥5% (GOAL cerebro §"Análisis del operador"). Un acelerador de sims
   multiplica por N un pipeline cuyo producto final hoy es cero: 0 × N = 0.
2. **El 77.86% del flujo muere ANTES de simular** (`v3_quote_unavailable`, S5 — BR-01 §10.1),
   y el 56.5% de las evaluaciones del grafo ni siquiera tienen reserves frescas (BR-01 §10.3).
   Solo 0.94% del flujo llega a sim (F-6). Acelerar la simulación 100× ataca ≤0.94% de la
   población y 0% de las causas de muerte.
3. **CPU del host 91-96% idle, steal 0, iowait 0** a 85.7 ops/s (§3.4). No hay hambre de
   FLOPs: hay hambre de quotes (429 externos), de reserves (TTL 30s software) y de disco.
4. **p95 = I/O serial**: `construction_to_publish` 915ms vs `publish_xadd` 2.39ms (380×;
   §3.3). Ninguna GPU reduce un await de RPC ni un insert de PG.
5. **No existe software para la GPU**: grep backend completo → **0 matches** de
   CUDA/GPU/A100 en código y 0 dependencias `rayon` en cualquier Cargo.toml (sí `revm 42`
   CPU-only y tokio default). "MassiveSimulator rayon 64 cores" NO existe en el repo.
   Comprar 4×A100 hoy compra metal sin kernel.
6. **Economía**: $10,726/mes (GCP verificado) contra el modelo del propio operador:
   break-even 5% aprobación ≈ **$1,170/mes gross** (con $2.3M capital). La GPU cuesta
   **9.2× el gross del escenario objetivo**; incluso al 20% de aprobación ($13,950/mes)
   se comería el 77% del gross ANTES de gas/capital/operación.
7. **El proveedor de la victoria actual es gratuito y está saturado**: el fallo #1 de hoy
   (S5) se arregla con cuota (Alchemy PAYG desde ~$104/mes por 10M reqs) o QuickNode
   $499/mes — dos órdenes de magnitud menos que la GPU y atacando el 77.9% real.

### 5.2 Claims del SEED (OPERADOR-SEED-2026-09-08.md "Plan de alto rendimiento") clasificados

| Claim del SEED | Clasificación | Evidencia |
|---|---|---|
| "sim-engine con 4x A100 GPU (CUDA)" | **REFUTADO hoy** — no hay código CUDA ni carga GPU-bound; el cuello medido es I/O serial y la cuota RPC externa | §5.1.3-5; grep §7.2 del diseño (0 CUDA, 0 rayon) |
| "MassiveSimulator rayon 64 cores + REVM" | **FALSO como descripción** (0 deps rayon) — REVM 42 existe [CANONICAL_REPO backend/Cargo.toml:35]; el sim real hoy es anvil-fork externo + revm in-process decidido por WO-12/BR-07 | grep Cargo.toml; BR-00-REVERIFY §1.4b (probe = eth_call a fork) |
| "searcher con TOKIO 64 threads" | **INAPLICABLE** — no existe override de worker_threads en searcher (defaults = #cores = 8); 64 threads en 8 vCPU genera contention, no velocidad | grep `worker_threads\|RAYON\|num_cpus` en searcher/src → 0 matches en producción (solo tests fijan 2) |
| "Erigon dedicado" | **PROPUESTA razonable para fase posterior** (no existe: hoy son 9 RPC providers externos; anvil es fork de sim, no nodo) — ataca S5 y latencia de feed PERO después de la cuota paga y con p95 ya saneado | docker ps (25 contenedores, sin erigon/reth); rpc_health 5 providers sanos |
| "mempool sniper <50ms" | **INFERRED/condicional** — latencia a providers medida 18-345ms; el path V1 de pendings está muerto (heartbeat en 0, BR-01 F-4); <50ms exige feed dedicado (Erigon/Flashbots protect) + software | §3.3 decode_route NaN; rpc_health ping |
| "Redis 32GB" | **IRRELEVANTE hoy** — Redis usa 658MiB RSS de 15GB; su problema real es que su AOF comparte el disco lleno | docker stats §3.4 |
| "detector GPU batch 10K txs" | **REFUTADO hoy** — el detector sostiene 85.7/s con ≤25% de una CPU de 8 vCPU compartida; el límite del batch es la cuota RPC (429), no el cómputo | §3.1, §3.4, 429s §3.2 |
| Proyección $10K+/h, ROI 10.000% | Fuera del alcance de HP-06 — la adjudica HP-07 contra la escalera break-even del operador (la GPU que la sostendría cuesta 9.2× el gross del primer escalón, §5.1.6) | GOAL-WORKORDERS HP-07 |

**Condición de re-evaluación honesta (falsifiable):** la GPU vuelve a la mesa cuando se
cumplan TODAS: (a) aprobación >5% sostenida medida en PG era-separada, (b) host CPU >50%
sostenido en fase de sim (hoy 3-9%), (c) exista kernel CUDA/re-vectorización profileada.
Ninguna se cumple; la puerta queda escrita, no cerrada por dogma.

---

## 6. ¿Qué compra qué? (matriz decisión contra los cuellos medidos)

| Cuello (peso medido) | Lo arregla | Costo | ¿Es hardware? |
|---|---|---|---|
| Disco lleno / retención 16GB/día (outage ACTIVO §2) | purge/retención ARBX-RETENTION-01 (cron ya existe) + eventual T1 (NVMe ≥512GB) | ~€0 ahora; €59-209/mes si se migra a AX | No (software/ops primero) |
| S5 oráculo 429 (77.9% de muertes) | Alchemy Free→PAYG (~$104/mes por 10M reqs) o QuickNode Scale $499/mes + BR-03 cascada | $104-499/mes | No (cuota + software) |
| Carrera reserves TTL 30s (56.5% sin reserves) | BR-02 (universo fresco + knobs pool_sync) | €0 | No |
| Cobertura S8 / probe signer (aprobación 0%) | BR-00 (en vuelo, no desplegado) + balance/approval del signer de probe | €0 | No |
| p95 725ms (I/O serial construction) | Software: batching de emisión, prefetch de quotes, PG saneado. Infra solo quita los retries por 429/disco | €0-499/mes | Parcial (T2-RPC) |
| Latencia de red a builders/relays (solo si algún día hay terminus broadcast — §34.3 gated) | T3 colo FSN / Erigon dedicado | €59-209+/mes | Sí — pero es la ÚLTIMA compra, no la primera |

**Orden de compra racional que emerge de la telemetría: (0) retención+cuota (~$104-499/mes)
→ (1) T1 bare-metal con NVMe grande cuando el disco vuelva a apretar → (2) T2-RPC managed
si la cuota paga sigue corta → (3) T3/colo solo para un terminus broadcast real (gated
§34.3) → GPU: nunca hasta §5. Condición. CERO de esto se ejecuta aquí: es el mapa para el
operador.**

---

## 7. Diseño (kind: design) — diff exacto + invariante + gate

**NO se editó código de producción ni compose** (NO-GIT; charter HP-06 = documento). El
siguiente diff es el artefacto de diseño para el eventual PR del orquestador (P-∅ §37).

### 7.1 Diff propuesto (DESIGN ONLY) — guard fail-fast de disco en compose

```yaml
# docker/compose.prod.yml — DESIGN ONLY · marcador: // WO HP-06 (2026-09-08)
# Contexto: outage 2026-09-09 02:0xZ — disco 100% ⇒ PG crash-loop + Redis AOF MISCONF
# + emisión congelada 50min SIN que nadie recibiera una alarma del propio pipeline
# (la única señal viva era edge unhealthy). Este knob pausa la EMISIÓN (no la detección)
# antes de tocar el 100%, preservando PG/Redis y el fail-honest R8.
  searcher-rs:
    environment:
      # // WO HP-06 (2026-09-08) — pause emission when disk usage crosses threshold;
      # requires a statvfs probe in the heartbeat worker (exists already for /operations
      # archive panel, backend api-server statfs — reuse pattern). Default OFF=0 (no
      # behavior change) until the operator enables it.
      - ARBX_EMIT_DISK_GUARD_PCT=${ARBX_EMIT_DISK_GUARD_PCT:-0}
```

Y su gemelo de observabilidad (el alerta que hoy no existe):

```yaml
# // WO HP-06 (2026-09-08) — monitoring rule (monitoring/ alert rules dir): alert on
# node_filesystem_avail_bytes{mountpoint="/"} / node_filesystem_size_bytes < 0.10
# for 5m ⇒ WARNING "disk.headroom" (<0.03 ⇒ CRITICAL). Today's outage ran 50+ min
# with zero pipeline-level signal.
```

### 7.2 Evidencia del veredicto GPU reproducible localmente

```bash
# 0 kernels GPU en el repo (los matches son DAI/BAL/pubkey false-positives):
rg -i "cuda|a100|gpu_" backend/ --type rust --type ts   # → 0 matches reales
rg "^rayon" backend/**/Cargo.toml                       # → 0 matches
rg "worker_threads|RAYON|num_cpus" backend/searcher-rs/src/  # → 0 en producción
```

### 7.3 Invariantes

- **INV-HP06-1 (read-only):** este WO ejecutó 0 escrituras en VPS (solo docker
  ps/inspect/logs/stats, redis-cli XLEN/XINFO, wget localhost:9090 dentro de prometheus,
  du/vmstat/df/lscpu/free) y 0 requests al dominio público (0/5). Verificable en §1/§3.
- **INV-HP06-2 (RULE 00 en cotizaciones):** todo precio cita URL de list-price leída HOY;
  lo no verificado se declara UNKNOWN (SKU del host, VM genérica cloud, colocation,
  list AWS p4d) — jamás se rellenó con estimaciones silenciosas.
- **INV-HP06-3 (eras no mezcladas, INV-BR01-3 heredada):** toda cifra de latencia/throughput
  declara su ventana (2h→03:11:44Z, incluyendo el período degradado por disco lleno —
  declarado, no promediado con épocas sanas).
- **INV-HP06-4 (§32/§33/§34.3):** nada de este documento autoriza compra, deploy o flip;
  el terminus broadcast sigue default-deny; la recomendación de Alchemy PAYG es una
  decisión de operador sobre SU cuenta de proveedor.
- **INV-HP06-5 (veredicto falsifiable):** el rechazo de GPU incluye la condición exacta
  de re-evaluación (§5 Condición) — no es dogma, es telemetría con fecha de expiración.

### 7.4 Gate de re-verificación (re-ejecutable por cualquiera)

```bash
# 1. p95/p99 vivo + atribución por stage (localhost dentro de prometheus):
ssh arbx "docker exec arbitragex-v2-prometheus-1 wget -qO- 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.95%2C%20sum%20by%20(le)%20(rate(arbx_pipeline_latency_seconds_bucket%5B2h%5D)))'"
# Esperado hoy: ~0.72 (y por stage: construction_to_publish ~0.9 domina; publish_xadd ~0.002)

# 2. Congelación de emisión (outage):
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:detected | head -16"   # ×2 con delta
# Congelado: entries-added idéntico en ambas muestras; vivo: delta > 0.

# 3. Modo falla PG (hasta que operador aplique retención):
ssh arbx "docker logs arbitragex-v2-postgres-1 --tail 3 2>&1"   # FATAL: No space left on device

# 4. Hambre de RPC:
ssh arbx "docker logs arbitragex-v2-searcher-rs-1 --since 30m 2>&1 | grep -c alchemy_failed"  # hoy ~1,620/30m

# 5. Saturación host (debe mostrar id alto, steal 0):
ssh arbx "vmstat 1 3 | tail -2; df -h / | tail -1"
```

---

## 8. Fail-honest — lo que NO pude verificar (declarado)

1. **SKU exacto del Hetzner actual** (cloud vs dedicado, precio que el operador paga hoy):
   UNKNOWN — la factura es del operador. La identificación CPX/CCX-class es INFERRED del
   string CPU + KVM + specs.
2. **AWS p4d.24xlarge list de HOY**: no re-verificado (aws.amazon.com devolvió 404 al fetch
   y el buscador 429). Se cita $32.7728/h como list-price estable conocido, marcado INFERRED.
3. **VM cloud genérica (AWS c7i/GCP c3) list-price**: no verificado hoy (rate-limit) —
   UNKNOWN; para no inventar, el tier T2 se cotizó por sus componentes VERIFICADOS (RPC
   managed + GPU) y la VM quedó marcada.
4. **Colocation 1U Frankfurt/Falkenstein**: no verificada hoy — UNKNOWN. El rango típico
   citado (€40-80/mes/U) es INFERRED de conocimiento general y se marca como tal.
5. **Cifras PG del embudo (rechazos por razón, sims por kind) HOY**: PostgreSQL está caído
   (§2) — no re-mediciones; se usan las de BR-01 con sus ventanas declaradas (09-07).
6. **Causa fina del edge unhealthy** (FailingStreak=122, output vacío): INFERRED que su
   healthcheck depende de api-server (restarting) — no se adjudicó a file:line en esta pasada.
7. **EUR/USD**: sin conversión inventada; cada tier se cotiza en su moneda de factura.
8. **Por qué el searcher no reinició en 38h pero su emisión sí se congeló**: la emisión
   depende de PG+Redis sanos; el proceso siguió vivo logueando WARNs. Sin contradicción,
   pero el path exacto emitter→freeze no se trazó a file:line aquí (obra de BR-10/F-1).

---

## 9. Entrega a la mesa (downstreams)

- **HP-07 (re-anclaje económico):** toma la condición GPU (§5) y la matriz §6 — tu escalera
  de aprobación ≥5%/10%/20% contra costos de infra VERIFICADOS hoy ($104-499/mes el
  primer escalón real; $10,726/mes la GPU que el SEED pone en el escalón 1).
- **BR-08 (latencia):** tu blanco está adjudicado a `construction_to_publish` (915ms p95,
  label nativo del searcher) — publish_xadd 2.39ms. El work es software (oracle quota +
  emission batching), no hardware; T2-RPC remueve solo los retries.
- **BR-02 (reserves):** hoy `pool_sync ok=0` por disco lleno — tu fix es prerrequisito
  también del tier upgrade: sin AOF sano no hay cache que sincronizar.
- **BR-10 (UI):** añade a tu contrato el alerta `disk.headroom` (§7.1) — el outage corrió
  50min sin señal de pipeline.
- **Orquestador/operador:** dos acciones de operador sin comprar hardware — (1) retención
  PG (playbook ARBX-RETENTION-01) para revivir el pipeline, (2) decisión Alchemy
  Free→PAYG/QuickNode contra el 77.9% de S5. Este documento no ejecuta ninguna de las dos
  (§32/§33); las pone sobre la mesa con su evidencia.

**Veredicto HP-06:** el sistema NO está limitado por cómputo — está limitado por (1)
capacidad de disco/retención [outage activo], (2) cuota de RPC externa [77.9% de las
muertes], (3) I/O-orquestación serial [p95 915ms en construction], (4) cobertura de
simulación [0% aprobación]. La escalera de compra correcta empieza en ~$104/mes (cuota) y
termina en bare-metal FSN solo si el terminus broadcast algún día se habilita (§34.3). La
GPU 4×A100 del SEED es capital que acelera la única parte del pipeline que hoy produce
cero: rechazada con números, con puerta de re-entrada escrita.
