# BR-05+06 — VERIFY: adversarial math audit (calibración bayesiana + SizeOptimizer)

- **WO**: BR-05+06-VERIFY · kind: **verify** (READ-ONLY — 0 compilación, 0 git write, 0 mutación VPS, 0/5 requests dominio público)
- **Agente**: math-validator (rubric math-validator, Gang Omniscience §9) · 2026-09-07
- **Charter**: derivar INDEPENDIENTEMENTE el shrinkage κ=20, verificar las 7 capas del invariante §4
  de `WO-07-DESIGN.md` contra el diff aplicado, las 3 ediciones de reconciliación, la no-portación de
  la migración 111 colisionante (BR-05); y que los property tests del SizeOptimizer asertan la FORMA
  CERRADA con fixtures de procedencia on-chain (BR-06).
- **Lexicon**: Topological Yield (profit) · Variedad de Liquidez (pool/DEX) · TLS (flash loan) ·
  Decoherencia de Estado (slippage).

## VEREDICTO EJECUTIVO

| WO | Veredicto | Razón en una línea |
|---|---|---|
| **BR-05** | **BLOCK — apply AUSENTE** | El port-back WO-07 NO fue aplicado en NINGÚN lugar verificable (árbol, 38 worktrees, ramas): `stage2_calibration.rs` y `priors_cache.rs` no existen fuera del branch fuente `feat/stage2-calibration-closure` (NO mergeado). La matemática del diseño y del blob quedó aquí **pre-verificada al 100%** — cuando el apply aterrice, este reporte es su referencia de verificación. |
| **BR-06** | **BLOCK — deliverable AUSENTE** | `size_optimizer.rs` está byte-idéntico a HEAD (`git diff` vacío, 0 marcadores BR-06): los property tests de la forma cerrada NO existen. Auditée el inventario PRE-EXISTENTE completo (21 tests size_optimizer + 12 kelly_sizing + 27 amm_math), derivé la forma cerrada del ciclo CFMM de 2 piernas que el builder DEBE asertar, y encontré 1 fixture que dice "Real …pool shape" SIN procedencia citada (criterio CRITICAL del charter, pre-existente). |

No es un veredicto de "mala calidad del builder": es que **el builder no ha entregado todavía** (yo soy
uno de los primeros agentes de la mesa — ver §0). Cero pares en `audits/cerebro-2026-09-07/` al momento
de este reporte aparte del board. Todo lo verificable hoy se verificó; lo que falta exige el apply.

---

## 0. Sincronía de mesa redonda (estado al escribir)

- `ls audits/cerebro-2026-09-07/` → SOLO `GOAL-WORKORDERS.md` (07:35). **0 reportes de pares BR-\* aún**.
- Upstreams leídos completos: `audits/omniscience-integration-2026-09-06/WO-07-DESIGN.md` (design
  VIABLE, 2026-09-06) y `WO-07-VERIFY.md` (PASS del mismo día, por math-validator — mi encarnación
  anterior). Este reporte **construye sobre** ese verify: re-confirma sus 4 MINORs con aritmética
  propia y detecta que **ninguno fue incorporado aún** (el apply no existe).
- Programa hermano `audits/control-board-2026-09-07/` (CB-\*) está activo en paralelo; verifiqué
  **cero overlap** con mis claims (`grep -l size_optimizer|stage2_calibration|priors_cache` = vacío);
  su CB-02-RUST-APPLY declara explícitamente que los programas no se tocan. Precedente útil: CB-02
  entregó "NO-OP CON EVIDENCIA" cuando su diseño-upstream no existía — misma honestidad que este reporte.
- Estado del árbol compartido: branch `feat/hops-live-01` @ `27aca289` (⚠️ §36 — NO es main), sucio
  con archivos ajenos a mis claims (`edge/worker/src/index.ts`, `edge-parity.test.ts`, settings,
  SKILL.md, workflow js). Mis 3 archivos claim: `size_optimizer.rs` LIMPIO vs HEAD; los otros 2 no existen.

