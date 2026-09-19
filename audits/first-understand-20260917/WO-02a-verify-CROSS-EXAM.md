# CROSS-EXAMINATION de WO-02a-verify — 2026-09-17

> Rol: cross-examiner par del agente que ejecutó WO-02a-verify. Mandato: REFUTAR.
> Método: Read/Grep/Bash-readonly SOLAMENTE (restricción board cumplida). CERO git,
> CERO cargo, CERO VPS, 0/5 requests HTTP. Ground truth recomputado con find/wc.

## 1. Lo que SOBREVIVE (la verificación es real, no de humo)

Re-abrí de forma independiente ~20 de las ~30 citas del verificador. Todas calzan:

- main.rs:259/267/275 gates legacy `unwrap_or(false)` — EXACTO (leído directo).
- main.rs:303-317 OMEGA SEAL "spawns no executor... execution_worker.rs is a
  never-spawned stub" — EXACTO.
- route_discovery/mod.rs:54-60 `enum RouteDiscoveryMode {Off, Shadow}`; parse
  `"active"→Off` :72-77; doc "no Active variant" :20-23 — EXACTO. El hallazgo más
  fuerte del reporte es real.
- cartridge_boot.rs:964 `active_evaluate_and_emit`; :972
  `math_registry: Arc<math_engine::OperatorRegistry>`; semáforo try_acquire
  drop-not-queue :989-1001 — EXACTO.
- math_map.json:2-16 MEV-01-001 → primary_ops [op_27,op_21,op_15,op_16], mode
  SHADOW — EXACTO.
- canonical_knobs.rs:71 `min_net_bps` (QUOTEBASE-264); size_optimizer.rs:85
  `GasFloorBreach` (`net_usd < gas_usd × kelly_gas_safety_multiplier`) — EXACTO.
- workers/mod.rs spawns reales :147-149 (RpcHealth), :163-167 (GasOracle),
  :182+ (PoolSync) — confirma la Corrección B del verificador.
- orchestrator.rs:261 `spawn_cartridge_eval` + gate `!= Active → return`
  :267-269; :292 `on_route_intent`; chain_id mismatch reject :300-307 — EXACTO.
- impact_index.rs:12-30 (5 fuentes, lending VACÍO Phase 11 R8) e invariantes
  :25-30 — EXACTO.
- LOC: execution_worker 41, hft_mempool_listener 27, jit_v3_worker 284
  (grep spawn: solo `pub mod` mod.rs:14 — CERO call-site), main.rs 1277,
  cartridge_boot 2465, route_intent 514, impact_index 1433 — TODO EXACTO (wc -l).
- D-1..D-3 NO aplicados: grep `WO-02a` en searcher-rs/src = **0 hits**;
  `RouterKind` SIN variante PancakeV3; :275 sigue `PancakeV3 => Unknown` —
  el único diff working-tree es el PREEXISTE del operador (dcfe890c). Cero
  mutación de código por el diseñador o el verificador. CUMPLIDO.
- Corrección A del verificador: recomputada — crate no-worktree = 742 archivos /
  223,273 LOC; src no-worktree = 184 entradas; .rhai = 271. CONFIRMO la
  contaminación por worktrees que el verificador denunció.

## 2. REFUTACIONES (gaps encontrados)

### R1 (MATERIAL) — El PASS de RULE 00/objetivo_usd se otorgó sobre un grep incompleto y con una premisa falsa

