# HP-07 — RE-ANCLAJE ECONÓMICO HONESTO de la escalera del SEED

> **WO:** HP-07 · **kind:** design (análisis) · **agente:** economics-validator PhD (Gang Omniscience, mesa OP-32)
> **Fecha:** 2026-09-08 local (mediciones VPS 2026-09-09 02:44→03:06Z, VPS=UTC).
> **Charter:** re-derivar CADA número del SEED (`OPERADOR-SEED-2026-09-08.md` §🚀 PLAN DE ALTO
> RENDIMIENTO, líneas 168-175) desde telemetría medible del propio sistema. Ninguna cifra se
> acepta ni se rechaza por vibra.
> **Rubric R8/RULE 00:** todo factor no medible hoy se declara UNKNOWN con la medición que lo
> resolvería. Cero datos fabricados. §34.3: este análisis NO autoriza capital — capital
> expuesto = 0; "revenue" aquí es siempre un PRODUCTO MODELOADO en shadow/paper.
> **Método:** read-only total. SSH `arbx` (docker ps/inspect/logs, redis-cli read), grep de
> knobs en el árbol local, canon mundial `skills/arbitragex-ultra/world/mev-practice/FINDINGS.md`.
> HTTP público: 0/5. PG inaccesible durante la medición (ver §0 — incidente vivo).

---

## 0. INCIDENTE P0 EN VIVO (medido al abrir la sesión — condiciona TODO el análisis)

**El pipeline completo está caído por disco lleno desde ~2026-09-09 02:49Z.**

Evidencia (comandos en §7, todos re-ejecutables):

| Hecho | Medición (ventana 02:44-03:06Z 09-09) |
|---|---|
| Disco VPS 100% | `df -h /`: 150G total, **145G usados, 0 disponible** |
| Postgres crash-loop | `RestartCount=9`, log FATAL `could not write lock file "postmaster.pid": No space left on device` |
| api-server crash-loop | dependiente de PG, `Restarting (1)` |
| Redis AOF rechaza writes | `aof_last_write_status:err`, `MISCONF: No space left on device` |
| Emisión MUERTA | `entries-added arbx:opps:detected` congelado en **7,217,889** (estacionario en ventana de 45s; antes crecía a ~48.7/s) |
| RU-3 degradado ~50× | 264 bloques/60min: `cycles_found` Σ=2,500, media 9.5, **solo 5/264 bloques con ciclos** (09-07: 500 capped/bloque, 254-291 dispatched) |
| Mecanismo de la caída de RU-3 | `route_discovery.tick_snapshot_set_failed` ×270/90min (MISCONF) — sin snapshot persistido no hay universo de ciclos para el siguiente tick |
| Volumen PG | `arbitragex-v2_postgres_data` = **99G** (era 32G el 2026-09-04 post-ARBX-RETENTION-01, memoria del proyecto) |
| Crecimiento neto de disco | ~+73G en 4.5 días ≈ **16GB/día neto** con el cron de retención corriendo (era 48% usado el 09-04) |

**Lectura económica del incidente (la más importante del WO):** la era-2 de #555 (throughput
×66, ~175K opps/h) produce **~4.2M filas/día** cuyo valor de información actual es **$0**
(100% rejected, 0 passed JAMÁS), a un costo de almacenamiento de **~16GB/día neto** (bruto
por fila 5-11KB INFERRED — índices+TOAST+WAL; resolver con PG vivo: `pg_database_size` +
`pg_wal`). El sistema se auto-negó el servicio con su propia telemetría ANTES de aprobar su
primera simulación. Precedente exacto: ENOSPC 2026-09-04 (memoria) y WAL-burst 2026-09-04.
**Esto es un hecho del modelo de costos del SEED:** la escalera exige uptime ≥99.9%
(43 min/mes); el MTBF medido a carga era-2 es **38h**. Corrección de un factor ≥22× en
uptime ANTES de hablar de revenue.

Todo lo demás de este documento usa la última era sana (era-2 BR-01) como base medible.

---

## 1. INSUMOS MEDIDOS (la base, con clasificación)

Denominaciones: λ = throughput/hora · a = tasa de aprobación (passed/sims, misma ventana,
era-separada) · ȳ = Topological Yield neto medio por trade aprobado-y-ejecutado (USD) ·
W = win-rate del auction/builder (fracción de trades aprobados que captura el sistema en
competencia). **Revenue model: `R$/h = λ_sim × a × ȳ × W`.** Todo claim de $/h del SEED DEBE
expresarse en estos cuatro factores (INV-HP07-1) — lo que no puede, es marketing.