---

# PARTE 1 — BR-05 (calibración bayesiana)

## 1.1 Derivación INDEPENDIENTE del shrinkage (charter: "derivar INDEPENDIENTEMENTE")

**Clasificación: CANONICAL_REPO** (blob `4c413ecd`, branch `feat/stage2-calibration-closure`,
`backend/recon/src/stage2_calibration.rs`) + derivación matemática propia.

**Estimador.** Operador k dispara en n_k eventos etiquetados; Y∈{0,1} con win ⇔ y>0. Prior
Beta(α,β) con α=κ·θ₀, β=κ·(1−θ₀) (media θ₀, "tamaño de muestra" κ). Likelihood Bernoulli
(w_k éxitos en n_k). Posterior Beta(α+w_k, β+n_k−w_k); su media:

```
E[p_k|data] = (κ·θ₀ + w_k)/(κ + n_k)      ≡  `shrunk_theta` (blob :326-332)
```

Coincide con la fórmula del blob (que la escribe como (κθ₀ + n·p̂_emp)/(κ+n), idéntica pues
n·p̂_emp = w). ✓ **La forma cerrada es la media posterior conjugada Beta-Bernoulli exacta.**

**log-LR.** Para el evento binario "operador k disparó" (E_k), la aditividad log-odds de Bayes da
logit(P(Y=1|E_k)) − logit(P(Y=1)) = ln[P(E_k|Y=1)/P(E_k|Y=0)] = LLR_k. El blob implementa
`log_lr = logit(clamp(θ_k)) − logit(clamp(θ₀))` con θ₀ = base-rate POOLED sobre TODOS los pares
etiquetados (correcto: el base rate es sobre todos los eventos, no solo los de disparo) y
θ_k = P̂(Y=1|disparo). Plug-in empirical-Bayes exacto para el fold naive-Bayes
`posterior = prior_log_odds + Σ log_lr_k·e_k` que define `math_evidence.rs::evidence_posterior_log_odds`
(verificada viva en el árbol: `backend/searcher-rs/src/math_evidence.rs:393`). ✓

**Identidades numéricas del charter — recomputadas por mí** (exactas, `py -3`):

| Claim del charter/design | Recomputo | Veredicto |
|---|---|---|
| κ=20, n=3 ⇒ "~87% prior" | 20/23 = **0.869565** = 86.96% | ✓ |
| n=150 ⇒ "~88% empírico" | 150/170 = **0.882353** = 88.24% | ✓ |
| n=0 ⇒ log_lr=0 exacto, LR=1 | (a) short-circuit `(0.0, 0)` en `operator_log_lr` (blob :337-340); (b) analítico θ_k = κθ₀/κ = θ₀ ⇒ logit diff = 0. **Doble-seguro** | ✓ |
| shrinkage n=3/w=3, θ₀=0.5 | (10+3)/23 = 13/23 = **0.565217**; log_lr = ln(1.3) = +0.2624 nats (3 eventos NO son evidencia) | ✓ |
| test `better_than_base`: n=100/w=80 | θ_k = 90/120 = 0.75; logit(0.75) = ln(3) = **1.0986123** — identidad EXACTA vs ln(3) | ✓ |
| test `all_wins_clamps`: n=10⁴/w=10⁴ | θ = 10010/10020 = **0.999002** (clamp ni siquiera binde); logit = ln(1001) = **6.908755** < 25 ✓ finito | ✓ |
| test `logit_is_symmetric_around_half` | logit(0.25)+logit(0.75) = ln(1/3)+ln(3) = 0 (anti-simetría) | ✓ |

## 1.2 La cota del clamp — el charter la trae MAL, re-confirmación con números exactos

El charter (y el design §4-capa-4, y el COMENTARIO del propio blob :73 "log-LR bounded ±~9.2")
enuncian clamp θ∈[1e-4, 1−1e-4] ⇒ |log_lr| ≲ 9.2. Mi derivación:

