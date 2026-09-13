# HP-06 — DESIGN/REPORTE: plan de infraestructura con costos reales de HOY

> **WO:** HP-06 · **kind:** design · **agente:** devops-platform PhD (rubric ecc:performance-optimizer
> embebida: evidencia PRIMERO) · **Fecha:** 2026-09-08 (ventanas VPS UTC 2026-09-09 02:54→03:12Z).
> **Entregable de decisión completo:** `audits/op32-highperf-2026-09-08/HP-06-INFRA.md`
> (tabla tiers×costo×latencia×cuello, veredicto GPU, telemetría citada comando+output+timestamp).
> Este archivo es el reporte de la mesa: diseño, invariantes, gate y sincronía con los pares.
> **Estado:** DESIGN ONLY — 0 compras, 0 deploy, 0 flips, 0 git, 0 escrituras VPS, 0/5 requests
> al dominio público. §32/§33/§34.3 intactos.

---

## 1. Sincronía de mesa redonda (qué leí, sobre qué construyo)

- `audits/op32-highperf-2026-09-08/GOAL-WORKORDERS.md` (kanban HP-01..HP-08; HP-06 exige
  BR-08 p95 medido como ancla) y `OPERADOR-SEED-2026-09-08.md` (los claims de infra/GPU
  que clasifico — §4 abajo).
- `audits/cerebro-2026-09-07/BR-01-FORENSE-EMBUDO.md` completo (contrato S1-S9, era-2
  madura: S5 77.86%, F-1..F-7, "la optimización tiene UN blanco hoy: S5"). **Construyo
  directamente sobre su telemetría** y la extiendo con medición propia viva (§2).
