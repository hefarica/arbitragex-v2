# WO-07 — VERIFY: auditoría adversarial del design (port-back Stage 2b/2c, PR#472 → main)

- **WO**: WO-07 · kind: **verify** (READ-ONLY — 0 git writes, 0 cargo, 0 mutación VPS, 0/5 requests dominio público)
- **Agente**: math-validator (rubric ecc:database-reviewer, capa migraciones/store) · Gang Omniscience §9 · 2026-09-06
- **Objeto auditado**: `audits/omniscience-integration-2026-09-06/WO-07-DESIGN.md` (aterrizó 19:26 sin peer-review — esta es la revisión adversarial)
- **Refs verificadas**: branch `feat/stage2-calibration-closure` @ `b8600895f817db0a2afc536b08eb95960d6a385e` (= PR#472, NO mergeado) · commits `113145e80e18bf18e702d1d89b417f519f7ab6b6` + `b8600895` · merge-base `20c93917` · **main real = `origin/main` = `9ac06d2d`** (⚠️ el ref local `main` está STALE en `28d48cdd` #531 — toda comparación de este verify usó `origin/main`).

## VEREDICTO: **PASS — NO BLOCK** (0 defectos CRITICAL · 4 MINOR · 2 cosméticos-agrupados · 1 corrección al charter)

El design es estadísticamente correcto, resuelve la colisión 111 por DROP-CONCIENTE (la única resolución válida), el inventario de 8 archivos aplica limpio contra main real, la cadena de blockers P1-3 es honesta (sin overclaim de activación), y los trazadores apuntan a contenido real del branch con UNA errata de blob-hash que corregir antes del apply-WO.

---

## 1. ESTADÍSTICA — derivo la corrección yo mismo

### 1.1 Derivación del estimador (verificada contra blob `4c413ecd`)

**Shrinkage**: θ_k = (κ·θ₀ + wins_k)/(κ + n_k) es EXACTAMENTE la media posterior conjugada Beta-Bernoulli con prior Beta(α, β), α = κ·θ₀, β = κ·(1−θ₀): tras observar wins_k éxitos en n_k trials, posterior = Beta(α + wins_k, β + n_k − wins_k), media = (κθ₀ + wins_k)/(κ + n_k). Correcto. (`stage2_calibration.rs:326-332` blob — código coincide con la fórmula).

**log-LR vs logit**: para el evento binario "operador k disparó" (E_k), la aditividad log-odds de Bayes da logit(P(Y=1|E_k)) − logit(P(Y=1)) = ln[P(E_k|Y=1)/P(E_k|Y=0)] = LLR_k. El blob implementa `log_lr = logit(clamp(θ_k)) − logit(clamp(θ₀))` con θ₀ = base-rate POOLED sobre todos los pares etiquetados (`:261` — correcto: el base rate debe ser sobre todos los eventos, no solo los de disparo) y θ_k = P̂(Y=1 | disparo). Plug-in empírico-Bayes exacto para el fold naive-Bayes que `math_evidence.rs::evidence_posterior_log_odds` (VERIFICADO en main, firma en `:393`, fold `Σ log_lr_k · e_k` con umbral |lr|>1e-12) ya define. Dimensionalmente coherente: logit:[0,1]→ℝ, resta de logits = log de odds-ratio.

**Aritmética del design re-computada por mí**:
- n=3, wins=3, θ₀=0.5, κ=20 → (10+3)/23 = 13/23 = 0.56522 ✓ ("~87% prior": 20/23 = 86.96% ✓); log_lr = ln(1.3) = 0.2624 nats ✓ (design §4 capa 4).
- n=150, wins=150 → 160/170 = 0.94118, 88.24% empírico ✓.
- n=100, wins=80, θ₀=0.5 → 90/120 = 0.75 → log_lr = ln(3) = 1.0986 ✓ (test `better_than_base_gets_positive_log_lr`, blob `:375-380`).

### 1.2 Edge cases (charter)

| Caso | Comportamiento del blob | Veredicto |
|---|---|---|
| **n_k = 0** | Short-circuit `(0.0, 0)` en `operator_log_lr` (`:337-340`); ADEMÁS `shrunk_theta(0,·)` degenera a θ₀ → log_lr=0. Doble-seguro. El upsert persiste log_lr=0/sample_count=0 para los 31 → store completo, motor honesto | ✓ |
| **θ₀ extremo** | `clamp_theta` [1e-4, 1−1e-4] en θ₀ (pooled) y θ_k; logit siempre finito; NaN/Inf labels excluidos (`:227-229`); κ del env filtrado `finite && > 0` (`:106-112`) | ✓ (ver MINOR-1: cota mal enunciada) |
| **wins = n** | θ_k = (κθ₀+n)/(κ+n) < 1 para n finito; clamp garantiza finito. Test `all_wins_clamps_to_finite_log_lr` (n=10⁴, θ₀=0.5 → θ=0.999002, log_lr=6.909 < 25) | ✓ |
| **Aritmética de índices** | arrays [0u64; 32], idx 0..=30 → op_n[1..=31]; `.take(OPERATOR_COUNT)` previene OOB si el JSON trae >31 slots | ✓ |

### 1.3 Backoff anti-feedback-loop (charter ítem 1)

Verifiqué las CINCO capas anti-realimentación, tres en el blob y dos estructurales:

1. **κ=20 = amortiguador estadístico**: un operador no puede auto-amplificarse desde pocos eventos (3 eventos mueven θ solo 13%).
2. **Recompute-from-source** (`:38-46`, upsert `sample_count = EXCLUDED.sample_count` absoluto): la realimentación no puede componerse vía estado acumulado — cada consolidación parte de cero desde las filas fuente.
3. **Y objetivo**: los labels son re-ejecución en el bloque asentado (drift-tracker), NO funciones del score calibrado → no hay feedback por el lado de Y.
4. **Sin feedback de selección bajo config default**: el fold NO gatea emisión — `ScoringPipeline` es "observe-only by default (no emission change)" (`opportunity_emitter.rs:122-125` main) y `ARBX_SCORING_HARD_GATE=false` default (`.env.example:195`). El fold se registra en el wire pero no cambia qué (e,Y) existen. Si el operador activara el hard-gate, un lazo selección→labels→calibración sería posible pero amortiguado por κ + cadencia 100-labels + recompute. El design no nombra este lazo condicional — lo agrego como observación LOW (§6).
5. **Backoff anti-starvation del tracker** (la pieza de la migración): main YA tiene el equivalente — verificado `drift_tracker.rs:200-203`: `sim_last_attempt_at < now() − (30 · 2^LEAST(sim_attempts,7))` con `sim_attempts < max_attempts` (default 10, `:53-64`). Horario equivalente al del branch (base 30s, techo 2⁷=64min) — claim "horarios equivalentes" del design §2.1 ✓ VERIFICADO. Es anti-starvation de poison-rows, no anti-feedback de calibración; el design NO lo confunde.

### 1.4 MINOR-1 — la cota |log_lr| "≲9.2" está mal calculada para θ₀ extremo

`ln(9999) ≈ 9.21` es la cota del logit UNILATERAL, no del log-LR. El log-LR es una DIFERENCIA de logits: si θ₀ queda pineado en el clamp inferior (base-rate 0 — plausible post-P1-3: el flood XEN/AGLD es 100% rechazo económico ⇒ Y=0 exacto es la etiqueta dominante) y un operador tiene wins=n, entonces |log_lr| ≤ 2·ln(9999) ≈ **18.42**. Alcanzable en la práctica: θ₀→1e-4, n=100 all-wins ⇒ θ_k≈0.833, log_lr ≈ +10.8 > 9.2. El test existente (`llr < 25`) no discriminaría entre 9.2 y 18.4. **Sin ruptura de correctitud** (todo finito, clamp vivo, sin fabricación), pero el enunciado aritmético del design §4-capa-4 debe corregirse a "|log_lr| ≤ 2·ln(9999) ≈ 18.4" antes del apply-WO.

### 1.5 MINOR-2 — watermark de `tick()` cuenta filas que `consolidate()` no pliega

`tick()` cuenta `actual_timestamp IS NOT NULL AND actual_timestamp > watermark` (blob `:156-166`) — INCLUYE filas unvalued (PASS sin precio USD: timestamp seteado, `actual_profit_usd` NULL — camino real en main, `drift_tracker.rs:394-405` escribe timestamp siempre en PASS) y filas sin evidence_vector. `consolidate()` solo pliega `(profit NOT NULL) ∩ (evidence join)` y avanza el watermark solo hasta el máximo timestamp PLEGABLE. Consecuencia: ≥100 labels no-plegables más nuevas que la última plegable ⇒ cada tick de 60s dispara una re-consolidación redundante (idempotente, mismos valores, `calibrated_at` sin avance — churn de log + recompute, NO corrupción ni escritura fabricada: si total_n=0 → `skipped_no_pairs` con cero writes). La Edición 3 del design añade `AND calibration_eligible` pero NO `AND actual_profit_usd IS NOT NULL`. **Recomendación al apply-WO**: completar Edición 3 con ese predicado para que el disparador cuente exactamente lo que el fold consume. La invariante §4 no se rompe en sustancia (el store solo deriva de pares reales), pero su enunciado ("≥100 labels Y reales (actual_timestamp + calibration_eligible)") es impreciso frente a la definición de label del propio blob (requiere además profit NOT NULL).

---

## 2. COLISIÓN DE MIGRACIÓN 111 — VERIFICADA, la resolución DROP es la única válida

### 2.1 La colisión es real y exacta como el design la describe

- **main TIENE** `database/migrations/111_paper_trade_runs_calibration_eligibility.sql` (leído completo): `sim_fail_family TEXT CHECK(structural|economic|market)` DO-block catalog-guarded, `calibration_eligible BOOLEAN NOT NULL DEFAULT TRUE`, `sim_attempts`/`sim_last_attempt_at`, índice parcial `idx_ptr_pending_calibration` **CONCURRENTLY**. Todo coincide con la tabla §2.1 del design, columna por columna.
- **El branch TIENE** `111_drift_tracker_backoff.sql` (leído completo vía `git show`): `actual_attempt_count INT NOT NULL DEFAULT 0`, `actual_next_attempt_at TIMESTAMPTZ`, índice `idx_paper_trade_runs_next_attempt` CONCURRENTLY, ALTER desnudo `ADD COLUMN IF NOT EXISTS`. Mismo número 111, archivo distinto, columnas distintas.
- `git show b8600895 --stat`: toca **SOLO** `database/migrations/111_drift_tracker_backoff.sql` (+19/−5) ✓ — descartar ese commit descarta la migración íntegra.
- El diff de `drift_tracker.rs` del branch referencia `actual_attempt_count`/`actual_next_attempt_at` **7 veces** — portarlo contra el schema de main (que NO tiene esas columnas) rompería en compilación o runtime. Ambos NO-PORT están acoplados y son obligatorios.

### 2.2 El port NO introduce migración alguna

- Inventario §3.1 del design: **cero archivos .sql**. Verifiqué que ninguna de las superficies que Stage 2b/2c toca requiere DDL: `math_operator_calibration` ya existe (`103_math_evidence_scoring.sql:21-28` — operator_id SMALLINT PK, log_lr, sample_count, calibrated_at), `scored_opportunities.evidence_vector JSONB` (`103:19-20`), `paper_trade_runs.actual_profit_usd NUMERIC(18,6)` (`051:31`) + `actual_timestamp TIMESTAMPTZ` (`051:34`), `calibration_eligible` (`111` main). El join del blob es type-correcto: `so.opportunity_id TEXT` (`097:24`) = `ptr.opportunity_id::TEXT` (UUID, `051:19`).
- Numeración: máxima existente en main = `118_opportunities_detected_at_breakdown_idx.sql`; si el port alguna vez necesitara migración, ≥119. Moot — no necesita.
- **CRITICAL evitado**: el cherry-pick mecánico prohibido por el design §7 habría aterrizado `111_drift_tracker_backoff.sql` con número duplicado sobre el 111 desplegado. La regla "port por hunks según §3.1, cherry-pick ciego PROHIBIDO" es el control correcto.

---

## 3. DRIFT DEL PORT — los 8 archivos aplican limpio; el charter corregido sobre cartridge_boot

### 3.1 Medición exacta (merge-base `20c93917` → `origin/main` `9ac06d2d`)

`git diff --stat` sobre los 12 archivos del branch: SOLO drifted `README.md` (+12), `drift_tracker.rs` (+482), `cartridge_boot.rs` (+150).

- **Los 8 archivos del port (2 nuevos + 6 hunks)**: drift **CERO** — `stage2_calibration.rs` y `priors_cache.rs` son nuevos; `recon/src/main.rs`, `searcher-rs/src/lib.rs`, `searcher-rs/src/main.rs`, `opportunity_emitter.rs`, `scored-opportunities-archiver.ts`, `.env.example` idénticos entre merge-base y main. Los contextos de los hunks existen verbatim en main (verifiqué `recon/main.rs:320-358` mod-block + spawn del drift-tracker; lib.rs `pool_sources`/`publisher`; main.rs `persistence`/`publisher`; emitter `:112-113` campo pool, `:126` área scoring, `:490-516` captura de evidence_vector + `build_score_record`; archiver `prior_log_odds:58`/`chain_id:59`; `.env.example:195`/`:202-203`/`:306-307`).
- **Dependencias NO-branch-touched**: `math_evidence.rs` drift CERO (la firma `evidence_posterior_log_odds` que `priors_cache.rs:201` invoca existe en main `:393`); `shared-rs/killswitch` drift CERO; `mod math_evidence` declarado en AMBOS targets (`lib.rs:216` pub, `main.rs:198`) ⇒ `crate::math_evidence` resuelve en priors_cache para lib y bin. **No hay dependencia compile-rompible.**
- **Diff del emitter NO depende del cartridge_boot dropeado**: sus 5 hunks solo tocan PriorsCache/fold/wire/tests — cero referencia a route_metadata. Dropear el diff de cartridge_boot (+87, Gap-1) es compile-safe.
- **Supersede verificado**: main construye el route SOLO desde plan-legs con gate estructural documentado (`cartridge_boot.rs:1558-1580`: "sc.candidate's flattened token pairs are NOT a valid source (structural gate), only the plan legs are") y los **9 sitios de emisión** pasan route_ref (grep: 1368, 1463, 1588, 1603, 1637, **1645**, 1656, 1664, 1674 — el design enuncia "9 sitios" pero lista 8 líneas, omite 1645, el único `emit_accepted`; cosmético). El enfoque merge-candidate del branch es efectivamente inferior.

### 3.2 MINOR-3 — §8 del design mal-enumeró el drift

"Drift main↔merge-base en archivos porteados: SOLO `drift_tracker.rs` y `cartridge_boot.rs`" — **README.md también drifted (12 líneas)** y está en el inventario (fila 9, opcional). El claim material (los 8 archivos CORE sin drift) es VERDADERO, y §3.5 ya acknowledges la adaptación de README (fila "Gap 1" ahora ✅, texto debe decir cerrado) — pero la frase de trazabilidad §8 es internamente inconsistente con §3.5. Corregir la frase. **Nota que refuerza hacer §3.5**: el README de main HOY overclaima ("2b Offline LR calibration ⏳ armed end-to-end — awaiting first real labels ... → log-LR") cuando el writer NO existe en main (grep `stage2_calibration|Stage2Config|math_operator_calibration` en `backend/recon/src/` = 0 hits) — pre-existente, no defecto del design; el port lo vuelve verdadero.

### 3.3 CORRECCIÓN AL CHARTER — cartridge_boot.rs NO fue tocado por WO-02/WO-04

El charter me pide declarar que "el apply WO-07 futuro chocará en cartridge_boot.rs que WO-02/WO-04 acaban de tocar". **La evidencia refuta la premisa**: el árbol sucio (`git status --porcelain`) tiene `backend/searcher-rs/src/hot_path_emitter.rs` (WO-02: wiring HotPathEmitter, board row 11) + `backend/searcher-rs/src/scanner.rs` + 3 archivos frontend — **cartridge_boot.rs está LIMPIO** (HEAD `f7db6867` = origin/main en ese archivo) y de todos modos es NO-PORT. **No existe colisión de archivos entre el inventario WO-07 y el árbol sucio** (scanner/hot_path_emitter/RuntimePostureBar/websocket-client ∉ port). La serialización REAL que el apply-WO debe respetar: (a) ambos cambios conviven en el MISMO crate searcher-rs ⇒ el `cargo check -p searcher-rs` del apply-WO compilará el árbol COMBINADO (sus hunks + los cambios sin commitear de WO-02); (b) §36 disciplina de branches (verificar `git branch --show-current` antes de cualquier commit futuro). Declarado.

---

## 4. CADENA DE BLOCKERS HONESTA — sin overclaim de activación

- **Línea base §0 re-verificada por mí hoy (ssh `arbx` read-only, 1 sesión)**: `math_operator_calibration` = **0** · `paper_trade_runs WHERE actual_timestamp IS NOT NULL` = **0** · `WHERE sim_block_number IS NOT NULL` = **0** · `WHERE calibration_eligible` = **598.878** · Redis `*calib*` = **0 claves**. Los 5 números del §0 son EXACTOS.
- El design NO overclama activación: §6.2 dice explícitamente "La ACTIVACIÓN de WO-07 exige P1-3 vivo — sin passed=t no hay labels... Encender ARBX_STAGE2_CALIBRATION_MODE=on hoy produciría únicamente stage2_calibration.waiting cada 60s (debug log) — honesto e inofensivo". La recomendación "apply-WO inmediato" se refiere al CÓDIGO dormante (3 flags OFF), no a activación. Correcto y honesto.
- El corolario estructural §0 (el drift-tracker no tendría nada que resolver: su scan exige `sim_block_number IS NOT NULL` — verificado `drift_tracker.rs:194`, el design lo cita como :190, off-by-4 cosmético) también es exacto: 0 filas hoy.
- Las 3 ediciones de reconciliación §3.2 aplican limpio sobre el blob (los 3 contextos existen verbatim en `4c413ecd`) y su SEMÁNTICA está verificada contra main: STRUCTURAL → `SET calibration_eligible=false` sin label (`drift_tracker.rs:307-311`); ECONOMIC/MARKET → label terminal `actual_profit_usd = 0.0` EXACTO + `actual_timestamp = now()` (`:327-332`); PASS → valorado best-effort o unvalued-NULL (`:394-410`). El blob ya excluye NaN/Inf y unvalued del fold — `win ⇔ y > 0.0` cubre ambas trayectorias terminales honestamente. La Edición 1 (doc) hace verdadero un comentario que en el branch era pre-S4-03.
- Invariante §4: las 7 capas existen como el design las describe (flag-off default en el hunk §3.3; umbral en `tick`; `total_n==0 → skipped_no_pairs`; matemática n=0; labels solo del tracker; `PriorsCache` honesto-None — verificado en blob `94a5eed3` líneas 18-27 y 107-135, PG caído retiene último slice bueno, PG ausente → `disabled()`; Zod sin `.passthrough()` — verificado grep, solo aparece en comentario `archiver.ts:46`). Con la precisión de MINOR-2 en la capa 2.

---

## 5. BLOBS — contenido real del branch

| Blob designado | Real (git ls-tree) | Veredicto |
|---|---|---|
| `stage2_calibration.rs` = `4c413ecd11abff8d2c0765fff4a31e618beb08a1` | `4c413ecd11ab...` ✓ 393 líneas, 6 tests (shrinkage×2, zero-n, log-LR+, clamp, logit-simetría) | ✓ EXACTO |
| `priors_cache.rs` = `94a5aed3...` (§1.1 y §8 completo) | **`94a5eed33b78a47b93eb9fe282af439eda2db007`** ✓ 267 líneas, 6 tests | **MINOR-4**: errata por transposición (ae↔ed). `git cat-file -e 94a5aed3...` = NO EXISTE. Fail-fast si un applier lo usara ciegamente (no hay riesgo de corrupción silenciosa), pero debe corregirse en §1.1 y §8 antes del apply-WO |

Diff completo del branch: "12 archivos, +1023/−39" ✓ EXACTO (re-medido). Commits y hashes completos ✓ resuelven. Conteo de tests: recon stage2 6/6 ✓, searcher priors_cache 6/6 ✓ (nombres coinciden con §5 gate 2), emitter "2 extendidos + 1 nuevo" ✓ (diff: accepted-variant extendido, rejected-variant extendido, `score_record_calibrated_fold_is_carried` nuevo).

---

## 6. Hallazgos consolidados

| # | Severidad | Hallazgo | Acción requerida |
|---|---|---|---|
| MINOR-1 | MINOR (matemática documental) | Cota \|log_lr\| del design (≲9.2) es la del logit unilateral; la real es 2·ln(9999) ≈ 18.4, alcanzable con θ₀ en clamp (escenario post-P1-3 plausible: rechazo económico dominante) | Corregir el enunciado §4-capa-4 en el apply-WO |
| MINOR-2 | MINOR (coherencia trigger/fold) | `tick()` cuenta unvalued/no-evidence hacia el umbral 100; `consolidate()` no los pliega ni avanza watermark sobre ellos ⇒ consolidaciones redundantes idempotentes ante ≥100 labels no-plegables nuevas | Extender Edición 3 con `AND actual_profit_usd IS NOT NULL` |
| MINOR-3 | MINOR (trazabilidad) | §8 dice drift "SOLO drift_tracker + cartridge_boot" — README.md también drifted (12 ln); inconsistente con §3.5 que sí lo trata | Corregir frase §8 |
| MINOR-4 | MINOR (trazabilidad) | Blob-hash de `priors_cache.rs` erróneo (94a5**ae**d3 vs real 94a5**ed**3) en §1.1 y §8 | Corregir antes del apply-WO |
| LOW-A | LOW (observación) | Lazo selección→labels→calibración si el operador activa ARBX_SCORING_HARD_GATE (hoy fold NO gatea emisión: observe-only default). Amortiguado por κ=20 + recompute + cadencia 100 | Documentar en el runbook del flip P1-3 |
| LOW-B | LOW (heredado) | Fold naive-Bayes asume independencia condicional de operadores (evidencia correlacionada se doble-cuenta) — supuesto del §IV motor de main, NO introducido por el port; caveat e_k escalar ya documentado (blob §caveat 3, design §7) | Follow-up con datos (ya previsto por el design) |
| COSM-1 | Cosmético | Citas file:line desplazadas: `sim_block_number` :190→real :194 · `051:16-18`→real :31/:34 · "9 sitios" con 8 líneas listadas (omite 1645) · "prefijos distintos" (ARBX_CALIBRATION_MIN_* vs _CONSOLIDATE — mismo prefijo, nombres/consumidores distintos, sin colisión real: verificado `scoring-status.ts:137-139` y ausencia de los knobs nuevos en `.env.example`) | Sin acción bloqueante |
| CHARTER | Corrección | La premisa "WO-02/WO-04 tocaron cartridge_boot.rs y el apply chocará ahí" es FALSA: árbol sucio = scanner.rs + hot_path_emitter.rs + frontend; cartridge_boot.rs limpio y NO-PORT; intersección port∩árbol-sucio = ∅ | Serialización real = build combinado del crate searcher-rs + §36 |

## 7. Chequeo de gates del design (§5) — factibilidad

Gates 1-4 requieren cargo/tsc — **0 cargo** en este verify (charter); su factibilidad es alta porque (a) los hunks compilaron en el branch, (b) cero drift en los 6 archivos-hunk y en `math_evidence.rs`, (c) `mod math_evidence` presente en ambos targets. Gate 5 (CI 14 checks): campos aditivos + flag off — sin impacto esperado, correcto. Gate 6 (invariante post-deploy): superficie observable bien definida (PG count 0, Redis 0, wire null/flat_prior, logs dormant/waiting) — ejecutable. Gate 7 NO-GIT: este verify tampoco tocó git.

## 8. Presupuesto y disciplina

- HTTP dominio público: **0/5** (toda la evidencia es git-local + 1 sesión ssh read-only).
- SSH: 1 sesión `arbx`, solo `docker exec ... psql -t -c SELECT` + `redis-cli --scan` — CERO mutación VPS.
- Git: solo lecturas (`show`, `diff`, `ls-tree`, `cat-file -e`, `rev-parse`, `merge-base`, `status`) — CERO checkout/commit/push.
- Lexicon: Topological Yield / Variedad de Liquidez / TLS aplicados donde corresponde.

## 9. Conclusión para el board

**WO-07-DESIGN = VERIFICADO VIABLE.** El apply-WO puede proceder bajo la tabla §3.1 con 4 correcciones menores incorporadas (MINOR-1..4: cota aritmética, predicado del watermark, frase §8, blob-hash) y 2 anotaciones de runbook (LOW-A/B). La colisión 111 está correctamente resuelta por DROP-CONCIENTE — portar cualquier migración habría sido CRITICAL. El charter debe corregir su premisa sobre cartridge_boot.rs/WO-02/WO-04: no hay colisión de archivos; la coordinación real es el build combinado del crate searcher-rs.

— math-validator (ecc:database-reviewer rubric), Gang Omniscience, 2026-09-06.