- Cada logit clamped ∈ [−ln(9999), +ln(9999)], ln(9999) = **9.210240** — esa es la cota UNILATERAL.
- log_lr es una DIFERENCIA de logits ⇒ cota general |log_lr| ≤ **2·ln(9999) = 18.420481**.
- **Contraejemplo alcanzable**: θ₀ pineado en 1e-4 (base-rate 0 — el escenario post-P1-3 plausible:
  rechazo económico dominante ⇒ Y=0 exacto es la etiqueta dominante del flood XEN/AGLD), operador
  n=100 all-wins, κ=20 ⇒ θ_k = 100.002/120 = 0.83335 ⇒ **log_lr = +10.8198 > 9.21**.

Esto ya fue MINOR-1 en `WO-07-VERIFY.md` (2026-09-06). **RE-CONFIRMADO con aritmética propia y
sigue SIN incorporarse**: el enunciado vive en el charter BR-05, en el design §4 capa 4, Y en el
comentario del blob fuente (`const THETA_EPS` doc-comment, blob :72-73). **Acción para el apply**:
corregir el comment del blob a "|log_lr| ≤ 2·ln(9999) ≈ 18.42 (9.21 unilateral por logit; 9.21 para
el diff solo si θ₀=0.5)". Sin ruptura de correctitud (todo finito, clamp vivo, cero fabricación) —
defecto documental en tres lugares.

## 1.3 Las 7 capas del invariante §4 — una por una (charter explícito: capas 2, 3, 4, 6)

**Estado del árbol: el port NO fue aplicado** (evidencia §1.5). Por tanto cada capa se verifica en su
ubicación canónica (blob fuente / hunk del design / main pre-existente) y se reporta el estado del
árbol — que es lo honesto; NO se puede verificar "contra el diff aplicado" porque no hay diff.

| # | Capa | Mecanismo (ubicación canónica) | Verificación mía | Estado árbol |
|---|---|---|---|---|
| 1 | Job no spawneado | hunk `recon/src/main.rs` (design §3.3): `ARBX_STAGE2_CALIBRATION_MODE≠"on"` → `stage2_calibration.dormant`; `.env.example` default `off` | Diseño correcto (default-off en 2 lugares) | **AUSENTE** — `grep -c stage2_calibration backend/recon/src/main.rs` = **0**; `grep -c ARBX_STAGE2_CALIBRATION_MODE .env.example` = **0** |
| 2 | **Umbral de disparo** (charter) | blob `tick()` (:145-177): watermark = `MAX(calibrated_at)`; `new_labels < consolidate_every (100)` → `stage2_calibration.waiting` + return — cero writes | **VERIFICADO en blob**: el count es `actual_timestamp IS NOT NULL AND > $1`; el watermark se escribe solo como el max `actual_timestamp` REALMENTE plegado (nunca salta fila) | AUSENTE (archivo no portado) |
| 3 | **Guard de vacuidad** (charter) | blob `consolidate()`: `total_n == 0` → `stage2_calibration.skipped_no_pairs`, store intacto | **VERIFICADO en blob** (:245-255): retorna Ok(()) ANTES del upsert; el único camino al store exige pares (e,Y) joinables | AUSENTE |
| 4 | **Matemática anti-fabricación** (charter) | n=0 ⇒ (0.0,0) doble-seguro; shrinkage κ=20; clamp [1e-4,1−1e-4]; NaN/Inf excluidos (:227-229); κ del env filtrado `finite && >0` (:106-112); `.take(OPERATOR_COUNT)` previene OOB | **VERIFICADO** (§1.1 completo) — con la corrección de cota §1.2. Los 3 tests citados por el design existen en el blob con la aritmética exacta | AUSENTE |
| 5 | Labels reales solamente | `drift_tracker.rs` main S4-03: PASS→yield realizado; ECONOMIC/MARKET→0 exacto; STRUCTURAL→sin label + `calibration_eligible=false`; PENDING ≠ label | **PRE-EXISTENTE EN MAIN** (citado `drift_tracker.rs:285-418` por WO-07-VERIFY; hoy 0 filas con `actual_timestamp` en VPS) — y `grep -c actual_attempt_count drift_tracker.rs` = **0** (cero contaminación de columnas del branch) | PRESENTE (pre-existente) |
| 6 | **Lectura honesta / wire null** (charter) | blob `priors_cache.rs` (94a5eed3): store vacío ⇒ `calibration()=None` ⇒ `section_iv_fold` retorna `posterior_log_odds: None, calibration_applied: false` (:176-184); PG caído retiene último slice bueno; PG ausente ⇒ `disabled()` (:82-93) | **VERIFICADO en blob** (lectura directa: :18-25, :49, :116, :158-184). Pre-state del wire HOY coherente: emitter emite `prior_log_odds`+`source_context:"flat_prior"` (`opportunity_emitter.rs:664-667`, test :869); archiver Zod sin `posterior_log_odds` (campo no inventado — coherente mientras el fold no exista) | AUSENTE (blob no portado) |
| 7 | Contrato wire estricto | hunk archiver: 2 campos Zod `nullable().optional()`; sin `.passthrough()` (campos no declarados se DROPEAN, nunca se inventan) | Diseño verificado por WO-07-VERIFY §4; **el hunk NO está** (grep `posterior_log_odds` en archiver = **0**) — y como el emitter tampoco lo emite, el wire actual es consistente (nada se pierde, nada se inventa) | AUSENTE (par emitter↔Zod coherentemente ausente) |