- `audits/cerebro-2026-09-07/BR-00-REVERIFY.md` (PASS-with-advisories; el fix NO está
  desplegado — verifiqué SHA `e65040f1` en VPS, todavía #555).
- `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md` (BR-08 764ms panel; break-even del
  operador 5%≈$1,170/mes).
- `~/.claude/skills/arbitragex-omniscience/LEARNINGS.md` (nombres de contenedores, pgrep
  auto-match, carrera de reserves 114/235 TTL 30s).
- No hay reportes previos de pares en `audits/op32-highperf-2026-09-08/` al momento de
  escribir (solo GOAL-WORKORDERS + OPERADOR-SEED — verificado con ls). Este es el primer
  entregable del programa que aterriza en el dir; HP-01/HP-07 citarán este archivo.

## 2. Telemetría viva NUEVA que aporta este WO (comando+output+ts en HP-06-INFRA §2-§3)

1. **OUTAGE ACTIVO del baseline por disco lleno** (nuevo, no estaba en ningún reporte):
   `/` 150G al 100% (0 bytes) a 02:54:41Z → PG crash-loop (`No space left on device`,
   RestartCount=9) → Redis AOF `MISCONF` → `pool_sync ok=0` → `cycles_found=0` →
   **emisión Redis congelada desde 02:09:01Z** (`entries-added 7,217,889` idéntico en dos
   muestras t0/t0+10s). PG data = 99G (creció 32G→99G desde la retención del 09-04 ≈
   16GB/día por la emisión post-#555). **El tier actual falló por capacidad, con CPU 3-9%,
   steal 0, iowait 0.**
2. **p95/p99 medidos hoy**: 724.8ms/952.6ms (ventana 2h→03:11:44Z, Prometheus localhost).
   **Atribución por stage (labels nativos del searcher)**: `construction_to_publish`
   p95=**915.4ms** (el cuello), `emit_boundary` 46.5ms, `publish_xadd` **2.39ms** —
   380× de diferencia: el p95 es I/O-orquestación serial, no publish, no grafo
   (scanner 72-315ms/bloque). Esto PRECISA a BR-08/BR-01 §10.4 (ellos lo inferían del
   emitter I/O; hoy está medido por stage).
3. **Throughput vivo**: 85.7 ops/s (media 2h, incluye el período degradado); hambre de RPC
   medida: `alchemy_failed` 1,620/30min (HTTP 429, Prices API, tier Free 25rps) — BR-01 F-3
   sigue vivo y es la causa operativa del asesino S5 (77.86%).
4. **Cero software GPU en el repo**: grep CUDA/A100/GPU/rayon/worker_threads → 0 matches
   reales en backend (solo false-positives DAI/BAL/pubkey y tests con worker_threads=2).
   REV M 42 existe (CPU); tokio sin override (defaults a 8 cores).

## 3. Tabla de decisión (resumen; completa con fuentes en HP-06-INFRA §4)

| Tier | $/mes HOY (verificado, URL en INFRA) | p95 esperado | Cuello REAL que ataca |
|---|---|---|---|
| T0 baseline Hetzner cloud 8vCPU/16GB/150G (MEDIDO; SKU UNKNOWN) | reemplazo hoy: CPX42 €69.49 / CCX33 €138.49 (subió 2.73× el 2026-06-15) | **725ms medido** + outage disco | — |
| T1 Hetzner AX bare-metal (AX162 EPYC 9454P, FSN) | desde €59 (AX-line) + €39 setup; AX162 ~€199-209 | S2 <100ms estable; p95 construction ~sin cambio | disco (fin del modo falla), single-tenant, proximidad FSN |
| T2 cloud managed RPC (Alchemy PAYG / QuickNode Scale) + VM | Alchemy PAYG $0.45/M CU (~$104/mes ×10M reqs); QuickNode Scale $499; VM genérica UNKNOWN-hoy | ~100-250ms [INFERRED] al matar 429-retries | **S5 (77.9% de muertes)**, elasticidad disco |
| T2-GPU GCP a2-highgpu-4g 4×A100 | **$14.6935/h = $10,726/mes** (us-central1; AWS p4d 8×A100 ≈$23,924 [INFERRED, no reverificado]) | **0ms de mejora** | **nada del cuello medido** |
| T3 bare-metal + colocation FSN | AX162 €199-209; colo UNKNOWN-hoy | ídem T1; colo solo si se persigue broadcast (§34.3-gated) | single-tenant + NVMe propio |

**Veredicto GPU: RECHAZADA hoy** — aprobación 0/1,020,039 (0.000%) ⇒ acelerar sims multiplica
cero; S5 (77.9%) y reserves (56.5%) matan ANTES de simular; solo 0.94% del flujo llega a sim;
CPU 91-96% idle; 0 kernels CUDA en el repo; $10,726/mes = 9.2× el gross del break-even del
propio operador (5% ≈ $1,170/mes). Puerta de re-entrada escrita y falsifiable (aprobación >5%
sostenida + CPU>50% + kernel CUDA existente) — INFRA §5.

**Escalera de compra que emerge:** retención+cuota (~$104-499/mes) → T1 NVMe si el disco
aprieta → T2-RPC si la cuota paga corta → T3/colo solo para terminus broadcast real → GPU:
nunca hasta la condición. **Cero de esto se ejecuta aquí** (documento de decisión).

## 4. Claims del OPERADOR-SEED clasificados (resumen; tabla completa INFRA §5.2)

- **REFUTADOS hoy**: 4×A100 CUDA (sin kernel, cuello es I/O), rayon 64 cores (0 deps),
  TOKIO 64 threads (no existe el knob; 64 sobre 8 vCPU = contention), detector GPU batch
  10K txs (límite es cuota RPC 429, no cómputo).
- **PROPUESTA razonable post-cuota**: Erigon dedicado (hoy: 9 providers externos; anvil es
  fork de sim, no nodo).
- **INFERRED/condicional**: mempool sniper <50ms (feed dedicado + arreglar path V1 muerto,
  BR-01 F-4); Redis 32GB (irrelevante: RSS 658MiB; su AOF comparte el disco lleno).

## 5. Diseño: diff + invariante + gate (DESIGN ONLY, no aplicado)

- **Diff** (marcado `// WO HP-06 (2026-09-08)`, para el PR futuro del orquestador):
  knob `ARBX_EMIT_DISK_GUARD_PCT` en compose.prod (default 0 = sin cambio de behavior) que
  pausa la EMISIÓN antes del 100% de disco, + regla de alerta `disk.headroom` <10%/5m —
  el outage corrió 50+ min sin señal del pipeline. Texto exacto en **HP-06-INFRA §7.1**.
- **Invariantes**: INV-HP06-1 read-only (0 escrituras VPS, 0/5 dominio público) · INV-HP06-2
  RULE 00 en cotizaciones (todo precio con URL; UNKNOWN declarado: SKU host, VM cloud
  genérica, colo, AWS p4d) · INV-HP06-3 eras/ventanas declaradas (INV-BR01-3 heredada) ·
  INV-HP06-4 §32/§33/§34.3 (nada autoriza compra/deploy/flip) · INV-HP06-5 veredicto GPU
  falsifiable con condición de re-entrada.
- **Gate re-ejecutable**: 5 comandos ssh read-only (p95 por stage, congelación de emisión,
  modo falla PG, hambre 429, saturación host) — **HP-06-INFRA §7.4**.

## 6. Fail-honest (resumen; detalle INFRA §8)

SKU exacto del host actual UNKNOWN (factura del operador); AWS p4d list no reverificado
hoy (404+429) — marcado INFERRED; VM cloud genérica y colocation no verificadas hoy
(rate-limit del buscador) — UNKNOWN declarado; cifras PG del embudo hoy imposibles de
re-medir (PG caído) — se citan las de BR-01 con ventana; causa fina del edge unhealthy
INFERRED; EUR/USD sin conversión inventada.

## 7. Cross con pares (aterrizaron mientras yo medía — leídos y citados, no ignorados)

- **HP-07-ECON.md / HP-07-DESIGN.md (economics-validator):** doble medición INDEPENDIENTE
  del mismo outage — él midió `entries-added` estacionario en **7,217,889** a 02:49Z, yo a
  02:59Z: número idéntico, cero discrepancia. Su nota "la línea #1 de tu cotización debe
  ser storage/retención y RPC PAYG, no GPU" es exactamente mi escalera §3/INFRA §6 —
  convergencia, no contradicción. Precisión que aporto: el onset fino del congelamiento es
  **02:09:01Z** (`last-generated-id 1788919741812-0`), anterior a su observación 02:49Z
  (que es cuando el disco llegó al 100% medible en df; la secuencia completa y el edge
  unhealthy ~02:08-02:10Z están en INFRA §2.1). Su veredicto "GPU 4×A100 = HYPOTHESIS con
  evidencia EN CONTRA" queda ahora COTIZADO por mi lado: $10,726/mes GCP verificado.
- **HP-01-CENSO.md (fallback de HP-04):** confirma op_32 inexistente y checklists [x]
  aspiracionales — consistente con mi clasificación de los claims del SEED (§4); no toca
  infra. No contradigo nada.
- **OPERADOR-SEED-V2-2026-09-08.md:** aterrizó a las 21:59Z; mi clasificación (§4) se hizo
  contra el SEED v1 (fuente citada por el board). Los claims de infra GPU no cambian entre
  v1/v2 en lo que clasifiqué (4×A100/rayon/TOKIO64 idénticos); si HP-07 u otro par censan
  deltas v2 en infra, extiendan mi tabla INFRA §5.2 — la dejo escrita para ser extendida.

## 8. Downstreams

- **HP-07**: usa §3/§5 de INFRA — tu escalera vs costos verificados ($104-499 primer
  escalón real; $10,726 la GPU rechazada).
- **BR-08**: blanco adjudicado a `construction_to_publish` 915ms (medido por stage).
- **BR-02**: pool_sync hoy ok=0 por disco — tu fix es prerrequisito de cualquier tier.
- **BR-10**: añade alerta disk.headroom (§5 diff).
- **Operador/orquestador**: 2 acciones de operador sin comprar hardware — retención PG
  (revive el pipeline HOY) y decisión Alchemy PAYG (mata el 77.9%). Este WO solo documenta.
