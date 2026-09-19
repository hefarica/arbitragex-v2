# CROSS-EXAMINATION de WO-02c (02c-SIM-STACK-SELECTOR + verify) — 2026-09-17

> Rol: cross-examiner par del agente que ejecutó WO-02c (ecc:rust-reviewer, designer)
> y de su verify (cs-validator). Mandato: REFUTAR el entregable.
> Método: Read/Grep/Bash-readonly SOLAMENTE. CERO cargo/npm/build (restricción board
> GOAL-WORKORDERS.md:17-19), CERO git, CERO VPS, 0/5 requests HTTP del presupuesto.
> Lexicon OMEGA respetado. Pares leídos ANTES de opinar: GOAL-WORKORDERS.md completo,
> 02-VPS-REMAP-20260917.md, WO-02a-verify-CROSS-EXAM.md (la refutación más reciente
> de la mesa), 02d-RELAYS-CLIENT-SHARED.md §5 vía su cita en el cross-exam de 02a.

## 1. Lo que SOBREVIVE (la verificación es REAL, no de humo)

Re-abrí de forma independiente ~25 marcadores file:line del designer y del verify.
Todos calzan:

- **SEL-GATE-01 (misión explícita del charter)**: `producerRejected`
  (selector-api/src/policy/engine.ts:36-39, leído directo) + wire en `prefilter`
  ANTES del trabajo caro (:56-58) + publish accept-only (consumer.ts:173-175,
  `if (decision.kind === "accept") publishValidated`) + test de regresión
  sel-gate01.test.ts con candado de orden leyendo el fuente de consumer.ts
  (test :89-100 verificado línea a línea: `preIdx < safetyIdx`, `preIdx < scoreIdx`).
  El veredicto consolidado §6 (local presente / VPS a06a968d sin deploy) reproduce
  contra 02-VPS-REMAP-20260917.md:4-5. CONFIRMADO.
- **Runtime order** del flujo: consumer.ts:23-25 (`detected`→`validated`,
  selector-g0) y sim-ctl consumer.rs:48-50 (`validated`→`simulated`, sim-ctl-g0),
  MAXLEN 10_000 ambos. CONFIRMADO.
- **Gates 2-3 latentes**: `decide({..., sim: null, ...})` en consumer.ts:213
  (leído directo). CONFIRMADO.
- **Invariante de aislamiento (C1 del verify, exacta)**:
  `MultiStepExecutionConfig::validate()` rechaza tipado `PaperModeRequired` /
  `StorageCheatsDisabled` (sim_multistep.rs:250-257, leído directo) PERO
  `execute_multistep_revm` con `paper_mode=false` NO retorna error — delega a
  `verified_simulation::execute` (:554-556, leído directo). sim_runner.rs fija
  `paper_mode: true, enable_storage_cheats: true` (:204-205, leído directo).
  §32/§33/§34.3 INTACTO. La corrección C1 del verify es correcta y necesaria.
- **HUEHUFOs**: `bellman_ford` — grep backend completo: solo doc-comment
  (simulator-v2/src/lib.rs:13,20) y SU anuncio en `capabilities()` (lib.rs:61);
  searcher usa su propio `spanning_tree_engine.rs:291 bellman_ford_cycles` y
  math-engine su propio `route_math.rs:25`. `bayesian.ts` — único import =
  bayesian.test.ts:2. AMBOS CONFIRMADOS (y la nota del verify sobre el anuncio
  en /capabilities también reproduce).
- **Idempotencia revm**: ON CONFLICT (opportunity_id) WHERE simulator='revm'
  DO NOTHING (persistence.rs, leído directo) + XADD solo si
  `sim.passed && inserted_fresh` (consumer.rs, leído directo). CONFIRMADO
  — pero ver G3: el claim está SOBRE-GENERALIZADO.
- **Backend selection**: main.rs:661-692 `SIM_BACKEND` default "anvil", revm
  fail-fast REDIS_URL, valor desconocido → bail. CONFIRMADO. `.env.example`
  :319 area documenta el flip P1-3 como OPERATOR-ONLY §34.3. CONFIRMADO.