**Conclusión de capas**: la matemática y la semántica de las 7 capas están CORRECTAS en sus fuentes
(blob + design); NINGUNA está aplicada al árbol. El invariante §4 NO puede violarse hoy porque el
writer no existe en el árbol — el store vive en 0 filas en VPS (verificado 2026-09-06 por
WO-07-VERIFY §4; sin cambios desde: nada se aplicó ni deployó de esto).

## 1.4 Las 3 ediciones de reconciliación + migración 111

- **Ediciones 1/2/3** (doc-semántica labels S4-03; gate `AND ptr.calibration_eligible` en
  `consolidate()`; mismo gate en `tick()`): existen SOLO como diffs en `WO-07-DESIGN.md` §3.2.
  **NO están en archivo alguno** (trivialmente: el archivo destino no existe). ⇒ "verificar que
  están" = **NO ESTÁN — apply ausente**.
- **MINOR-2 de WO-07-VERIFY sigue ABIERTO y NO incorporado al design**: la Edición 3 añade
  `AND calibration_eligible` pero NO `AND actual_profit_usd IS NOT NULL`. Consecuencia (análisis del
  par, re-validado por mí leyendo el blob): ≥100 labels unvalued (timestamp seteado, profit NULL —
  camino real: `drift_tracker.rs` escribe timestamp siempre en PASS) o sin evidence_vector, más nuevas
  que el watermark, disparan re-consolidaciones redundantes cada 60s — idempotentes (recompute
  absoluto), sin corrupción, pero churn. **Recomendación al apply (reiterada)**: completar Edición 3
  con `AND actual_profit_usd IS NOT NULL` para que el disparador cuente EXACTAMENTE lo que el fold
  consume.
- **Migración 111 colisionante**: `ls database/migrations/ | grep ^11` → existe SOLO
  `111_paper_trade_runs_calibration_eligibility.sql` (la de main). `111_drift_tracker_backoff.sql`
  del branch NO existe en el árbol; 0 referencias a `actual_attempt_count`/`actual_next_attempt_at`
  en `backend/recon/src/drift_tracker.rs`. **NADA de la migración colisionante se portó** ✓ —
  (vacuamente cierto: el port entero está ausente; el cherry-pick ciego PROHIBIDO del design §7 no
  ocurrió). Numeración main ya avanzó a `119_trading_config_lp_fee_default_pct.sql` — si el apply
  algún día necesitara DDL (no lo necesita: cero DDL nuevo, verificado por WO-07-VERIFY §2.2),
  sería ≥120.

