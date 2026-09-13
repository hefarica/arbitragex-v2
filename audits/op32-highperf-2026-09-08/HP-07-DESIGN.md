# HP-07 — REPORTE DE DISEÑO (economics-validator PhD, mesa OP-32)

> **WO:** HP-07 · **kind:** design · **agente:** economics-validator · **Fecha:** 2026-09-08.
> **Entregable principal:** `audits/op32-highperf-2026-09-08/HP-07-ECON.md` (re-anclaje
> completo con fórmulas reproducibles, rejilla de sensitividad, clasificación de cada claim,
> invariantes INV-HP07-1..4 y gates GATE-HP07-F0/Y/S/F1/RPC). Este archivo es el reporte
> corto a la mesa. **0 código de producción editado · 0 git · VPS read-only · HTTP 0/5.**

## Sincronía de mesa (qué leí antes de opinar)

Board `GOAL-WORKORDERS.md` + `OPERADOR-SEED-2026-09-08.md` (op32) · `BR-01-FORENSE-EMBUDO.md`
(§10 era-2 madura — mi base medible) · `BR-00-APPLY.md` + `BR-00-REVERIFY.md` (fix PASS en
código, NO desplegado — verificado con `git rev-parse HEAD` = e65040f1 en VPS) · board cerebro
(línea 39: break-even 5%≈$1,170/mes con $2.3M — cita exacta usada) · canon mundial
`world/mev-practice/FINDINGS.md` (márgenes reales de searchers) ·
`docs/ROUTES_CROWN_JEWEL_DOCTRINE.md` (financing = dimensión de ruta). Al abrir, era el primer
reporte de la mesa op32 — no había pares a los que citar más allá de cerebro.

## ⚠️ HALLAZGO P0 FUERA DE WO (medido al abrir la sesión — acción de operador)

**El pipeline está caído desde 2026-09-09 02:49Z por disco lleno**: `df /` = 145G/150G usados;
Postgres crash-loop (`No space left on device`, RestartCount=9) y api-server con él; Redis AOF
`aof_last_write_status:err` (MISCONF) → emisión CONGELADA (`entries-added` estacionario en
7,217,889; antes +48.7/s) y RU-3 degradado ~50× (5/264 bloques con ciclos; mecanismo:
`route_discovery.tick_snapshot_set_failed` ×270/90min). Causa raíz: volumen PG 99G (era 32G el
09-04) — la era-2 de #555 genera ~4.2M filas/día de valor $0 a ~16GB/día neto. Primer auxilio
del OPERADOR (yo soy read-only): `docker builder prune` libera ~21.5GB reclaimable medido;
luego retención PG + alarma disco 80%. Precedente idéntico: ENOSPC 2026-09-04 (memoria).

## Hallazgos del re-anclaje (detalles y fórmulas en HP-07-ECON.md)

1. **Modelo obligatorio:** `R$/h = λ_sim × a × ȳ × W` con λ_sim=1,566 sims/h y a=**0.000%**
   (0 passed en 1,020,039 históricas, terminus XLEN=0 re-verificado vivo), ȳ=**UNKNOWN**
   (S5 muerto: 77.87% sin precio; columnas en unidades crudas — muestra viva
   `expected_profit_usd: 4.0e15`), W=**UNKNOWN** (relays 0/2). Toda cifra del SEED que no
   factoriza así es marketing (INV-HP07-1).
2. **La brecha no es 8.4× — es ∞/indefinida:** el escalón real previo a F1 es **F0 = primera
   sim passed de la historia**. El mercado NO es el bloqueo (gas medido hoy 0.0535-0.0571
   gwei — mínimo histórico; RU-3 señalaba 3,289 rutas/h).
3. **Sensitividad (techo, W=1):** 0.5%×$50 → $391/h (≈F1) · 5%×$100 → $7,830/h (≈F3) ·
   10%×$500 → $78,300/h. La rejilla "alcanza" la escalera SOLO porque hereda el flood era-2
   de densidad $0 — con W realista (0.1-0.5) baja 2-10×.
4. **Veredicto por tramo:** F0 BLOCKED (ingeniería: disco + BR-00 deploy + S5 + signer probe
   41.1% reverts + reserves). **F1 ACHIEVABLE-IN-PRINCIPLE, NO desde aquí** (banda de los 16
   searchers chicos, $192K/mes media — arXiv:2507.13023; exige uptime ≥99% vs MTBF medido 38h
   y storage neto ≤0 vs +16GB/día). **F2 UNACHIEVABLE con arquitectura actual** (puertas del
   top-3; exige builder-integration + p95<100ms; hoy p95=764ms). **F3/F4 UNACHIEVABLE —
   exceden el mercado:** F4 ≥$7.2M/mes = 59% del agregado CEX-DEX completo ($233.8M/19 meses
   = $12.31M/mes) y 22% del MEV total que el propio operador cita ($393M/año).
5. **Refutación interna más dura:** el análisis del propio operador (5%→$1,170/mes=$1.63/h;
   20%→$13,950/mes=$19.4/h) pone su break-even **61× debajo de F1** y **516× debajo de F4**,
   e implica ȳ≈$0.02/trade. Sus dos documentos (escalera vs break-even) no pueden estar
   ambos bien; mi re-derivación favorece el break-even como lo alcanzable primero.
6. **Claims clasificados:** escalera/$5M/ROI 10.000%/knobs de safety ($50K/$200K/$1M/500gwei/
   $100/$50K) = **HYPOTHESIS** (los knobs NO existen en el repo — grep 0; los reales son
   `min_profit_usd=2.0` trading_config.rs:756 y `max_gas_burn_usd=100.0` risk_ledger.rs:262);
   break-even del operador = PRIMARY_SOURCE con derivación INVERIFICABLE (no lineal);
   "mercado IDEAL, 0% es falla del sistema" = CANONICAL_REPO-CONFIRMADO (re-medido);
   **GPU 4×A100 = HYPOTHESIS con evidencia EN CONTRA** (cuellos medidos son I/O-orquestación,
   storage y RPC-429, no FLOPs — detalle de cotización es HP-06).
7. **UNKNOWNs con resolver (R8):** ȳ → BR-03+BR-00+7d paper ledger; W → shadow-bidding;
   bruto/fila storage → PG vivo (`pg_database_size`/`pg_wal`); derivación break-even →
   operador publica inputs.

## Cómo construí sobre los pares (y qué corrijo)

- **BR-01 (ecc:database-reviewer):** confirmo tu terminus XLEN=0 y tu §10.2 (columnas no-USD)
  con muestra viva del stream; tu proyección "~4M filas/día" la mido REAL (4.2M/día) y le
  agrego el costo: 16GB/día neto → disco 100% en 38h. Tu F-6 ("Redis es el contador de
  verdad") fue decisivo para medir con PG caído.
- **BR-00-APPLY/REVERIFY:** uso tu protocolo era-separada como INV-HP07-2 y tu ADV2/ADV3 como
  parte del GATE-HP07-F0. Tu fix sigue sin desplegar — es gate #2 de F0.
- **A nadie contradigo aún** (primer reporte de la mesa op32); dejo sentado para HP-06: la
  línea #1 de tu cotización debe ser storage/retención y RPC PAYG, no GPU.

## Gates que dejo instalados

GATE-HP07-F0 (primera passed real) → F0/Y (ȳ p50/p95 medido 7d paper) → S (14d disco ≤0) →
RPC (0 semanas 429) → F1 (30d paper ≥$100/h neto + uptime ≥99%). Flips y capital: operador
(§34.3 intacto, capital expuesto = 0).
