# /goal — OP-32 + HIGH-PERF DOCTRINE (orden del operador 2026-09-08)

> "He analizado el manual completo de 264 estrategias... guía técnica unificada que integre
> los 32 operadores (incluyendo el operador 32 desarrollado)... Profundiza en algún módulo
> específico, genera el código completo todas las estrategias, desarrolla el plan de
> infraestructura con proveedores cloud específicos y valida que estén bien. Agrega esto al
> /goal usa /arbitragex-omniscience." — Operador

**Fuentes íntegras**: `OPERADOR-SEED-2026-09-08.md` (v1: 32 operadores / matriz / infra /
proyecciones) · `OPERADOR-SEED-V2-2026-09-08.md` (v2, mismo día: estructuras de datos core
—TokenKey/TokenMeta/PairBucket triangular/DirectedEdge multigrafo/StrategyConfig con
hop_mask—, adjacency bitsets, pipeline discovery <30ms con presupuesto ms/stage
(2+3+4+3+7+5+3+2=29ms — COINCIDE con el target 29ms del panel de latencia BR-08), código
concreto de operadores, GPU CUDA/SIMD AVX/lock-free, FlashLoanExecutor/MevBoostSubmitter/
OnlineLearner, metas medibles — win rate >70% · Sharpe >3.0 · 10K opps/s · $1K-10K/h).