## 1.5 Evidencia de la ausencia del apply (forense, reproducible)

| Verificación | Resultado |
|---|---|
| `ls backend/recon/src/` | 8 archivos; **sin `stage2_calibration.rs`** |
| `ls backend/searcher-rs/src/priors_cache.rs` | **No existe** |
| `git status --porcelain` (árbol, branch `feat/hops-live-01`) | `size_optimizer.rs` LIMPIO; ningún archivo de calibration |
| `git log --all --oneline -- backend/recon/src/stage2_calibration.rs` | **único** commit: `113145e8` (el branch fuente, NO mergeado) |
| 10 worktrees `agent-*` + 28 históricos | `grep stage2|priors|size_opt|calibr` en status de cada uno = **0** |
| `git cat-file -e 4c413ecd…` / `94a5eed3…` | Ambos blobs EXISTEN y alcanzan (fuente del port intacta y disponible para el apply) |
| Blobs correctos | `stage2_calibration.rs` = `4c413ecd11ab…` (393 ln, 6 tests) ✓ · `priors_cache.rs` = `94a5eed33b78…` (267 ln, 6 tests) ✓ — nota: el design §1.1/§8 arrastra la errata `94a5aed3` (MINOR-4, corregir al copiar) |

---

# PARTE 2 — BR-06 (verificación matemática del SizeOptimizer)

## 2.1 El deliverable no existe

`git diff HEAD -- backend/searcher-rs/src/size_optimizer.rs` = **vacío**; 0 marcadores
`BR-06`/`WO-07` en el archivo (148,419 bytes, mtime 07:22 = checkout del branch, no edición).
Los "property tests del sizing convexo CFMM (fórmula cuadrática cerrada) contra fixtures on-chain
verificados" del board NO fueron escritos. **BLOCK**.

Lo que sigue es la auditoría del inventario PRE-EXISTENTE — la línea base que el builder no puede
regresar, y el mapa exacto de lo que falta (charter: "no solo casos felices").

## 2.2 Forma cerrada del ciclo CFMM de 2 piernas — DERIVACIÓN PROPIA (el target del builder)

Derivación completa (el builder DEBE asertar contra esto; hoy nadie lo hace):

Ciclo x →[pool A: reservas (x₁,y₁), fee γ₁=1−f₁]→ y →[pool B: reservas (y₂,x₂), fee γ₂=1−f₂]→ x:

```
o₁ = γ₁·Δ·y₁/(x₁+Δ)          out₂ = γ₂·x₂·o₁/(y₂+o₁)          Net(Δ) = out₂ − Δ
```

Net es cóncava en Δ; ∂Net/∂Δ = γ₁γ₂·x₁y₁x₂y₂/[(x₁+Δ)²(y₂+o₁)²] = 1 ⇒ tomando raíz y sustituyendo
o₁ (la ecuación resultante es LINEAL en Δ tras el sqrt — la "fórmula cuadrática cerrada"):

```
Δ* = ( √(γ₁·γ₂·x₁·y₁·x₂·y₂) − x₁·y₂ ) / ( y₂ + γ₁·y₁ )
```

**Auto-checks de la forma** (los que el property test debe asertar):
1. **Simetría de pools**: x₁=y₁=x₂=y₂=R, γ₁=γ₂=γ ⇒ Δ* = R²(γ−1)/(R(1+γ)) = R(γ−1)/(1+γ) < 0 para
   γ<1 ⇒ sin ciclo rentable (coincide EXACTAMENTE con el test existente
   `bucket_sweep_2leg_curve_symmetric_pools_yield_best_none` — la honestidad R8 de hoy ya es la
   predicción de la forma cerrada).
2. **γ=1 simétrico**: Δ* = 0 exacto (caso marginal).
3. **El fee solo entra vía √(γ₁γ₂)** en K ⇒ intercambiar los fees entre piernas con reservas
   espejadas deja Δ* invariante — ESTA es la "simetría fee" asertable como propiedad.