| # | Símbolo | Valor | Fuente / ventana | Clase |
|---|---|---|---|---|
| I1 | λ_opp | **175,276/h** (48.7/s) | Redis `entries-added`: 7,217,889 − 546,976 (BR-01 12:51Z 09-07) = 6,670,913 en 38.06h (era-2 12:46:44Z 09-07 → congelamiento 02:49Z 09-09) | PRIMARY_SOURCE (medido por mí + BR-01 §10.1) |
| I2 | λ_sim | **1,566/h** | 7,797 sims en 4.98h era-2 (BR-01 §10.3-F6). Cobertura 0.94% de opps (streams trim-starved) | PRIMARY_SOURCE (BR-01) |
| I3 | λ_ru3 | **3,289/h** rutas `is_opportunity` | RDO 3h: 9,866/3h (0.23% de 4.27M evaluaciones) — BR-01 §10.3 | PRIMARY_SOURCE (BR-01) |
| I4 | a | **0.000%** | **0 passed en 1,020,039 sims históricas**; era-2 0/7,797; terminus `XLEN arbx:opps:simulated = 0` JAMÁS (re-verificado vivo por mí) | PRIMARY_SOURCE |
| I5 | ȳ | **UNKNOWN** | S5 muerto: 77.87% de la era-2 muere `v3_quote_unavailable` SIN precio → `expected_profit_usd` NULL; las filas "positivas" guardan unidades crudas (media 1.6M unidades ≈ dust; muestra viva 09-09: `expected_profit_usd: 3.998e15` para USDC/WETH UniV2→Sushi, `amount_in_wei: 0`) | UNKNOWN — lo resuelve BR-03 (cascada oráculos) + BR-00 desplegado → distribución real de `net_expected_profit_usd` (normalizada por decimals) en ledger paper ≥7 días |
| I6 | W | **UNKNOWN** (hoy estructuralmente 0: relays 0/2 — certificación HG 2026-09-06) | No hay builder-integration viva ni papel de bidding | UNKNOWN — lo resuelve shadow-bidding contra inclusión observada (sin capital) |
| I7 | gas | **0.0535-0.0571 gwei** (09-09 02:5xZ, logs searcher `gas_price_gwei`) · 0.087 gwei (operador 09-07) | Mínimo histórico — viento de cola | PRIMARY_SOURCE (medido) |
| I8 | ETH | $2,489 (operador 09-07) | — | PRIMARY_SOURCE (operador) |
| I9 | MEV total | ~$393M/año (operador 09-07) | Consistente con canon mundial (builders solos pagan ~$14M/mes = $168M/año por prioridad, arXiv:2508.04003) | PRIMARY_SOURCE (operador) + cross-check CANONICAL_WORLD |
| I10 | Análisis break-even del operador | 5% a ≈ $1,170/mes (con $2.3M capital) · 10% ≈ $4,650/mes · 20% ≈ $13,950/mes · "gap crítico >10%; actual 0%" | `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md:39` | PRIMARY_SOURCE (declarado; derivación NO publicada — ver §5.4) |
| I11 | Estado BR-00 | PASS-with-advisories en código, **NO desplegado** (VPS sigue en `e65040f1` = PR #555) | `git rev-parse HEAD` vivo + BR-00-REVERIFY §0 | PRIMARY_SOURCE (medido por mí) |
| I12 | Latencia | p95 764ms vs target 29ms (FAIL); grafo sano 98-192ms/bloque | board cerebro + BR-01 §10.3 | PRIMARY_SOURCE |

---

## 2. LA ESCALERA DEL SEED POR TRAMOS — qué tiene que ser verdad

El SEED declara (`OPERADOR-SEED-2026-09-08.md:170-175`): fases **$100-500/h → $1K-3K/h →
$5K-10K/h → $10K+/h**, break-even mes 6, ~$5M año 1, ROI 10.000%+.

Conversión a $/mes (720h/mes): F1 $72K-360K · F2 $720K-2.16M · F3 $3.6M-7.2M · F4 ≥$7.2M.
"$5M año 1" = $416.7K/mes = **$579/h promedio** (entre F1-top y F2-base).

### 2.1 Requisitos de flujo por tramo (fórmula: `trades_aprobados/h = target$/h ÷ ȳ`)

A ȳ = $100 (el "min profit $100" del propio SEED, línea 175) y W = 1 (techo ingenuo — ver
§4.1 por qué W=1 es irreal):

| Tramo | Target | Trades aprobados/h req | a implícita sobre λ_sim=1,566/h | % del techo de detección λ_ru3=3,289/h |
|---|---|---|---|---|
| F1a | $100/h | 1.0 | **0.064%** | 0.030% |
| F1b | $500/h | 5.0 | 0.319% | 0.152% |
| F2a | $1,000/h | 10.0 | 0.639% | 0.304% |
| F2b | $3,000/h | 30.0 | 1.916% | 0.912% |
| F3a | $5,000/h | 50.0 | 3.193% | 1.52% |
| F3b | $10,000/h | 100.0 | 6.386% | 3.04% |
| F4 | ≥$10K/h | ≥100 (a ȳ=$100) · ≥20 (a ȳ=$500) | ≥6.39% · ≥1.28% | ≥3.04% · ≥0.61% |

**Hallazgo estructural:** el suministro de detección NO es el cuello — F4 completo cabe en el
3% del techo λ_ru3. Los cuellos son **ȳ (S5 muerto)**, **a (BR-00 sin desplegar + reverts)**,
**W (0 relays)** y **uptime (38h MTBF)**. Cada tramo exige, además del flujo:

### 2.2 Condiciones de verdad por tramo (medidas, no aspiracionales)

| Condición | F1 | F2 | F3/F4 | Estado medido HOY |
|---|---|---|---|---|
| Primera sim passed (JAMÁS ha habido una) | necesaria | necesaria | necesaria | **0/1,020,039**; terminus XLEN=0 |
| S5 oráculo vivo (ȳ medible) | necesaria | necesaria | necesaria | 77.87% del flujo muere sin precio; Alchemy 429 ×783/20min (BR-01 F-3) |
| BR-00 desplegado | necesaria | necesaria | necesaria | código PASS, VPS sin deploy (e65040f1) |
| Probe anvil sano | necesaria | necesaria | necesaria | 41.1% de sims que corren = `reverted` (signer sin balance/approval) + 20.8% timeout |
| Uptime | ≥99% (7.2h/mes) | ≥99.9% (43min/mes) | ≥99.95% (22min/mes) | **caída total a las 38h** (disco); redeploys cuestan 5-7min de feed (N4) |
| Latencia p95 emit | <12s (alcanzar bloque sig.) | <100ms | <29ms + colocation | 764ms (FAIL 26×) |
| Storage neto | ≤0 GB/día | ≤0 | ≤0 + tiering | **+16GB/día** (100% disco) |
| RPC quota | PAYG / cascada | PAYG + multi-provider | dedicado/nodos propios | free-plan saturado A CERO revenue |
| Builder/relay integration | útil | **necesaria** (W>0) | necesaria + colocation | relays 0/2 (Eden/Beaver inexistentes — cert. HG) |
| Capital TLS (flash) | financing ya es dimensión de ruta (AAVE_FL/BALANCER_FL/V2_FLASH_SWAP) | ídem | ídem + depth | fees se leen on-chain (doctrina); §34.3 intacto — el análisis NO lo autoriza |

**Nota sobre capital:** la doctrina de rutas (financing = dimensión de ruta) implica que el
$2.3M de capital propio del análisis 2026-09-07 NO es prerrequisito de la escalera si los
modos TLS funcionan; el requisito real es que S5/Bro tome el costo TLS (0.05% Aave HOY,
leído on-chain) dentro de ȳ neto.

---

## 3. SENSITIVIDAD: aprobación × Topological Yield → $/hora esperado

`R$/h = λ_sim × a × ȳ × W` con λ_sim = 1,566/h medido (era-2) y **W = 1 (TECHO — sin
competencia)**. Tabla: $/h para a ∈ {0.5, 1, 5, 10}% × ȳ ∈ {$10, $50, $100, $500}:

| a \ ȳ | $10 | $50 | $100 | $500 |
|---|---|---|---|---|
| **0.5%** | $78 | $391 | $783 | $3,915 |
| **1%** | $157 | $783 | $1,566 | $7,830 |
| **5%** | $783 | $3,915 | $7,830 | $39,150 |
| **10%** | $1,566 | $7,830 | $15,660 | $78,300 |

Lecturas honestas:
1. **La rejilla numéricamente "alcanza" la escalera** (p.ej. 5%×$100 = $7,830/h ≈ F3) — PERO
   solo porque λ_sim hereda el flood era-2 cuya densidad de valor es $0 (I5: ȳ UNKNOWN,
   S5 muerto). Es una rejilla de TECHO, no de expectativa.
2. **El factor faltante W destruye la rejilla en mainnet:** backrun atómico es un auction.
   Canon mundial: 11 searchers = 80%+ del arb no-atómico (arXiv:2401.01622); CEX-DEX
   $233.8M/19 meses por 19 searchers, top-3 = 75%, rentabilidad correlacionada con lazos
   EXCLUSIVOS searcher-builder (arXiv:2507.13023); builders pagan ~$14M/mes por el primer
   cuartil (arXiv:2508.04003). Con W ∈ [0.1, 0.5] realista para un recién llegado, multiplica
   cada celda hacia abajo 2-10×.
3. **Sensibilidad a λ_sim:** la cobertura de sims hoy es 0.94% de opps (inanición por trim).
   Si BR-04/retención sanan los streams y la cobertura sube a ~50%, λ_sim ×53 — la rejilla
   escala linealmente. Inversamente, si el fix de storage (§0) recorta el flood sin valor,
   λ_sim puede CAER sin pérdida económica (las filas recortadas tienen ȳ indemostrable).
4. **Sensibilidad al régimen de gas:** I7 es un mínimo histórico (0.054 gwei). A 20-50 gwei
   (norma 2024) el set marginal de rutas colapsa vía `gas_floor_breach` (precedente G-ECON:
   48% de nets eran 0 honesto por gas floor). **La escalera debe re-derivarse por régimen de
   gas** — el viento de cola actual es temporal y no puede anclarse como constante.

---

## 4. RIESGOS del análisis del operador (validados contra world/mev-practice)

### 4.1 Competencia de searchers institucionales — CONFIRMADO como el riesgo dominante
- $132B de volumen atribuido a arb no-atómico, **11 searchers = 80%+** (arXiv:2401.01622,
  IEEE S&P 2024). CEX-DEX: **$233.8M extraídos por 19 searchers en 19 meses**; **3 searchers
  capturan 3/4**; la rentabilidad correlaciona con integración de builder y lazos exclusivos
  (arXiv:2507.13023, AFT 2025). Traducción a la clase de referencia (INFERRED, aritmética
  sobre la fuente): agregado ≈ **$12.31M/mes**; media/searcher $648K/mes; top-3 ≈ $3.08M/mes
  c/u; los otros 16 ≈ $192K/mes c/u.
  - **F1 ($72-360K/mes)** = banda de los 16 chicos. **F2 ($720K-2.16M)** = puertas del top-3.
    **F3 ($3.6-7.2M) EXCEDE la media del top-3. F4 (≥$7.2M) = 59% del agregado CEX-DEX de
    TODO el mercado** y 22% del MEV total que el propio operador cita (~$393M/año → $32.75M/mes).
- La evidencia de bidding: el mejor resultado publicado (PPO bidder MEV-X, arXiv:2510.14642)
  captura 80.93% vs 56.54% del incumbente — **el auction se gana con política de bid, no con
  más detección**. El sistema no tiene política de bid viva (relays 0/2).

### 4.2 Gas variable — CONFIRMADO con dirección favorable HOY, régimen no-anclable
- Medido 0.0535-0.0571 gwei (I7) — mínimo histórico. El modelo net-profit ya es honesto
  (`gas_floor_breach` con kelly_gas_safety_multiplier, `size_optimizer.rs:524-533`). El
  riesgo es calendar el break-even sobre un régimen de gas que es cola de distribución.

### 4.3 Falla adversarial / de simulación — CONFIRMADO con números propios
- 41.1% de sims era-2 que corrieron = `reverted` (TRANSFER_FROM_FAILED/STF — probe signer sin
  balance/approval), 20.8% timeout (BR-01 F-7). **El simulador no sobrevive contacto con el
  fork** — prerrequisito de cualquier a > 0.
- Stale state es el driver DOMINANTE de shortfall de routing (2.02bps/trade medios, $24M —
  arXiv:2607.20762): la ventaja es frescura + split conjunto, no más rutas. Coherente con
  had_reserves=f 56.5% (BR-02) y p95 764ms (BR-08).
- La extracción óptima está certificada formalmente (Lean, arXiv:2510.14480) — los
  adversarios de top-tier conocen su techo exacto; no hay alpha matemática residual en la
  topología básica.

### 4.4 Riesgo NO listado por el operador, medido HOY: auto-DoS de infra (§0)
- El costo de telemetría por fila (5-11KB bruto, 16GB/día neto) con valor de información $0
  llenó el disco y mató detección+emisión+persistence en 38h. Ninguna fase de la escalera es
  operable sin storage neto ≤0. La propuesta GPU 4×A100 (SEED línea 171) ataca un cuello que
  NO existe medido: enumeración 98-192ms/bloque (sana), sims I/O-bound (anvil), oráculo
  RPC-hambriento (429). **Veredicto económico GPU: HYPOTHESIS con evidencia en contra** —
  el dinero va a storage/retención + RPC PAYG + latencia de emisión (el detalle de
  cotizaciones es de HP-06).

### 4.5 RPC quota como piso de costos (medido)
- Alchemy 429 ×783/20min a CERO revenue (BR-01 F-3; la cuenta free ya saturó). Antes del
  primer dólar hay un costo fijo PAYG/multi-provider que el SEED no modela.

---

## 5. VEREDICTO POR TRAMO — achievable/unachievable con la brecha exacta

### 5.1 El denominador de la brecha es cero — la escalera está gated en "primera approved"
a_medida = 0/1,020,039 = **0.000% exacto**. Toda brecha multiplicativa ("requiere 8.4× la
aprobación actual") es **indefinida (∞)**: el sistema no ha aprobado NUNCA una sim. El primer
tramo real de la escalera no es $100/h — es **F0 = 1 passed en la historia**. Gates medidos
que bloquean F0, en orden: (1) incidente disco (§0), (2) deploy BR-00 (código listo, PASS),
(3) S5/BR-03 (77.87% sin precio), (4) signer del probe (41.1% reverts), (5) reserves BR-02.

### 5.2 Veredicto por tramo

| Tramo | Veredicto | Brecha exacta vs medido | Razón |
|---|---|---|---|
| **F0** (implícito) 1 passed | **BLOCKED** (ingeniería, no mercado) | 0 → 1 en 1,020,039 | cadena de gates §5.1; el mercado NO es el bloqueo (gas 0.054 gwei, 3,289 rutas/h señaladas) |
| **F1** $100-500/h ($72-360K/mes) | **ACHIEVABLE-IN-PRINCIPLE, NO desde el estado actual** | requiere a=0.064-0.32% @ȳ=$100 con W real; hoy a=0.000% y ȳ=UNKNOWN | dentro de la banda de los 16 searchers chicos ($192K/mes media — arXiv:2507.13023); PERO exige: S5 vivo + BR-00 + probe sano + uptime ≥99% (hoy 38h MTBF) + storage ≤0 + RPC PAYG. Ninguna imposible; ninguna existente |
| **F2** $1K-3K/h ($720K-2.16M/mes) | **UNACHIEVABLE con la arquitectura actual; HYPOTHESIS a horizonte** | a=0.64-1.92% @ȳ=$100 CON W>0; W hoy estructuralmente 0 (0 relays) | puertas del top-3 mundial; exige builder-integration + p95 <100ms + política de bid (canon 4.1). Un solo VPS Hetzner con p95 764ms no juega esa liga |
| **F3** $5K-10K/h ($3.6-7.2M/mes) | **UNACHIEVABLE** — excede la media del top-3 mundial ($3.08M/mes) | a=3.2-6.4% @ȳ=$100 con W>0.5 | clase de referencia: ningún searcher individual fuera del #1-2 captura esto; además 11-22% del MEV total anual del propio operador |
| **F4** $10K+/h (≥$7.2M/mes) | **UNACHIEVABLE — el mercado es más chico que el claim** | ≥6.4% @ȳ=$100 sostenido = 59% del agregado CEX-DEX mensual completo, 22% de TODO el MEV ($393M/año del propio operador) | el techo del mercado acota antes que la ingeniería; sostenido 24/7 en un niche atómico DEX-DEX no existe evidencia de un capturador de esa escala |

### 5.3 El claim compuesto: break-even mes 6 · $5M año 1 · ROI 10.000%
- **break-even mes 6:** el costo fijo mínimo (Hetzner + storage + RPC PAYG) es ~$0.7-2.8/h
  equivalente — POR DEBAJO de F1 y CONSISTENTE con la propia línea del operador (5% a →
  $1,170/mes = $1.63/h). Veredicto: **plausible SOLO a escala dust** (si F0 y S5 aterrizan
  en <2 meses — la velocidad medida del programa: BR-00 tomó 1 día de código y sigue sin
  deploy). Break-even NO implica estar en la escalera.
- **$5M año 1** ($579/h promedio): = 2.2× la media de los 16 searchers chicos, 13.5% de la
  media del top-3. **HYPOTHESIS** — ninguna cifra medida del sistema lo soporta hoy (todos
  los factores de revenue son 0 o UNKNOWN).
- **ROI 10.000%+**: $5M/$50K. La infra cotizada por el propio SEED (Erigon dedicado + 4×A100
  + colocation) cuesta ≥$18-36K/año SOLO en GPU (cotización exacta es de HP-06) + storage
  (medido: la necesidad existe YA a $0 revenue) + RPC PAYG (429 medidos) → base realista
  ≥$50-150K/año → ROI techo 3,300-10,000% SOLO si el revenue llega a $5M con costo marginal
  ~0. **INTERNAMENTE INCONSISTENTE** como promesa; es un caso límite, no un plan.

### 5.4 Refutación interna más dura: el análisis del operador CONTRADICE su escalera
Las cifras del propio operador (I10) son la refutación cuantitativa de su escalera:
- 5% a → $1,170/mes = **$1.63/h** → F1 ($100/h) está **61×** arriba.
- 10% a → $4,650/mes = **$6.46/h** → F1 está **15-77×** arriba.
- 20% a → $13,950/mes = **$19.4/h** → F4 está **516×** arriba.
Consistencia interna de esas cifras con mi λ_sim: implican ȳ ≈ $1,170/(720×1,566×0.05) ≈
**$0.021/trade** (con denominador opps: $0.016) — o sea, el modelo del operador asigna a cada
trade aprobado un yield de ~2 centavos. Además sus tres puntos NO son lineales en a
($1,170→$4,650→$13,950 para 5→10→20%: marginales $3,480/5pp y $9,300/10pp) → la derivación
no es un modelo único publicado; clasifico los tres números PRIMARY_SOURCE-declarados con
derivación INVERIFICABLE. **Conclusión para el operador: o la escalera está 2-3 órden arriba,
o el break-even está 2-3 órden abajo — los dos documentos no pueden estar ambos bien.** Mi
re-derivación (§2-§3) indica: el break-even dust es lo alcanzable primero; la escalera
requiere ȳ y W que hoy no tienen ninguna medición que los soporte.

---

## 6. CLASIFICACIÓN de cada claim económico del SEED

| Claim (SEED línea) | Clase | Evidencia |
|---|---|---|
| Escalera $100-500/h → $10K+/h (170) | **HYPOTHESIS** | sin derivación; F3/F4 refutados por clase de referencia (§5.2); F1-2 gated en F0 |
| Break-even mes 6 (174) | **HYPOTHESIS** | plausible solo a escala dust (§5.3); requiere F0+S5 en <2 meses |
| ~$5M año 1 (174) | **HYPOTHESIS** | $579/h promedio sin factor medido que lo soporte |
| ROI 10.000%+ (174) | **HYPOTHESIS internamente inconsistente** | base implícita $50K < costo anual de la propia infra propuesta |
| MIN_PROFIT_FOR_BUNDLE $500 (172) | **HYPOTHESIS** | no existe en el repo; CANONICAL_REPO real: `min_profit_usd: 2.0` default (`backend/shared-rs/src/trading_config.rs:756`), `min_profit_wei=1wei` placeholder (`backend/relays-client/src/bundle_builder.rs:313`) |
| Safety $50K/$200K/$1M/500 gwei/$100/$50K-día (175) | **HYPOTHESIS** (propuesta) | grep repo: 0 knobs con esos nombres; CANONICAL_REPO real: `max_gas_burn_usd: 100.0` (`backend/shared-rs/src/risk_ledger.rs:262`), `max_gas_usd` (`trading_config.rs:164`) — los valores del SEED serían NUEVOS knobs, no upgrades de existentes |
| "el mercado está IDEAL, 0% aprobación es falla del sistema" (board cerebro :38-39) | **CANONICAL_REPO-CONFIRMADO** | re-medido: gas 0.054 gwei, terminus XLEN=0, 0/1,020,039; la falla es del sistema (S5/S8/infra), no del mercado |
| Break-even 5%≈$1,170 / 10%≈$4,650 / 20%≈$13,950 con $2.3M (board cerebro :39) | **PRIMARY_SOURCE declarado, derivación INVERIFICABLE** | no lineal en a; implica ȳ≈$0.02 (§5.4) |
| MEV ~$393M/año, builder Titan 53% (:38) | **PRIMARY_SOURCE** (operador 09-07) + cross-check consistente con canon ($14M/mes builders, arXiv:2508.04003) | — |
| Gas 0.087 gwei (:38) / ETH $2,489 | **PRIMARY_SOURCE** (operador 09-07); gas re-medido 0.0535-0.0571 gwei 09-09 | — |
| GPU 4×A100 acelera el sistema (171) | **HYPOTHESIS con evidencia en contra** | cuellos medidos son I/O-orquestación (oráculo 429, reserves, emitter p95 764ms, storage), no FLOPs; enumeración 98-192ms sana; canon: producción usa optimización exacta, ML solo para política (FINDINGS §5) |

---

## 7. FÓRMULAS Y COMANDOS REPRODUCIBLES (inputs → outputs)

**Modelo:** `R$/h = λ_sim × a × ȳ × W` · `trades/h = target$/h ÷ ȳ` ·
`a_req = trades/h ÷ λ_sim` · `$/mes = R$/h × 720`. Factores: λ_sim=1,566/h, λ_ru3=3,289/h,
λ_opp=175,276/h, a=0.000%, ȳ=UNKNOWN, W=UNKNOWN.

```bash
# 1) Emission rate + terminus (Redis es el contador de verdad — F-6 BR-01):
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:detected | grep -A1 entries-added; \
  docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:simulated"
# λ_opp = Δentries-added/Δh. (Salida de este WO: 7,217,889 congelado; XLEN simulated=0.)

# 2) Approval rate era-separada (cuando PG reviva — hoy BLOCKED por §0):
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F'|' -c \
 \"SELECT passed, count(*) FROM simulations WHERE simulated_at >= 'TS_DEPLOY' GROUP BY 1;\""
# a = passed/total de LA MISMA ventana (INV-HP07-2). Baseline: 0/1,020,039 histórico.

# 3) Taxonomía de muerte (vista D1 de BR-01) + conservación Σ==COUNT (INV-BR01-1).

# 4) Detección viva (RU-3): agregar route_scanner.done:
ssh arbx "docker logs arbitragex-v2-searcher-rs-1 --since 60m 2>&1 | grep -o \
 'route_scanner.done.*cycles_found\":[0-9]*' | grep -o '[0-9]*$' | \
 awk '{n++; s+=\$1; if(\$1>0)nz++} END {print \"blocks=\"n, \"sum_cycles=\"s, \"blocks_with_cycles=\"nz+0}'"
# (Salida de este WO: blocks=264 sum=2500 blocks_with_cycles=5 — degradado por §0.)

# 5) Disco/storage:
ssh arbx "df -h /; du -sh /var/lib/docker/volumes/arbitragex-v2_postgres_data"
# (145G/150G usados; volumen PG 99G. Bruto/fila = Δvol/Δfilas era-2 ≈ 67G/6.7M ≈ 10.3KB — INFERRED.)

# 6) Gas vivo (telemetría del propio searcher):
ssh arbx "docker logs arbitragex-v2-searcher-rs-1 --since 120m 2>&1 | grep -ioE 'gas_price_gwei\":[0-9.]+'"
# (Salida: 0.0535-0.0571 gwei.)

# 7) ȳ (resolver UNKNOWN): post BR-03+BR-00, sobre ledger paper ≥7d:
#   SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY net_expected_profit_usd)  -- p50
#   FROM paper_trade_runs WHERE executed_at >= TS AND net_expected_profit_usd > 0;
#   (con normalización por decimals VERIFICADA — §10.2 BR-01 prohíbe leer la columna como USD hoy)
```

### Invariantes (INV-HP07)
- **INV-HP07-1 (factorización):** todo claim de $/h se expresa como λ×a×ȳ×W con cada factor
  medido (ventana citada, era-separada) o declarado UNKNOWN. Factor invisible = claim inválido.
- **INV-HP07-2 (denominador):** `a` se computa sobre sims de la misma ventana; jamás mezclar
  eras (INV-BR01-3 de BR-01 aplica).
- **INV-HP07-3 (§34.3):** el modelo es shadow/paper; capital expuesto = 0; nada aquí autoriza
  flips ni broadcast. Revenue modelado ≠ revenue.
- **INV-HP07-4 (clase de referencia):** toda comparación de escala cita la fuente (arXiv o
  medición propia) — prohibido el "un searcher gana $X" sin fuente.

### Gates de decisión (cada tramo abre SOLO con su gate medido; flips = operador)
- **GATE-HP07-F0:** primera sim passed de la historia (XLEN `arbx:opps:simulated` ≥1 con
  passed real, protocolo ADV2/ADV3 de BR-00-REVERIFY). Prereq: incidente disco resuelto +
  BR-00 desplegado + S5 vivo.
- **GATE-HP07-Y:** 7 días de ledger paper con distribución ȳ p50/p95 medida (resuelve I5).
  Sin este gate, la rejilla §3 es techo, no expectativa.
- **GATE-HP07-S:** 14 días de crecimiento neto de disco ≤0 a carga era-2 (retención/rollup +
  no-persistir lo muerto sin precio — BR-01 §10.4). Sin esto, uptime F1 no existe.
- **GATE-HP07-F1:** 30 días de revenue paper ≥$100/h promedio con net honesto (fees on-chain
  + gas) Y uptime ≥99% medido. SOLO ENTONCES la conversación F1 es real.
- **GATE-HP07-RPC:** 0 semanas con quota-429 (PAYG/cascada viva) antes de intentar F1.

---

## 8. Fail-honest: lo que NO pude medir / declaro UNKNOWN

1. **ȳ (I5)** — el hallazgo económico central es un agujero: el sistema NUNCA ha computado
   un Topological Yield neto real en USD (S5 muerto + columnas en unidades crudas). Resolver:
   BR-03 + BR-00 + 7d paper ledger (§7.7). Mientras tanto, CUALQUIER $/h del SEED o mío es
   un techo condicional, no una proyección.
2. **W (I6)** — sin integración builder/relay viva no hay observación de win-rate. Resolver:
   shadow-bidding contra inclusión observada (diseño, sin capital).
3. **Derivación del break-even del operador (I10)** — no publicada; sus tres puntos no son
   lineales; no puedo reconstruir su fórmula. Resolver: que el operador publique inputs.
4. **Bruto/fila de storage** — INFERRED 5-11KB (PG caído impide `pg_database_size`/`pg_wal`).
5. **Costos exactos de infra tiers** — fuera de mi WO; HP-06 cotiza. Mis cotizaciones
   preliminares de GPU son órden de magnitud del canon, no quotes.
6. **Mix canonical_dispatch vs event-driven** — 4.2% vs ~95.8% (BR-01 §10.1 corrección);
   split fino UNKNOWN.
7. **PG fresco completo** (taxonomía 24h, sims por kind post-BR-00) — BLOCKED por el
   incidente §0; los queries están listos (§7.2-7.3).

---

## 9. Entrega a la mesa (downstreams)

- **Operador (URGENTE, §0):** disco 100% → PG crash-loop + Redis AOF rechaza writes +
  emisión muerta + RU-3 a ~0 desde 02:49Z 09-09. Acciones de operador (yo read-only):
  `docker builder prune` libera ~21.5GB (reclaimable medido) como primer auxilio; luego
  retención/purga PG y alarmas de disco al 80%. El precedente 09-04 aplica (builder prune
  14.7GB).
- **HP-06 (infra):** tu cotización debe incluir storage/retención como línea #1 de costos
  (medido 16GB/día neto) y RPC PAYG como piso (429 a $0 revenue); la GPU tiene evidencia en
  contra (§4.4). La escalera no es computo-bound.
- **HP-01 (censo):** clasifiqué los knobs de safety del SEED como HYPOTHESIS (grep negativo)
  — confirma en tu tabla 1:1 con `trading_config.rs:756` / `risk_ledger.rs:262` como
  CANONICAL_REPO real.
- **HP-08 (verify):** el browser-verify de este WO es imposible hasta resolver §0 (el
  dominio sirve el frontend cacheado pero la API viva está caída).
- **Mesa cerebro BR-03:** eres EL gate económico (I5): sin tu cascada de oráculos, ȳ
  permanece UNKNOWN y toda la escalera del SEED permanece no-evaluable más allá de techo.
- **BR-04:** tu anti-flood ahora tiene un argumento de COSTO medido: 16GB/día por telemetría
  de valor $0 — no-persistir lo muerto sin precio es ahorro de infra, no solo de CPU.

**Veredicto HP-07 (una línea):** la escalera del SEED está correctamente ordenada en su
arquitectura (detección→valuación→simulación→ejecución) y F1 es alcanzable-en-principio por
clase de referencia, pero HOY todos sus factores de revenue son 0 o UNKNOWN, la infra
medida se auto-deniega a las 38h, y F3/F4 exceden el tamaño del mercado que el propio
operador cita — la escalera honesta de hoy es F0 (primera sim passed), y el propio análisis
del operador (5%→$1.63/h) pone su break-even 61× por debajo de su primera fase.