WO-02a-verify §3 afirma: *"los únicos min_profit_usd con valores viven en
src/engines/* y TODO ese árbol está tras #[cfg(feature = \"experimental-engines\")]
(engines/mod.rs:38-51)"*. FALSO en dos cuentas:

1. **"TODO ese árbol está gated" es incorrecto**: engines/mod.rs:25-28 compila
   `dex_engine`, `flashloan_engine`, `liquidation_engine`, `triangular_engine`
   SIN gate — solo los 8 motores de :38-52 son experimentales. (El valor
   `min_profit_usd: 0.01` de triangular_engine.rs:871 SÍ es test — está dentro
   de `#[cfg(test)] mod tests` :843-845 — así que la conclusión de valores
   sobrevive, pero la premisa "todo gated" no.)
2. **El hot-path de producción del propio searcher usa un piso USD absoluto**:
   scanner.rs:2568 `min_profit_threshold: cfg.min_profit_usd` dentro de
   `decode_and_score_tx` (scanner.rs:1488, sin feature-gate; default build) —
   alimenta `PrioritizationEngine`. Es decir, WO-02a §9 *"el sistema opera con
   umbrales RELATIVOS (bps y múltiplos de gas), no con un blanco USD absoluto"*
   está SOBRE-ENUNCIADO como conclusión de board: existe un piso USD absoluto
   operador-configurable (PG `trading_config`) en el camino canónico del scanner.
3. **El patrón de grep `target_usd` no captura `target_profit_usd`**: el tipo
   canónico `TradingConfigState` (shared-rs/src/trading_config.rs) define
   `simulation_target_profit_usd: Option<f64>` (:249, doc :242-249: "Simulation
   UI filter — minimum profit... backend STORES but does NOT gate, R8
   fail-honest"), `simulation_target_roi_pct` (:254), `simulation_capital_usd`
   (:166), `min_profit_usd: f64` (:260, gate real) y
   `effective_min_profit_usd(strategy)` (:622, override por estrategia).
   El GAP "no hay objetivo_usd POR ESTRATEGIA en searcher-rs" sobrevive en lo
   angosto, pero la afirmación de inexistencia de blancos USD del §9 debe
   corregirse: los blancos del operador existen canónicamente en shared-rs y se
   consumen en el terminus y en el scanner.

Clasificación de mi evidencia: CANONICAL_REPO (file:line citados, recomputables).

### R2 (MATERIAL) — Fallo de sincronía de mesa redonda: 02d SÍ toca el claim y lo contradice

WO-02a-verify §0 descartó a 02d: *"no toca mi claim salvo el acople terminus; sin
contradicciones con WO-02a"*. FALSO:
- 02d-RELAYS-CLIENT-SHARED.md:212 etiqueta `trading_config.capital_usd`
  (trading_config.rs:183) como **"fuente REAL de objetivo_usd runtime"**.
- 02d §5 (líneas 245-258) documenta TRES fuentes USD runtime: `capital_usd`
  (cap 2% por oportunidad, execution_admission.rs:193-201),
  `max_value_eth` default 1.0 ETH (config.rs:156-158) y el piso
  `min_profit_usd` (checklist 4/7). Todo esto cualifica directamente el §9 de
  WO-02a que el verificador PASÓ.
- 02b (:148, G1) y 02c (§7, :209-211, :314) también trataron objetivo_usd a las
  01:06 — un minuto antes del verify (01:07) — y no fueron citados ni
  contrastados. Mínimo exigible: nota de concurrencia o lectura post-escritura.

### R3 (MENOR) — La Corrección A del verificador contiene su propio desliz aritmético

Verify dice "src .rs = **178 archivos, ~123.7K LOC**". Recomputado:
- 123,737 = LOC de las **184 entradas** src no-worktree (INCLUYE ~33.3K LOC de
  fixtures JSON: dirty_trace.fixture.json 22,929, etc.).
- Real .rs no-worktree: **178 archivos / 90,395 LOC** (ground truth `find src
  -name "*.rs" | wc` = 178 archivos / **90,401** líneas; delta 6 = conteo de
  últimas líneas sin newline).
El corrector que denunció números contaminados introdujo un número mal
atribuido. Irónico pero menor: la conclusión (charter 193 ≠ real) sigue en pie.

### R4 (MENOR, proceso) — Requisito de muestra no trazable

Verify reclama "15 fichas (≥8 exigidos)". Ni GOAL-WORKORDERS.md ni charter
visible fijan mínimo de 8. Afirmación de requisito sin fuente — anotar como
práctica a evitar (no afecta la sustancia: más muestras = mejor).

## 3. Reglas duras — compliance

- RULE 00: sin fabricación detectada en fichas; PERO el dictamen RULE 00 §3
  queda condicionado por R1/R2 (veredicto parcialmente refutado, no por
  invención sino por grep incompleto + premisa falsa + omisión de pares).
- §32/§33 (read-only): CUMPLIDO (0 git, 0 VPS, 0 HTTP, sin marcadores en src/).
- NO-GIT: CUMPLIDO (git status solo muestra diffs PREEXISTES del operador).
- Fail-open Sancho declarado honestamente — aceptado por política.

## 4. Veredicto del cross-examiner

**VERIFICACIÓN SUSTANTIVA = REAL** (0 contenido falso en ~20 citas re-abiertas;
los hallazgos estructurales del WO-02a reproducen). **PERO el dictamen RULE 00
(§3) y la sincronía de mesa (§0) quedan REFUTADOS EN PARTE** → requiere adendum
correctivo antes de que los gates 1-8 consuman la conclusión "umbral relativo
sin blanco USD".

### Gaps

| # | Gap | Clasificación | Fix |
|---|---|---|---|
| G1 | §9/§3 de WO-02a+verify: corregir "umbrales solo relativos" — reconocer piso USD absoluto `min_profit_usd` en hot-path (scanner.rs:2568, PrioritizationEngine) + blancos canónicos `simulation_target_profit_usd`/`capital_usd` (shared-rs trading_config.rs:166,249,260,622; 02d §5) | agent-fixable | Adendum append-only en WO-02a-DESIGN.md y WO-02a-verify-VERIFY.md citando a 02d:212/§5 y 02b:148; NO tocar el GAP por-estrategia (sigue válido) |
| G2 | Premisa falsa "TODO engines/ tras experimental-gate" (verify §3) — mod.rs:25-28 compila 4 engines estables sin gate | agent-fixable | Corrección de una línea en el adendum |
| G3 | Desliz aritmético Corrección A: "~123.7K LOC .rs" → real 178 .rs / ~90.4K LOC (123.7K = 184 entradas con fixtures) | agent-fixable | Corrección numérica en el adendum |
| G4 | Hot-path central sin ficha formal (size_optimizer 3,625 LOC — el motor económico; scanner 3,372; opportunity_emitter 1,093; engines/ estables 4) | operator-gated | El orquestador decide si abre WO de seguimiento (verify ya lo listó honestamente en §5) |
| G5 | 01-INVENTARIO.md (evidencia WO-01) sigue ausente del dir | operator-gated | Regeneración por el orquestador (script bash, 0 LLM) |

## 5. Preguntas para el operador

1. ¿Aprueba el adendum correctivo G1-G3 (append-only, sin borrar trabajo ajeno)
   antes de que WO-06/gates 1-8 consuman la conclusión de umbrales?
2. ¿Abre WO de seguimiento para fichar size_optimizer/scanner/emitter/engines
   estables (los ~10 módulos omitidos con presencia real de hot-path)?
3. ¿Regenera 01-INVENTARIO.md para que WO-01 vuelva a tener evidencia auditable?