4. **Monotonía en el desbalance**: ∂Δ*/∂(x₁·y₂ relativo a K) — a mayor desbalance (K/(x₁y₂) ↑),
   Δ* ↑ monótonamente — propiedad de monotonía del argmax.
5. Net(Δ*) es el máximo de la cóncava: Net(Δ) ≤ Net(Δ*) ∀Δ — el golden-section del kernel debe
   converger a Δ* (dentro de resolución de grilla + redondeo entero floor del wei).

El charter de BR-06 pide exactamente 1+3+4 (closed form, fee symmetry, monotonicidad) + borde
decimals 6/18 + caps Kelly. Ver §2.4 para el estado de cada uno.

## 2.3 Inventario pre-existente (lo que YA está y está bien)

**size_optimizer.rs — 21 tests** (11 `#[test]` + 10 `#[tokio::test]`):
- `bucket_sweep_2leg_curve_bounded_by_golden_and_envelope_enforced` (:2128) — el más fuerte:
  grid-argmax ≤ óptimo continuo (golden-section como cota superior — correcto para cóncava),
  envelope N∈[8,128] fail-fast, refinamiento N=128 ≥ 99.5% de p*, monotonía del refinamiento
  (best₈ < best₁₂₈). Referencia auto-derivada explícitamente SIN literal hardcodeado ✓.
- `bucket_sweep_2leg_curve_symmetric_pools_yield_best_none` (:2175) — pools idénticos ⇒ best None
  honesto (R8) ✓ (≡ check 1 de la forma cerrada).
- `route_quote_v2_leg_matches_v2_amount_out` (:2197) — regresión byte-idéntica contra
  `amm_math::v2_amount_out` (red de seguridad del refactor de quoting).
- Kelly post-optimization (:2952-3121): pass-through de Rejected, cap no-binding ⇒ unchanged,
  **cap binding ⇒ amount y gross escalan abajo** (dirección, no ratio exacto), gas-floor rechaza/
  acepta en la frontera exacta del multiplicador, KellyNegativeEdge con aritmética del comentario
  verificada por mí (W=100/30≈3.33, p=0.2 ⇒ f*=−0.04 <0 ✓; p=0.7, W=10 ⇒ f*=0.67>0 ✓).
- `flashloan_wrapped_subtracts_fee` (:2875) — TLS: pools simétricos ⇒ None SIN pánico (honestidad
  del fee-path de la Variedad de Liquidez).

**kelly_sizing.rs — 12 tests**: ESTOS SÍ son forma cerrada del Kelly: `f* = p − (1−p)/W` asertado
exacto (2:1 con p=0.5 ⇒ 0.25 ✓; 60/40 even ⇒ 0.20 ✓; certain-win ⇒ 1; unfavorable ⇒ 0) + rechazo de
dominio inválido (NaN/Inf/p∉[0,1]/ganancia o pérdida ≤0 — 6 tests de borde). Sólido.

**amm_math.rs — 27 tests**, propiedades relevantes:
- `v3_at_1to1_price_zero_fee_equals_cpmm_virtual_reserves` (:843) — invariante CROSS-MODELO:
  V3 single-tick en price 1:1 y fee 0 = CPMM(L,L) **bit-para-bit** en zero_for_one, ±1 wei en la
  dirección inversa (redondeo por-dirección de SwapMath documentado — excelente honestidad numérica).
- `v3_output_monotonic_in_input` (:890) — monotonía del OUTPUT vs INPUT (una sola pierna, no del
  argmax del sizing).
- `v3_deeper_liquidity_less_slippage` (:899) · `fee_30_bps_reduces_output_vs_zero_fee` (:557) ·
  `v3_fee_reduces_output` (:878) — direcciones correctas (no exactas).
- Decimales: `wei_str_weth_18` / `wei_str_usdt_6` / `wei_str_wbtc_8` (:576-590) +
  `wei_str_bug1_regression_usdt_input` (:611, regresión del bug de 6 decimales) +
  `decimal_adjustment_shifts_rate_by_ten_pow_dec_delta` (:964) — el borde 6/18 vive en la capa de
  conversión y spot-snapshot, NO en el path de sizing.

