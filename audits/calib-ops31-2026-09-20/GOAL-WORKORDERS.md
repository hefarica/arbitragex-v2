# GOAL BOARD — CALIB-OPS31-2026-09-20

## /goal (orden del operador, verbatim 2026-09-20)
> "si no están integrados ni calibrados los 31 ops a las 264 estrategias en el pipeline para arbitrage. Hazlo"

## Estado verificado al abrir (datos reales, no afirmaciones)
| Eslabón | Estado | Evidencia |
|---|---|---|
| Integración (evidencia por estrategia) | ✅ VIVO | 59,638 rows con `evidence_vector` en `scored_opportunities`; 3,732 keys Redis `arbx:math_evidence:*` |
| Labels Y (drift-tracker Stage 2a) | ❌ DORMANTE | `labels=0`; `ARBX_DRIFT_TRACKER_MODE` ausente de `.env` VPS |
| Calibración (Stage 2b writer) | ❌ DORMANTE | `math_operator_calibration=0 rows`; `ARBX_STAGE2_CALIBRATION_MODE` ausente |
| Infra sim (requisito del label-writer) | ✅ VIVA | `arbitragex-v2-sim-ctl-1` + `arbitragex-v2-anvil-1` Up/healthy; `SIM_BACKEND=anvil` |
| Consumidor §IV (searcher-rs) | ✅ LISTO | `priors_cache.rs` spawn con refresh periódico (`ARBX_PRIORS_REFRESH_SECS`) — SIN restart necesario |
| Causa raíz única | **Los 2 flags OFF** | `recon/src/main.rs:342,372` — default dormante por diseño (WO-07 port-back BR-05) |

## Matemática del calibrador (recon/src/stage2_calibration.rs — ya auditada BR-05+06-VERIFY)
- Shrinkage jerárquico: `θ_k = (κ·θ₀ + wins_k)/(κ + n_k)`, `log_lr_k = logit(θ_k) − logit(θ₀)`, κ=20 default.
- Recompute-from-source idempotente; watermark `calibrated_at`; Θ clamp ±9.21.
- Honesty: label SOLO con `actual_profit_usd IS NOT NULL`; ECONOMIC/MARKET reject = Y:0 exacto (etiqueta VÁLIDA — los perdedores enseñan tanto como los ganadores); STRUCTURAL = ineligible.
- Trigger: cada 100 labels nuevos (`ARBX_CALIBRATION_CONSOLIDATE_EVERY`).

## WORK ORDERS

### WO-C1 — Activación de flags en VPS [OWNER: -61] [GATE: contenedor recon healthy con ambos jobs spawned]
- [ ] Backup `.env` → append `ARBX_DRIFT_TRACKER_MODE=on` + `ARBX_STAGE2_CALIBRATION_MODE=on`
- [ ] `docker compose --env-file .env -f docker/compose.dev.yml up -d recon` (recreate con nuevo env; SIN build — env runtime, no baked)
- [ ] Verificar logs: `drift_tracker.spawned` + `stage2_calibration.spawned` (o `.dormant` = flip falló)

### WO-C2 — Labels fluyen [OWNER: -61] [GATE: `SELECT COUNT(*) FROM paper_trade_runs WHERE actual_timestamp IS NOT NULL` > 0 y creciendo]
- [ ] Dentro de ~5 min: primeras resoluciones (batch=20/30s; backlog 598K pending alimenta solo lo elegible)
- [ ] Clasificar: Resolved vs NotPassed(Y=0) vs Structural(ineligible) vs Pending — histograma honesto

### WO-C3 — Calibración materializada [OWNER: -61] [GATE: `math_operator_calibration` > 0 rows con log_lr finitos]
- [ ] Tras ≥100 labels nuevos: `stage2_calibration.consolidated` en logs
- [ ] Verificar 31 rows, sample_count>0 en ops activos, log_lr=0 en ops sin data (honesto)

### WO-C4 — Fold §IV aplicado en searcher [OWNER: -61] [GATE: `calibration_applied=true` observable]
- [ ] `priors_cache` refresh picks up calibration (sin restart — polling)
- [ ] Verificar en emisiones/searcher logs que section_iv_fold corre con calibration Some

### WO-C5 — Validación matemática independiente [OWNER: gang Hermes] [GATE: vector externo reproducido]
- [ ] Recalcular log-LRs a mano para (n, wins, θ₀, κ) dados — vectores INDEPENDIENTES (doctrina 2026-09-17: jamás test que re-compute la propia fórmula)
- [ ] Verificar clamps, simetría logit, casos n=0