- **C2 del verify es CORRETA y la confirmo con evidencia propia**: "87%" y
  "funnel 2026-09-17T04:00Z" NO existen en NINGÚN archivo de
  audits/real-cycles-audit-20260916/ (grep: 0 hits; el archivo contiene
  "74/9h" S4, "76 opps/9h" detección, "100% rejected", "v3_quote_unavailable
  60/76"). El número vive SOLO en sel-gate01.test.ts:5-9 y en la ficha 02c
  (circular). Violación del estándar de evidencia §34.5.3 en el ENTREGABLE.
- **§7 objetivo_usd GAP SOBREVIVE la tensión con 02d** (punto donde el cross-exam
  de 02a REFUTÓ a su par): grep propio en las 4 piezas por
  `trading_config|target_profit|min_profit_usd|capital_usd|objetivo` = **0 hits**.
  Los blancos USD que 02d fichó (capital_usd, min_profit_usd, simulation_target_profit_usd
  — shared-rs/trading_config.rs) se consumen en el TERMINUS
  (relays-client pre_execute_checklist.rs:222-235,333) NO en el stack sim/selector.
  El scoping de 02c ("GAP en las 4 piezas") fue más preciso que el de 02a. CONFIRMADO.
- **Compliance reglas duras**: git status muestra solo diffs PREEXISTES del
  operador (route_intent.rs + .claude/.mcp/CLAUDE.md); audits/ es untracked que
  contiene los reportes. Cero marcadores WO-02c en src/. 0 ejecución prohibida
  (no puedo probar negativo absoluto, pero no hay rastro de cargo/npm/HTTP/VPS).
  CUMPLIDO.
- **Sincronía de mesa**: 02c citó 02-VPS-REMAP (dato crítico deploy), difirió
  shared-rs a 02d sin duplicar (verificado: 02d fichó los 33 shared-rs), handed
  off a 02a/02b con puntos de acople concretos. El fallo de sincronía que el
  cross-exam de 02a denunció (R2) fue de WO-02a-verify contra 02c/02b/02d —
  NO de 02c contra sus pares. CUMPLIDO de su lado.

## 2. REFUTACIONES (gaps encontrados)

### G1 (MATERIAL) — La corrección C2 fue DECLARADA pero JAMÁS APLICADA al entregable

El verify dictaminó (§10.4) que la cifra "87% de sims quemadas en rows ya
rechazadas" no es reproducible y ordenó "re-bajar la cifra en test header +
este reporte". Resultado: la ficha 02c-SIM-STACK-SELECTOR.md §5.3 (línea 171)
SIGUE afirmando "87% de sims quemadas... citado en el test header :5-9" como
evidencia del incidente — una cita CIRCULAR (el test header es la única fuente
del número, y él mismo cita un funnel que no existe). El adendum §10 documenta
el problema pero nadie corrigió el texto que la mesa y WO-06/gates van a
consumir. El archivo de auditoría NO es código de producción ni está bajo
NO-GIT (audits/ es untracked): es agent-fixable inmediato. Solo la edición del
header del test (código) es operator-gated.

### G2 (MATERIAL) — route_lookup.rs sigue SIN ficha y el WO quedó DONE

El propio verify calificó la omisión de route_lookup.rs (232 LOC, núcleo del
enriquecimiento A3) como "OMISIÓN REAL, la de mayor peso". El gate del board
(GOAL-WORKORDERS.md:16) exige "cada ficha con ruta+firma+prueba (línea exacta)
> o marcada GAP". Ni ficha ni marca GAP existe; WO-02c está ✅ DONE en el board.
Ídem los 2 hallazgos nuevos del verify (doc-drift route_lookup.rs:15;
observación `unpinned_replay` por block_number NULL) viven solo en el adendum,
fuera de la matriz de destinos §8. El ciclo designer→verify→corrección se
cortó en "declarar": la mesa hereda fichas incompletas con sello VERIFICADA.

### G3 (MATERIAL, hallazgo NUEVO mío) — Hard finding 4 (idempotencia) está sobre-generalizado: es revm-scoped POR DISEÑO

02c-DESIGN hallazgo 4 enuncia: "Idempotencia de redelivery: insert_simulation
ON CONFLICT DO NOTHING + skip de XADD duplicado" — incondicional. Evidencia
propia:
- El índice árbitro es PARCIAL: `CREATE UNIQUE INDEX simulations_revm_idempotency_uq
  ON simulations(opportunity_id) WHERE simulator = 'revm'`
  (database/migrations/113_simulations_revm_idempotency.sql:27-29, comment:
  "PARTIAL (WHERE simulator = 'revm') on purpose: legacy anvil history").
- El path legacy anvil produce `SimulatorKind::Anvil` (sim_engine.rs:163,205) →
  para esas filas el ON CONFLICT nunca dispara → `inserted_fresh` SIEMPRE true →
  un PEL-redelivery de una sim anvil passed **re-publicaría duplicado a
  arbx:opps:simulated** (el comentario del propio consumer.rs reconoce que el
  skip existe para no "double-count the opportunity downstream").
- Y 02c MISMO dice (§7/ficha 4.6) que el default en compose es SIM_BACKEND=anvil:
  el branch sin dedup es el que el reporte decla como path por defecto.
- Mig 004 documenta el diseño original: "One simulation per attempt; an
  opportunity may have multiple (e.g., retried with tweaked params)" — la
  dedup es una invariante de la era revm, no del stack completo.
Impacto práctico HOY = nulo (00-SYNTHESIS: passed=true 0 en toda la historia),
pero la ficha se presenta como verdad canónica de wiring. Corrección: una
frase "válido solo para simulator='revm' (B2c); el path anvil legacy no deduplica".
Clasificación: CANONICAL_REPO (migrations 004/112/113 + sim_engine.rs citados).

### G4 (MENOR) — El tally de completitud del verify es off-by-one

Verify §10.5: "selector-api: COMPLETO (12/12)". Ground truth:
`find selector-api/src -name "*.ts" ! -name "*.test.ts" | wc -l` = **13**
(consumer, index, persistence, score, policy×2, scoring×3, token_safety×4).
Irónico frente al estándar de evidencia que el propio verify invocó en C2.
La conclusión (cobertura completa) sobrevive; el conteo no.

### G5 (MENOR, clasificación de evidencia) — "default anvil" presentado como verdad runtime

Ficha 4.6/§7: "es el default SIM_BACKEND=anvil en compose — docker/compose.prod.yml
NO define SIM_BACKEND → default anvil". Cierto PARA COMPOSE (leído: el bloque
sim-ctl solo pasa ARBX_CONFIG_PATH/SIM_PORT/DATABASE_URL/REDIS_URL/ANVIL_URL),
PERO ese mismo bloque monta `env_file: ../.env` — si el .env del VPS define
SIM_BACKEND=revm, el default se sobreescribe y TODO el análisis de "cuál branch
corre en runtime" cambia (B2c carrier-first incluido: B2cCtx exige
SIM_BACKEND=revm, main.rs:718-731). El valor efectivo del VPS es UNKNOWN desde
evidencia local (02-VPS-REMAP no leyó el contenido del .env). Debió clasificarse
INFERRED/UNKNOWN para runtime, CANONICAL_REPO solo para compose.