## 2.4 GAPS vs charter BR-06 (la lista exacta para el builder)

| # | Propiedad exigida | Estado hoy | Qué falta |
|---|---|---|---|
| 1 | **FORMA CERRADA** | AUSENTE — la única referencia es el golden-section del propio kernel (numérico, self-derived) | Test que aserte Δ* analítico (§2.2) vs el argmax del kernel (golden-section y bucket-sweep) sobre fixtures con desbalance no-trivial, incluyendo Net(Δ*) exacto y la cota Net(Δ)≤Net(Δ*) ∀Δ de la grilla |
| 2 | **Monotonía del sizing** | PARCIAL — solo output-vs-input V2/V3 y refinamiento N | Sweep de desbalance creciente ⇒ Δ* estrictamente creciente (check 4 de §2.2) |
| 3 | **Simetría fee** | AUSENTE — fee fijo 30bps en los tests de quoting; dirección-only en amm_math | Δ* invariante bajo intercambio γ₁↔γ₂ con reservas espejadas (check 3 de §2.2); o aserción exacta de que el fee entra solo vía √(γ₁γ₂) |
| 4 | **Borde decimals 6/18** | PARCIAL — capa de conversión y spot-snapshot solamente | Fixture de sizing end-to-end USDC(6)↔WETH(18) donde el amount_in óptimo y Net se asertan con la conversión de decimales ejercitada en el path del optimizador |
| 5 | **Caps Kelly** | FUERTE en dirección (§2.3) | Upgrade a ratio exacto: `s.optimal_amount_in == min(kernel, nav·f*·multiplier, nav·max_per_trade)/precio` — hoy solo `<` |
| 6 | **Procedencia de fixtures** | 1 HALLAZGO (§2.5); resto honestamente sintético | Todo fixture NUEVO que diga "on-chain/real" DEBE citar pool + block height + fuente (slot0()/liquidity()/getReserves eth_call o explorer) |

## 2.5 Hallazgo de procedencia — CRITICAL bajo el criterio del charter (PRE-EXISTENTE)

`backend/searcher-rs/src/amm_math.rs:942-948` — test `usdc_weth_mainnet_vector`:

> "Real USDC/WETH 0.05% pool shape: token0=USDC (6 dec), token1=WETH (18 dec). sqrtPriceX96 ≈
> 1.7727e33 …" con valores exactos `sp = 1_772_712_074_874_819_459_120_282_715_246_463`,
> `l = 548_640_024_015_773_269` — **SIN block number, SIN timestamp, SIN tx/explorer/pool address.**

- El charter BR-06 dice literalmente: "un fixture sin procedencia = RULE 00 violado = CRITICAL".
  Este fixture AFIRMA realidad ("Real … pool shape") sin cita verificable ⇒ **CRITICAL bajo el
  criterio del charter**. Clasificación: **PRE-EXISTENTE** (no lo introdujo BR-06 — el builder no
  ha tocado nada); es deuda que BR-06 debe saldar o heredar explícitamente.
- Verificación interna que SÍ hice (la aritmética del propio vector es consistente):
  √P = sp/2⁹⁶ = **22374.772**; raw = 5.0063e8 wei-WETH/wei-USDC; humano 6→18 = **5.0063e-4
  WETH/USDC** ⇒ ETH ≈ $1,997.5 — el assert `~5.0054e-4 (rel 1e-3)` pasa (diff rel 1.8e-4). La
  matemática del test es correcta; lo inverificable es el claim "Real" (pool slot0 cambia por
  bloque — sin block height no hay forma de reproducirlo).