### WO-C6 — Medición de efecto [OWNER: gang Hermes + -61] [GATE: delta pre/post medido]
- [ ] Precision@k proxy del ranking pre vs post calibración (la pregunta del operador "cuánto mejoran los 31 ops" se responde AQUÍ con el delta real)
- [ ] Honestidad: si θ₀≈0 (todo rejected), log-LRs≈0 y el delta será pequeño — reportar tal cual

### WO-C7 — Merge queue paralela [OWNER: -61] [STATUS: monitor #606 armado; cola #607→#608→#609→#610→#611]

## Riesgos anotados
1. **Backlog 598K pending**: drift-tracker procesa batch=20/30s — si las rows antiguas son elegibles (sim_block_number forkable), puede generar carga de sim. Vigilar latency sim-ctl + disco. Mitigación: si carga alta, bajar batch o archivar pending antiguo (decisión documentada).
2. **Todo-rejected ⇒ θ₀ clamp**: si 100% Y=0, θ₀ clamp a 1e-4 y log-LRs casi nulos → calibración honesta pero sin señal. NO es fallo: es el estado real del sistema. El operador ya sabe (funnel 09-17).
3. **anvil fork de bloques viejos**: rows antiguas pueden caer en Pending/Structural — el ladder S4-03 lo maneja fail-honest.
4. **Flip = acción sobre prod VPS**: autorizado por orden explícita del operador ("Hazlo"), paper-path, capital expuesto = 0, reversible (quitar flags + up -d).

## Registro de autorización
- Orden del operador 2026-09-20 (verbatim arriba) = autorización explícita para este flip específico (calibración paper-path). NO es flip live/mainnet (§34 intacto). Reversible en 1 comando.

## ESTADO AL CORTE 09:05Z — CONT.1 (verificación con datos reales)

### WO-C1 ✅ PASS — Activación completa
- Flags `ARBX_DRIFT_TRACKER_MODE=on` + `ARBX_STAGE2_CALIBRATION_MODE=on` en .env VPS (backup `.env.bak-calib-ops31-20260920`).
- Recon recreate OK; logs VERIFICADOS: `drift_tracker.spawned` (interval 30s, max_attempts 10) + `stage2_calibration.spawned` (interval 60s, consolidate_every=100, κ=20.0).
- Reversible: quitar flags + `up -d recon`.

### WO-C2 ⏸️ ARMADO pero SIN COMBUSTIBLE — causa raíz del upstream (datos)
- `paper_trade_runs` = 0 rows (truncada por orden "sin almacenamiento hasta E2E" 09-19; solo ACCEPTED persisten ahora).
- Paper executor solo persiste sims `passed` + net_yield>0 → stream `arbx:hot:simulated` XLEN=0, group lag=0, NUNCA entregó.
- Heartbeat searcher: `passed_all_gates: 0`/min → nada llega al simulador.
- **Distribución rejection_reason 15m (PG, real)**: non_positive_profit 55,992 (65%) · v3_quote_unavailable 14,113 (16%) · spot_product_le_one 5,228 · gas_floor_breach 2,968 · spread_zero_equilibrium 2,548 · single_pool_no_spread 1,899 · NegativeNetProfit 280.
- Lectura honesta: 65%+ = SIN EDGE REAL al estado actual de pools (economía, no defecto). v3_quote_unavailable = defecto conocido reparables (anomalía abierta 09-17: 93 claves sin reparar + 2do path de quote sin instrumentar).

### Integración §IV (el otro half del /goal) ✅ VIVA Y MASIVA
- `math_evidence.evaluated` × 273/15m; `scored_opportunities` +86,394/15m con evidence_vector.

### Hallazgos laterales (nuevos, para BOARD perf-stack)
1. **Schema drift**: `opportunity_emitter.db_error` "insert opportunity" — PG log: `column "status" does not exist at character 8` (22 err/15m intermitente; publish a Redis sigue → fail-soft). searcher espera columna status que la tabla desplegada no tiene.
2. `simulator.boot`: "hot path stays fail-closed (reason=encoder_not_ready) until Phase A.3 encoder lands" — mensaje de boot posiblemente stale (sim_encoder.boot presente); requiere verificación si Phase A.3 ya landed.

### Veredicto del /goal al corte
- INTEGRACIÓN: completa y fluyendo (evidencia por estrategia en cada scored row).
- CALIBRACIÓN: cadena activada extremo a extremo; consolidará automáticamente al llegar los primeros 100 labels. Los labels esperan el primer gate-pass→sim→paper-run de la historia reciente. NO se fabrican (RULE 00).
- Desbloqueo máximo disponible SIN romper honestidad: cerrar v3_quote_unavailable (work-stream separado, anomalía abierta).