## 3. Reglas duras — compliance del WO-02c

- RULE 00 / R8: fichas sin fabricación detectada (0 de ~25 marcadores falsos);
  PERO la cifra "87%" en §5.3 del entregable es una estadística sin artefacto
  reproducible citada como evidencia — exactamente la clase de afirmación que
  §34.5.3 (precedente 2026-09-15, issue #567) prohíbe. Condición G1.
- §32/§33 (read-only, sin executor/wallets/capital): CUMPLIDO — el stack sim se
  fichó sin tocar nada; la invariante paper_mode+cheats verificada de primera mano.
- §34.3: CUMPLIDO — el flip SIM_BACKEND=revm y DEPLOY-DEBT-01 quedaron como
  operator-gated; default-deny del terminus intocado (ficha diferida a 02d).
- NO-GIT: CUMPLIDO.
- Restricción extra del run (0 cargo/npm/build): CUMPLIDO según lo observable.

## 4. Veredicto del cross-examiner

**VERIFICACIÓN SUSTANTIVA = REAL** (las fichas reproducen; el verify re-abrió
de verdad; SEL-GATE-01/HUEHUFOs/objetivo_usd-GAP sobreviven). **PERO el ciclo
de calidad quedó INCOMPLETO**: la corrección C2 no se aplicó al entregable (G1),
la omisión admitida más pesada no se cerró antes del DONE (G2), y el hard
finding 4 está sobre-generalizado con un contraejemplo en el propio árbol de
migraciones (G3). El board NO debería consumir §5.3 ni el hallazgo 4 sin los
matices.

### Gaps

| # | Gap | Clasificación | Fix |
|---|---|---|---|
| G1 | Re-bajar la cifra "87%" en 02c-SIM-STACK-SELECTOR.md §5.3/§6 a "fracción mayoritaria no cuantificada" + nota de circularidad (la edición del header de sel-gate01.test.ts SÍ es operator-gated) | agent-fixable | Append-only/corrección quirúrgica en el .md del audit (no es código, no es git-gated); citar C2 §10.4 |
| G2 | Escribir la ficha de sim-ctl/src/route_lookup.rs (ruta/firma/Err-vs-None/merge_decimals) + subir a la matriz §8 los 2 hallazgos del verify (doc-drift :15, unpinned_replay) | agent-fixable | Nueva sección en 02c o ficha anexa; cita route_lookup.rs:15,108,116-127 |
| G3 | Cualificar hard finding 4: idempotencia de redelivery válida SOLO para simulator='revm' (migrations/113:27-29 parcial a propósito); path anvil legacy sin dedup → posible double-publish a arbx:opps:simulated bajo PEL-redelivery | agent-fixable | Frase de scope en 02c-DESIGN hallazgo 4 + ficha 4.5; el fix de código (si se quiere extender dedup a anvil) = WO propio operator-gated |
| G4 | Corregir tally verify "12/12" → 13 archivos no-test en selector-api/src | agent-fixable | Nota en el adendum |
| G5 | Reclasificar "default anvil" como INFERRED/UNKNOWN para runtime VPS (env_file ../.env puede override) | agent-fixable | Nota de clasificación en §7/ficha 4.6 |
| G6 | DEPLOY-DEBT-01: deploy de fdb40401+125b1e0b+dcfe890c (SEL-GATE-01/SIM-FUND-01/PANCAKE-ROUTER-01 siguen siendo código muerto en runtime) — incluye limpiar el stale /tmp/arbx-deploy.lock antes (02-VPS-REMAP hallazgo 4) | operator-gated | Acción VPS del operador con gate post-deploy `git rev-parse HEAD` == SHA |
| G7 | Diseños WIRE-BAYES-01 / DEPRECATE-BF-01 / SEL-GATE-01bis: abrir WO con diff exacto | operator-gated | Decisión de apertura de WO |

## 5. Preguntas para el operador

1. ¿Aprueba los fixes agent-fixable G1-G5 (ediciones append-only/quirúrgicas en
   los .md del audit, sin tocar código) antes de que WO-06/gates 1-8 consuman
   las conclusiones de 02c?
2. ¿Autoriza la edición del header de sel-gate01.test.ts para re-bajar el "87%"
   (código de producción → gated, y de archivo el query del embudo que lo
   produjo si existe en alguna sesión)?
3. ¿Ejecuta DEPLOY-DEBT-01 (deploy main local + limpieza del deploy-lock stale),
   dado que SEL-GATE-01/SIM-FUND-01 llevan un día sin efecto runtime?
4. ¿Quiere que el matiz G3 (double-publish potencial del path anvil bajo
   redelivery) entre como observación/wire-audit en un WO, dado que hoy el
   impacto es nulo (passed=0 histórico)?