- **Remedio (para el builder BR-06)**: citar `pool 0x88e6…5640 (USDC/WETH 0.05%) @ block N,
  eth_call slot0()/liquidity() del YYYY-MM-DD` o re-etiquetar como vector sintético
  shape-plausible. NOTA RULE 00: los fixtures sintéticos del resto del inventario están
  honestamente etiquetados (constantes de test, "hand-built test fixtures", R8-comments) — NO hay
  más claims de realidad sin cita (grep on-chain/mainnet/etherscan/0x40 en los 3 módulos: solo
  este caso + referencias de documentación de QuoterV2/Multicall3, que SÍ citan addresses
  canónicos `amm_math.rs:6-8` ✓).

---

## 3. Declaración para el turno serial-rust (charter: "si un test exige ejecución, declararlo")

Yo NO compilé (target/ pertenece a la cadena serial). Los tests cuya EJECUCIÓN cierra este verify,
en orden, para el orchestrador:

1. **Ahora (baseline, sin apply)**: `cargo test -p searcher-rs size_optimizer` + `amm_math` +
   `kelly_sizing` — deben seguir 60/60-ish verdes; es la línea base que BR-06 no puede romper.
2. **Tras el apply BR-05**: `cargo test -p recon stage2` (6/6) · `cargo test -p searcher-rs
   priors_cache` (6/6) · los 3 tests de record del emitter (2 extendidos + 1 nuevo) · `tsc --noEmit`
   api-server (hunk Zod) — los gates 1-4 de WO-07-DESIGN §5.
3. **Tras el apply BR-06**: los nuevos property tests de forma cerrada (§2.2-2.4).

## 4. Presupuesto y disciplina

- HTTP dominio público: **0/5** (los objetos de verificación son código+matemática locales; el apply
  no está deployado — el dominio no tendría nada que decir de esto).
- SSH VPS: **0 sesiones** (baseline PG/Redis verificada 2026-09-06 por WO-07-VERIFY §4; sin cambios
  posibles desde — nada se aplicó ni deployó).
- Git: SOLO lecturas (`show`, `cat-file -e`, `ls-tree`, `log --all`, `diff`, `status`, `worktree
  list`). 0 checkout/commit/push. 0 edición de código (mi único archivo escrito es este reporte).
- Compilación: 0 (charter). Cálculos: `py -3` local, reproducibles.

## 5. Conclusión para la mesa

- **BR-05 = BLOCK (apply ausente)** — con la matemática 100% pre-verificada aquí: estimador
  conjugado exacto, identidades del charter recomputadas (87%/88%/n=0/LR=1 ✓), cota corregida
  (9.21 unilateral / **18.42 general**, contraejemplo +10.82 — a corregir en 3 lugares al aplicar),
  7 capas verificadas en fuente, MINOR-2 re-abierto para la Edición 3. Cuando el apply aterrice,
  este reporte + WO-07-VERIFY son su rubrica completa; re-despachar verify tras el apply.
- **BR-06 = BLOCK (deliverable ausente)** — con el inventario base auditado (fuerte en Kelly
  cerrado, simetría-honesta y regresión byte-idéntica; débil en forma cerrada/monotonía/fee/decimals
  del sizing), la **forma cerrada derivada y auto-checkeada** (§2.2) lista para asertar, y 1
  CRITICAL pre-existente de procedencia (`amm_math.rs:942`) que el builder debe citar o re-etiquetar.
- **Acciones concretas al orchestrador**: (1) despachar el apply BR-05 por hunks según WO-07-DESIGN
  §3.1 (blobs verificados alcanzables) incorporando MINOR-1 (cota), MINOR-2 (predicado del
  watermark) y MINOR-4 (blob-hash errata) — luego re-verify en turno serial con los tests §3.2;
  (2) despachar BR-06-builder con la tabla §2.4 como especificación y §2.2 como forma cerrada
  objetivo; (3) el apply de BR-05 DEBE respetar §36 (el árbol hoy está en `feat/hops-live-01`, no
  en main).

— math-validator, Gang Omniscience, 2026-09-07. Construido sobre WO-07-DESIGN/WO-07-VERIFY
(2026-09-06); sin pares BR-* previos que citar (soy temprano en la mesa — §0).