**Estado de verdad verificado en disco (orquestador, 2026-09-08)**: math-engine tiene
**op_01..op_31**; `mod.rs:6` documenta CÓMO añadir op_32 ("crear archivo + registrar en
registry.rs") — el op_32 NO existe aún. La numeración de la tabla del operador está
desfazada del canon (documento op_01="GD" vs repo op_01=svd; op_12="BFS" vs repo
op_12=mle; op_28="Shapley" vs repo op_29=shapley; op_30/op_31 también difieren). Los
checklists [x] del documento (implementado/testeado/desplegado) son aspiracionales.
ESTE PROGRAMA LOS HACE REALES.

## Kanban

| WO | Ítem | Acción | Estado |
|---|---|---|---|
| HP-01 | **Censo de verdad de la Guía del operador vs canon del repo**: tabla 1:1 de cada afirmación del SEED contra la realidad en disco — (a) los 31 operadores reales (`backend/math-engine/src/operators/op_*.rs`, registry) vs la tabla de 32 del documento (adjudicar el desfase de índices con mapeo exacto documento↔repo); (b) las 11 familias MEV-01..11 y sus conteos vs `skills/arbitragex-ultra/capability_matrix.json` + cartridges reales; (c) los mapeos estrategia×operador propuestos vs `knowledge_graph.jsonl` (2,511 edges) — qué edges existen, cuáles son nuevos, cuáles contradicen el canon; (d) HopMask/hops 2-7 vs los hop tiers reales (scanner gates h≤3 universo, h4-5 anchor, h6-8 shadow); (e) qué checklists [x] del documento son FALSOS hoy. Entregable: tabla de verdad + mapeo normativo | Censo read-only con evidencia file:line | PENDIENTE |
| HP-02 | **DISEÑO del op_32 — NSGA-II multi-objetivo** (rentabilidad neta / riesgo CVaR / latencia) para `backend/math-engine`: fast non-dominated sort + crowding distance + SBX + mutación polinomial + elitismo, PERO dimensionalizado al hot-path real del repo (budget de evaluaciones, sin allocs innecesarias §4.3, determinista/seedable para tests); contrato con el trait de operador existente; integración con la Master Matrix (264×31 → 264×32); selección por vector de preferencias [profit, risk, latency] normalizado; R8: frente de Pareto vacío = None honesto, jamás solución fabricada; math-validator adversarial sobre la matemática (dominancia, diversidad, convergencia) | Diseño + verificación matemática adversarial | PENDIENTE |
| HP-03 | **APPLY del op_32**: `op_32_multi_objective.rs` + registro en registry + wiring + property tests REALES (no-dominancia del frente, crowding distance correcto, preferencia selecciona del frente, degeneración con 1 objetivo, determinismo con seed, presupuesto de generaciones acotado); tests del registry existente siguen verdes; `cargo check/clippy/fmt -p math-engine` + tests (AppControl 4551 → `--no-run` + exe directo) | Apply + tests | PENDIENTE |
| HP-04 | **Matriz extendida**: edges nuevo op_32 en el grafo canónico (knowledge_graph + capability_matrix) para las estrategias donde aporta (según HP-01, no según el marketing — p.ej. cross-chain MEV-06 con riesgo de bridge, triangular MEV-01-016 con latencia por hop); actualización del enum/registro TS espejo si aplica; diff generado (no hand-edit masivo) | Apply de datos canónicos | PENDIENTE |
| HP-05 | **UI de preferencias multi-objetivo** (diseño conforme FRONTEND-DOCTRINE.md §5/§7: R1 snapshot, delta-streaming, a11y AA, sin estado global nuevo): panel de sliders rentabilidad/riesgo/velocidad + toggle Pareto con pesos normalizados y preview del efecto; DÓNDE vive (página existente de configuración — no página nueva sin censos); persistencia vía el config plane existente (NO localStorage de producción §contrato) | Diseño + apply + tests | PENDIENTE |
| HP-06 | **Plan de infraestructura con proveedores específicos**: documento con 3 tiers cotizados en concreto (Hetzner dedicado actual como baseline medido — p95 764ms, throughput real del embudo, saturación actual; alternativa cloud AWS/GCP con nodos managed; bare-metal low-latency tipo 2x3690X/EPYC con colocation) — costos $/mes reales de HOY, latencias esperadas, y QUÉ cuello de botella real ataca cada inversión (BR-08 latencia p95 medido, no asumido); veredicto honesto sobre la propuesta GPU 4x A100 del SEED (¿el cuello es cómputo o es I/O-orquestación? evidencia primero); CERO compras, CERO deploy — documento de decisión para el operador | Plan con cotizaciones y evidencia | PENDIENTE |
| HP-07 | **Re-anclaje económico honesto**: las proyecciones del SEED ($10K/h, ROI 10.000%) contra el modelo medible del propio operador (break-even 5% aprobación ≈ $1.170/mes con $2.3M — análisis 2026-09-07) y el estado post-BR-00; qué tiene que ser verdad (aprobación ≥5%, latencia, uptime) para CADA tramo de la escalera; riesgos (competencia, gas, adversarial) del análisis del operador; verdict: la escalera es achievable/unachievable y POR QUÉ con números | Análisis con números verificables | PENDIENTE |
| HP-08 | **Verificación por capas + browser** de todo lo aplicado (HP-03/04/05): verify adversarial por WO, cross-exam de sincronía, y browser-verify en el dominio vivo post-deploy del PR del orquestador (ORDEN SAGRADO) | Verify + cross + browser | PENDIENTE |
| HP-09 | **Censo de estructuras de datos core (SEED-v2 Fase 2)**: qué YA existe en searcher-rs (`route_discovery/`, grafo de liquidez, watchlist N=22→C(22,2)=231 pares≈235 pools vivos, reserves por bloque) vs lo propuesto — TokenRegistry con dense IDs, PairBuckets con índice triangular `i*(2N-i-1)/2+(j-i-1)`, DirectedEdge multigrafo (múltiples pools por par, fee_bps, amount_buckets), adjacency bitsets u64 (N≤64), dirty-pair tracking atómico (AtomicDirtySet), best_bid/best_ask por par. Entregable: tabla brecha-real vs ya-existente con anclas file:line — el apply SOLO de los gaps que el censo pruebe ausentes (P-∅: prohibido duplicar lo vivo); SLA <30ms por-stage (29ms) como meta medible contra p95 764ms de BR-08 | Censo read-only + diseño de gaps | PENDIENTE |
| HP-10 | **DISEÑO SHADOW del terminus (SEED-v2 Fase 5 — §34.3 INTOCABLE)**: documento de diseño (CERO código de broadcast, capital expuesto = 0) de (a) selector de flash providers con fees ON-CHAIN leídas (Aave/Balancer; dYdX Solo deprecado 2023 — documentar como legacy), (b) MevBoostSubmitter con simulate-before-send — mapeado al relays-client EXISTENTE (default-deny `ARBX_LIVE_EXEC_ENABLED != "true"`, MainnetRefused ×6) y al relay catalog D-REL-01 (0 filas hoy), (c) OnlineLearner (GBM no existe nativo en Rust — linfra/linfa parcial; evaluar contra la calibración Beta-Bernoulli κ=20 de BR-05 que YA está diseñada); todo pasa por `arbx-mev-ethics-gate` + `arbx-simulation-mandatory` | Diseño shadow read-only | PENDIENTE |

## Reglas duras

- **§34.3 INTOCABLE**: FlashbotsExecutor, backrun bundles, sandwich, send_bundle, capital,
  broadcast = **SOLO DISEÑO** (audit/scaffold/shadow/read-only, capital expuesto = 0).
  `arbx-mev-ethics-gate` + `arbx-simulation-mandatory` + default-deny jamás se remueven.
- **RULE 00 / P-∅**: NADA del SEED es hecho hasta verificación HP-01 (los [x] aspiracionales
  se reportan como tales). "Genera el código completo de todas las estrategias" = completar
  los GAPS que el censo encuentre — las 264 ya existen como cartridges; PROHIBIDO duplicar.
- **Hot-path mode-invariant §34.1** · fees on-chain · R8 fail-honest (frente de Pareto
  vacío = None ≠ solución cero).
- Diffs `// HP-XX (2026-09-08)` · NO-GIT (PRs del orquestador al final) · VPS read-only ·
  serial-group "rust" (HP-03 es el único builder Rust simultáneo) · verify adversarial por WO.
- **DIRECTIVA OPERADOR (permanente)**: máximo 15 agentes PhD confiables — ecc:rust-reviewer,
  ecc:typescript-reviewer, ecc:security-reviewer, ecc:database-reviewer,
  ecc:performance-optimizer, ecc:react-reviewer, math-validator, cs-validator.
- **Sinergia de mesa**: HP-01 citará a BR-00-APPLY (gate estructural por stem), HP-05 a
  FRONTEND-DOCTRINE.md (§5 BR-11 + §7 contrato), HP-06 a BR-08 (latencia p95 medida) y al
  SEED-V2 (GPU/SIMD/lock-free — `#[cuda_kernel]` NO es atributo Rust real: cust/wgpu son
  las vías; evaluar contra el cuello medido), HP-07 al SEED-V2 (metas win rate >70%,
  Sharpe >3.0, 10K opps/s), HP-09 a BR-02 (114/235 pools con reserves) y BR-08 (29ms
  target), HP-10 a §34.3 + D-REL-01 + BR-05.
