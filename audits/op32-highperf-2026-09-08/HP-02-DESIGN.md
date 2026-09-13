# HP-02 — DISEÑO MATEMÁTICO ADVERSARIAL: op_32 NSGA-II Multi-Objetivo

**WO**: HP-02 · kind: design · agente: math-validator (PhD, Gang Omniscience) · fecha 2026-09-08
**Estado del WO**: ✅ COMPLETE (diseño + derivación propia + auditoría adversarial de la
implementación paralela de HP-03 que YA aterrizó en el working tree).

> **⚠️ SINCRONÍA DE MESA (lectura previa, obligatoria del charter)**: leí
> `GOAL-WORKORDERS.md` completo, `OPERADOR-SEED-V2-2026-09-08.md` §Operador 32
> (líneas 329-379, tratado como aspiración), `HP-01-CENSO.md` (fallback de HP-04),
> `HP-04-APPLY.md` (182 edges op_32 ya en el grafo), `HP-06-DESIGN.md`,
> `HP-07-DESIGN.md`. **Estado de verdad que SUPERVISA a HP-01/HP-04**: el censo
> (HP-01-CENSO.md:18-22, escrito 22:17) dice "op_32 NO existe" — eso era cierto a
> esa hora; a la hora de este diseño el working tree YA contiene
> `backend/math-engine/src/operators/op_32_multi_objective.rs` (1036 líneas,
> untracked) + registro en `mod.rs:43,163-164` + 2 tests de integración en
> `real_ops_tests.rs` (diff verificado). HP-03 aterrizó EN PARALELO a este diseño.
> No es contradicción: es la carrera documentada. **Consecuencia para mi WO**: el
> entregable no es un diseño en el vacío — es (a) el contrato matemático exacto
> que HP-03/HP-08 deben satisfacer, y (b) la auditoría adversarial de la
> implementación que ya existe, con hallazgos que HP-03 DEBE cerrar (uno BLOCKER:
> los property tests no compilan).
>
> Clasificación de afirmaciones: CANONICAL_REPO (file:line verificado hoy),
> PRIMARY_SOURCE (arXiv citado del canon local), INFERRED (derivación propia
> abajo), HYPOTHESIS (marcada). Lexicon OMEGA en todo el texto.

---

## 0. Veredicto ejecutivo

| # | Hallazgo | Severidad | Evidencia |
|---|---|---|---|
| F1 | Los 6 property tests declarados por HP-03 **NO COMPILAN** (E0631 ×2, `op_32_multi_objective.rs:974-975`) → nunca han corrido | **BLOCKER** | §7.1, salida cargo |
| F2 | `dominates` con NaN admite **ciclos de dominancia** (contraejemplo 3D verificado numéricamente) y deja que un vector con NaN **domine** a uno finito — contradice su propio doc-comment | HIGH | §3.1, §7.2 |
| F3 | Artefacto de gas en el plan nulo: `f1(x=0)=+gas` reporta `Some(−gas)` para un plan que jamás se transmite (fabricación bajo RULE 00); colapsa el frente cuando todo candidato es no-rentable | HIGH | §2.3, §7.3 |
| F4 | Overflow → objetivos ±∞ no filtrados: solo la fila SELECCIONADA se valida finita (`:704`); `matrix_result` puede llevar filas ±∞ pese al claim "finitos por construcción" | MEDIUM | §7.4 |
| F5 | Wiring Matrix: `topology_map.rs:20 COLS=31` (la Master Matrix sigue 264×31) + doc stale `mod.rs:90` "(1-31)" | MEDIUM | §7.5 (ya flaggeado por HP-04-APPLY.md:136-141; confirmado abierto) |
| F6 | Fee fallback `0.003` hardcodeado (`:459-466`, espeja op_15) — doctrina dice fees SIEMPRE on-chain | LOW-MED | §7.6 |
| F7 | Degeneración yield-only delega en op_15 (1 pool) ≠ argmax del propio f1 n-pool separable (waterfilling) — semántica aceptada CON documentación | LOW | §5.2 |
| F8 | Allocs por generación (clones de filas, `cols`, `front_objs`) — el charter pide "sin heap alloc por generación si es posible"; spec de arena incluida | LOW | §4.4 |

La matemática del NÚCLEO (sort no-dominado, crowding, SBX, mutación polinomial,
elitismo μ+λ, selección por preferencias, determinismo, presupuesto) es
**correcta en su forma canónica** (Deb et al. 2002) y está bien dimensionalizada
al hot-path con los defaults. Los hallazgos son de contratos de borde (NaN,
overflow, plan nulo) y de verificación (F1). La elección de NSGA-II está
respaldada por producción mundial: solvers de CoW usan NSGA-II bajo deadline de
subasta (arXiv:2510.21647, `world/quant-math/FINDINGS.md:50`).

---

## 1. Formalización del problema (derivación propia)

CANONICAL_REPO para las estructuras (`operators/mod.rs:52-106`); INFERRED para
todo lo derivado en esta sección.

### 1.1 Espacio de decisión

Sea `n = |pools usables| ≤ MAX_POOLS = 8` (`op_32_multi_objective.rs:53`). El
vector de decisión es la asignación de capital por Variedad de Liquidez:

```
x = (x_1..x_n) ∈ Ω = Π_i [0, r0_i]
```

donde `(r0_i, r1_i)` son las reservas de la venue i (validadas finitas > 0,
`:551-556`). Bounds de caja por venue = la reserva propia de entrada —
convención idéntica al bracket de op_15 (`op_15_golden_section.rs:114-115`).

### 1.2 Los tres objetivos (todos se MINIMIZAN)

**f1 — −Topological Yield neto (fricción termodinámica incluida):**

```
Y(x)   = Σ_i [ r1_i·γ·x_i / (r0_i + γ·x_i) − x_i ]
f1(x)  = −Y(x) + gas·1{x ≠ 0}        ← ver F3: el indicador es el CONTRATO, la
                                        implementación actual usa gas sin indicador
gas    = gas_price_gwei · 21000 · 1e−9 · p_ref     (token1; `:621`, = op_15:102)
γ      = 1 − fee,  fee = fee_bps/1e4 | pool_fee    (`:459-466`, = op_15:43-50)
```

Cada sumando es el output AMM de curva de producto constante con fee en entrada
(UniV2-style `getAmountOut`), menos el input. **Lema 1 (concavidad estricta)**:
`g_i(x) = r1_i·γ·x/(r0_i+γ·x) − x` tiene `g_i''(x) = −2·r1_i·γ²·r0_i/(r0_i+γ·x)³ < 0`
para todo x ≥ 0 (denominador > 0 pues r0_i > 0, γ > 0). Por lo tanto `Y` es
estrictamente cóncva y separable en Ω, `f1` es convexa como minimización.
*Consecuencia*: el problema yield-only tiene óptimo único computable por
condiciones de primer orden (ver §5.2 waterfilling) — la degeneración NO
requiere un algoritmo genético.

**f2 — CVaR_α de la Decoherencia de Estado por pata activa:**

```
δ_i(x) = γ·x_i / (r0_i + γ·x_i)           (decoherencia relativa de la pata i, ∈ [0,1))
k(x)   = #{i : x_i > 0}                    (patas activas)
t      = clamp(⌈α·k⌉, 1, k)
f2(x)  = (1/t) · Σ_{peores t} δ_i(x)       (media de las t PEORES patas)
```

Es el supercuantil discreto (upper-CVaR) de la distribución empírica equiprobable
{δ_i}₁^k con átomos 1/k. Familia monótona en α: α→0⁺ da t=1 (peor pata = max),
α=1 da t=k (media). Bounded en [0,1) — sin colas infinitas. **Lema 2
(monotonía)**: si x ≤ x' componente a componente entonces δ_i(x) ≤ δ_i(x') ∀i
(δ es estrictamente creciente en x_i), luego los órdenes componentwise de los
vectores ordenados se preservan y la media de las t peores es ≤ — i.e. `f2` es
monótona no-decreciente en x. *Nota semántica (INFERRED, documentar)*: comparar
CVaR entre individuos con k distinto (1 pata vs 3) compara distribuciones con
distinto soporte — es un proxy de riesgo de ejecución concentrado, no un CVaR
monetario; unidades: f1 en token1, f2 adimensional, f3 en ms. La normalización
min-max de §3.5 es lo que vuelve comparable la suma ponderada.

**f3 — latencia de inclusión:**

```
f3(x) = k(x) · ℓ,   ℓ = mo_per_leg_latency_ms | block_time_sec·1000 | 12000 ms  (`:626-629`)
```

Toma a lo más n+1 valores discretos (plateaus) — edge E8. La latencia como
objetivo de primera clase está respaldada por el canon: el delay de ejecución
degrada Kelly incluso sin costos de transacción (arXiv:1907.08771,
`FINDINGS.md:32`).

### 1.3 Por qué NSGA-II y no escalarización a priori

Una suma ponderada fija `min Σ w_j f_j` NO puede representar frentes no-convexos
(los óptimos de pesos positivos cubren solo la envolvente convexa del frente) y
obliga a fijar w ANTES de conocer el trade-off. El patrón de producción mundial
para el mismo shape de problema (deadline de subasta, 3 objetivos, población
pequeña) es exactamente NSGA-II dentro del solver de CoW (arXiv:2510.21647,
`FINDINGS.md:50`: "NSGA-II multi-objective user-surplus maximization inside
auction deadlines"). La escalarización se aplica DESPUÉS, SOBRE el frente
enumerado (§3.5) — esa es la propiedad "nunca fabrica": la preferencia elige
entre planes reales evaluados, no sintetiza un plan nuevo.

---

## 2. Contrato matemático del algoritmo (lo que HP-03 debe implementar)

Todas las referencias de línea son a `op_32_multi_objective.rs` actual (la
implementación de HP-03); el contrato es lo que DIGO que debe ser, y §7 audita
la brecha.

### 2.1 Dominancia de Pareto (estricta) — contrato NaN

**Definición** (minimización): `a ≺ b ⟺ (∀j: a_j ≤ b_j) ∧ (∃j: a_j < b_j)`.
Débil: `a ⪯ b ⟺ ∀j: a_j ≤ b_j` (permite iguales). El sort usa la estricta.

**Propiedades** (en ℝ^m finito): irreflexiva, asimétrica, transitiva ⇒ orden
parcial estricto ⇒ DAG de dominancia ⇒ **todo conjunto finito no-vacío tiene
elemento minimal** (siguiendo cualquier cadena dominante se termina por
asimetría) ⇒ `fronts[0]` nunca es vacío. **Esta teorema REQUIERE el contrato
NaN**:

```
// HP-02 (2026-09-08) — contrato: componente no-finita ⇒ NO dominancia (ninguna dirección)
fn dominates(a: &[f64], b: &[f64]) -> bool {
    let mut any_less = false;
    for (&x, &y) in a.iter().zip(b.iter()) {
        if !x.is_finite() || !y.is_finite() { return false; }  // ← LÍNEA REQUERIDA
        if x > y { return false; }
        if x < y { any_less = true; }
    }
    any_less
}
```

**Por qué (contraejemplo verificado numéricamente hoy, no hipotético)**: bajo la
semántica IEEE-754 (toda comparación con NaN es false) SIN la línea requerida,
la relación implementada (`:96-108`) admite el CICLO:

```
a = [NaN, 0, 5]  ≺  b = [1, NaN, 6]  ≺  c = [2, −1, NaN]  ≺  a     (las tres TRUE)
```

y además `d = [NaN, 1] ≺ e = [0.5, 2]` es TRUE (un vector con NaN DOMINA a uno
finito) mientras `e ≺ d` es false. Un ciclo rompe la prueba de que `fronts[0]`
es no-vacío: con dom_count > 0 para todos, `fronts[0] = []`, el loop de pelado
`:137` no arranca y el run degeneraría (hoy cae al `empty_front` honesto por
accidente, pero el doc-comment `:112-113` "NaN ⇒ no-dominancia ⇒ cae al frente 0"
es FALSO en ambas mitades: cae al frente 0 pero POR dominar, no por
no-dominar). Verificación ejecutada: réplica exacta de la semántica Rust en
Python, salida `True/True/True` para el ciclo y `True/False` para el par
asimétrico; con la línea requerida el ciclo colapsa a `False/False/False`.

**Segunda capa (obligatoria)**: los vectores de objetivos no-finitos (NaN o ±∞)
se FILTRAN antes del sort — son "no computado" bajo R8, se emiten como
`reason_non_finite_objectives` y no participan del frente. El filtro es la
defensa real (el contrato de `dominates` es defensa en profundidad). Hoy
`evaluate_objectives` promete finitud "por construcción" (`:225-226`) — esa
promesa tiene un agujero de overflow (F4, §7.4) — y solo la fila seleccionada se
valida (`:704`).

### 2.2 Fast non-dominated sort (Deb 2002)

```
S_p = {q : p ≺ q};  n_p = |{q : q ≺ p}|
F_0 = {p : n_p = 0}
para i = 0,1,2…:  para p ∈ F_i, q ∈ S_p: n_q −= 1; si n_q = 0 → F_{i+1}
```

Complejidad O(M·N²) (M=3 objetivos, N = |población|). Con el contrato §2.1 la
terminación y la no-vacuidad de F_0 son teorema; sin él son suerte. La
implementación `:114-154` es canónica y determinista (recorrido por índice).
**Contrato de duplicados**: dos vectores de objetivos idénticos NUNCA se
dominan entre sí (la estricta exige un <) → coexisten en el mismo frente → el
crowding con span 0 les da contribución 0 → el truncado del elitismo los poda
primero. Ese es el manejo de duplicados correcto y YA está en la implementación.

### 2.3 Crowding distance

```
para cada frente F, |F| = m:
  si m ≤ 2:  d_i = +∞  ∀i                                  (todos son frontera)
  si no, para cada objetivo j:
    ordenar F por f_j (total_cmp — orden total determinista)
    d del primero y del último por f_j := +∞
    span_j = max − min;  si span_j = 0 ó no finito: contribución 0 (sin div/0)
    interior: d_k += (f_j[k+1] − f_j[k−1]) / span_j  (solo si d_k finito)
```

Convenciones exactas (la implementación `:161-187` las cumple): frontera en
CUALQUIER objetivo ⇒ ∞ (asignación, no máximo); objetivo degenerado (span 0)
aporta 0 — correcto porque un eje donde todos empatan no informa diversidad.
Crowding debe computarse sobre los OBJETIVOS, no sobre x.

### 2.4 SBX + mutación polinomial (contrato de distribución)

**SBX** (Deb & Agrawal 1995), por par, con p_c = 0.9, η = 15, un solo draw de
decisión por par y un u por gen:

```
u ≤ 0.5:  β = (2u)^(1/(η+1))        u > 0.5:  β = (1/(2(1−u)))^(1/(η+1))
c1 = ½[(1+β)p1 + (1−β)p2]           c2 = ½[(1−β)p1 + (1+β)p2]
clip(c, lo, hi)  — preserva bounds; rompe la propiedad de intercambio exacta (documentado)
```

Con `u ∈ [0,1)` (rand 0.8 `gen::<f64>`) no hay singularidad en u→1. Padres
idénticos ⇒ hijos = padres (edge E6: la mutación es la única fuente de
diversidad restante — estándar, documentar).

**Mutación polinomial** (Deb & Goyal 1996), p_m = 1/n por gen, η_m = 20:

```
u < 0.5:  δ = (2u)^(1/(η_m+1)) − 1     u ≥ 0.5:  δ = 1 − (2(1−u))^(1/(η_m+1))
x_i' = clip(x_i + δ·(hi−lo), lo, hi)
```

La implementación `:266-313` es canónica en ambas. La distribución es
auto-escalante por gen (δ·(hi−lo) y el spread SBX ∝ |p1−p2|) — soporta pools con
r0 de escalas dispares sin normalización previa.

### 2.5 Elitismo μ+λ (selección ambiental)

```
R = P ∪ Q (2μ);  sort no-dominado de R;  llenar F_1, F_2, … hasta μ;
si el último frente desborda: truncar por crowding DESCENDENTE (empates ⇒ índice menor)
```

Garantía (Deb 2002): elitista — el mejor rank nunca empeora entre generaciones;
convergencia asintótica al frente verdadero. **En presupuesto finito (hot-path)
el frente es APROXIMADO** — el contrato lo declara: `matrix_result` = frente
aproximado final, no el frente exacto del Ω continuo (que sería computable por
waterfilling en el caso separable, pero no en el multi-objetivo con f2/f3
discretas en tiempo hot-path razonable). Torneo binario con crowded-comparison:
rank asc, luego crowding desc, empate ⇒ regla fija sin moneda (`:317-323`) —
determinista.

### 2.6 Selección por vector de preferencias — la propiedad central

**Contrato**: dado el frente F (todas las filas finitas) y w ∈ ℝ³₊ con Σw = 1
(normalizado de `mo_weight_{yield,risk,latency}`, defaults 0.5/0.3/0.2 del SEED):

```
lo_j = min_F f_j,  hi_j = max_F f_j,  span_j = hi_j − lo_j
n_ij = (f_ij − lo_j)/span_j  si span_j > 0 finito;  si no 0
U_i  = Σ_j w_j · n_ij
sel  = argmin_i U_i   (empates ⇒ primer índice)
```

**Propiedad (charter, con demostración)**: `sel` SIEMPRE es un índice de F.
*Prueba*: U_i está definida para todo i ∈ F (finitud de filas; span 0 ⇒ término
0 incluso con w_j > 0); argmin sobre una enumeración finita no-vacía retorna un
índice del conjunto; el empate se resuelve por primer índice — no hay ruta de
escape fuera de F. ∎ **Corolario operativo**: `vector_result` debe ser
bit-idéntico a una fila de `matrix_result` (el test `:907-919` ya lo exige —
correcto). **Limitación declarada (no bug)**: la suma ponderada lineal sobre
normalización min-max no puede preferir tramos cóncavos del frente — garantiza
"el mejor miembro del frente bajo la utilidad declarada", NO "el plan
óptimo en Ω para esa utilidad". Peso cero ⇒ eje sin influencia (correcto:
si todos empatan o el peso es 0, el eje no debe decidir).

### 2.7 Determinismo / seed

```
seed = f(block_number, n, bits(w)) — wrapp_mult/mix/rotates  (`:657-663`)
rng  = SmallRng::seed_from_u64(seed)   (rand 0.8 + small_rng YA en Cargo.toml:26 — cero crates nuevas)
```

Mismo `MarketState` ⇒ mismo seed ⇒ mismo stream de draws ⇒ misma salida
bit-idéntica. Sin reloj (§Date.now prohibido — patrón op_22). HashMap solo se
accede por `.get()` — sin iteración dependiente del orden de hash. Todos los
sorts con `total_cmp` y estables para desempates. **Caveat documentado (no
bloqueante)**: `SmallRng` es Xoshiro256++ en 64-bit / Xoshiro128++ en 32-bit —
la reproducibilidad es por-plataforma-y-versión-de-rand, no universal; el VPS y
CI son x86_64 ⇒ el par (VPS, CI, dev) es coherente.

### 2.8 Presupuesto (§4.3 — milisegundos son millones)

```
pop  ∈ [4, 128]   (default 40)     generations ∈ [1, 100]  (default 24)
evaluaciones = pop·(generations + 1)   ≤ 128·101 = 12 928
costo sort: ~2·M·(2·pop)²/2 comparaciones de dominancia por generación
```

**Perfil hot-path (default 40×24)**: 1000 evaluaciones + ~460k comparaciones de
dominancia ≈ orden de 1-3 ms por run — cabe en el presupuesto por-stage (29 ms
del SEED-v2 / BR-08) con margen; el op corre en la capa EVALUATION (doctrina
ROUTES_CROWN_JEWEL "dos capas", nunca en discovery). **Perfil MAX (128×100)**:
~19.6M comparaciones — decenas de ms; DEBE etiquetarse offline/backtest: el
contrato exige que el DEFAULT sea el hot-path profile y que `evaluations` se
reporte en metadata (ya se hace, `:714`). Los loops son `for` acotados por
construcción — terminación garantizada sin recursión (test P7).

### 2.9 Fail-honest R8 — enumeración cerrada de salidas None

`reason_no_usable_pools` · `reason_too_many_pools` (n>8) ·
`reason_invalid_preference_vector` (peso <0, NaN, Σ=0) · `reason_invalid_fee`
(γ≤0) · `reason_invalid_price` · `reason_empty_front` ·
`reason_non_finite_objectives` · `reason_degenerate_null_trade`. Ningún otro
camino puede producir None. `Some(0.0)` es legal y honesto (computado y
exactamente cero — p.ej. yield del plan nulo BAJO el contrato F3). **Regla
frente-vacío**: `front0.is_empty()` ⇒ None declarado, JAMÁS fila fabricada ni
vector cero (`:668-670` cumple; con el contrato §2.1 ese camino queda como
defensa en profundidad, no como ruta esperada).

---

## 3. Firma exacta del trait y estructura de datos

### 3.1 Firma del trait (CANONICAL_REPO, ya registrada)

```rust
// mod.rs:89-106 — el trait NO cambia; op_32 es un implementador más
pub trait TopologicalOperator: Send + Sync {
    fn id(&self) -> u8;                    // op_32 → 32
    fn name(&self) -> &'static str;        // "Optimizacion Multi-Objetivo"
    fn category(&self) -> &'static str;    // "optimization"
    fn evaluate(&self, state: &MarketState) -> OperatorOutput;
    fn is_available(&self) -> bool { true }
}
// mod.rs:163-164 — registro
32 => Box::new(crate::operators::op_32_multi_objective::MultiObjectiveOperator::new()),
```

**Contrato del output** (integra el de `mod.rs:69-83`):

- `scalar_value = Some(net_yield_del_plan_seleccionado)` — negativo = computado
  y honestamente NO rentable (misma semántica que op_15:171). Bajo F3-fix, el
  plan nulo reporta `Some(0.0)`, no `Some(−gas)`.
- `vector_result = fila exacta (bit-idéntica) del plan seleccionado`.
- `matrix_result = frente aproximado`, cada fila:
  `[x_0..x_{n−1}, net_yield, cvar_riesgo, latency_ms]` — n+3 columnas,
  presentación ordenada lexicográfica por `total_cmp` (`:685-693`, determinista).
  **Regla de reconstrucción para los property tests**: el vector de
  minimización de la fila r es `[−r[n], r[n+1], r[n+2]]`.
- `metadata`: `computed`, `n_pools`, `population`, `generations_run`,
  `evaluations`, `front_size`, `selected_index`, `weight_{yield,risk,latency}`,
  `active_objectives`, `cvar_alpha`, `per_leg_latency_ms`, `gamma`, `gas_cost`,
  `nsga2_seed`, `net_yield`, `risk_cvar`, `latency_ms` + `reason_*` en los None.
  Se AGREGA (contrato F6): `fee_source` ∈ {1.0 = features/on-chain, 0.0 =
  fallback 0.003} para observabilidad de la doctrina de fees.

### 3.2 Estructura interna del frente — spec de arena (F8, para HP-03)

Contrato de estructura: **cero heap allocs por generación tras la primer
alocación del run**. Espec exacta:

```text
// HP-02 (2026-09-08) — spec de buffers (HP-03 aplica)
stride      = n + 3                        // x ‖ f1..f3
slab_x      = [f64; 2·pop·stride]          // población P (filas 0..pop) ‖ descendencia Q (pop..2·pop)
slab_rank   = [usize; 2·pop]
slab_dist   = [f64; 2·pop]
slab_front  = [usize; 2·pop]               // frente 0 compacto (índices)
sort_idx    = [usize; pop]                 // reutilizado por objetivo en crowding
```

Ping-pong P/Q por slices del slab (sin `clone()` de filas — hoy `:380,429-434`
clona `Vec<Vec<f64>>` por generación; con pop=40, n=8 son ~2k clones ≈ 180KB de
churn por run: no rompe corrección, viola la disciplina §4.3). La presentación
final (`matrix_result`) sí alocará una vez, al borde del run — legítimo.

---

## 4. Invariantes de los property tests (contrato EXACTO de HP-03)

Los 6 del charter + R8, formalizados. Cada uno con el invariante y el oráculo.
HP-03 DEBE hacerlos compilar (hoy no compilan, F1) y pasar; HP-08 los re-corre.
Se exige extender de fixture-único a **propiedad randomizada con SmallRng
sembrado** (≥100 casos por propiedad, seeds fijas en el test) — un fixture solo
es un ejemplo, no una propiedad.

**P1 — No-dominancia por pares del frente retornado.**
∀ i≠j filas de `matrix_result`: ¬(a_i ≺ a_j) ∧ ¬(a_j ≺ a_i), con a = [−r[n],
r[n+1], r[n+2]] y la `dominates` del CONTRATO §2.1 (no la del bug). Oráculo:
fuerza bruta O(m²) con la misma definición. Randomizado sobre estados
generados (reservas log-uniformes [1e2, 1e9], fees [0, 300] bps, pesos
Dirichlet-discretizados, gas [1, 200] gwei).

**P2 — Sort no-dominado exacto vs fuerza bruta.**
Para N ≤ 12 objetivos aleatorios (M=3, con duplicados y con colineales):
F_0 = conjunto minimal por fuerza bruta; y la PROFUNDIDAD de cada elemento
(rankeado en F_k) coincide con la longitud máxima de cadena dominante que
termina en él ± definición estándar. El test actual (`:822-845`) solo cubre
F_0 de un fixture — extender.

**P3 — Crowding distance correcto.**
Fronteras de cada objetivo = ∞; interior = Σ (vecino⁺ − vecino⁻)/span por
objetivo; m ≤ 2 ⇒ todos ∞; objetivo con span 0 ⇒ contribución 0 y SIN pánico
de división; duplicados ⇒ distancia finita baja (podados primero por el
truncado). El fixture actual (`:849-871`) es correcto — mantener y añadir caso
randomizado.

**P4 — Preferencia selecciona DEL frente (nunca fabrica).**
(a) `selected_index < |matrix_result|`; (b) `vector_result` bit-idéntico a una
fila; (c) pesos puros [1,0,0]/[0,1,0]/[0,0,1] seleccionan el argmin de
f1/f2/f3 respectivamente (el test actual cubre yield y riesgo/latencia sobre
un frente 3D fijo — mantener); (d) pesos [0,0,0] / negativos / NaN ⇒ None con
`reason_invalid_preference_vector`.

**P5 — Degeneración a 1 objetivo.**
w = [1,0,0] ⇒ scalar bit-idéntico al de op_15 sobre el MISMO estado
(delegación exacta, metadata `delegated_to_op=15`, `degenerate_pools_ignored`
si n>1 — semántica del §5.2 documentada). w = [0,1,0] ó [0,0,1] ⇒ None
`reason_degenerate_null_trade` — JUSTIFICADO por el Lema 2 (f2, f3 monótonas
no-decrecientes ⇒ x=0 es óptimo único).

**P6 — Determinismo con seed.**
Mismo estado ⇒ scalar/vector/matrix/metadata numéricos bit-idénticos (con
`f64::to_bits`), incluido `nsga2_seed`. El test existe (`:950-976`) — el fix
F1 lo revive.

**P7 — Presupuesto acotado y terminación.**
Features absurdas (`mo_population=1e9`, `mo_generations=1e9`) ⇒ clamps
[4,128]×[1,100]; `evaluations == pop·(gens+1)` reportado; el propio test que
termina ES la prueba de terminación. Ya presente (`:980-999`).

**P8 — R8 enumeración cerrada.**
Cada reason_* del §2.9 alcanzado por un estado construido a propósito;
ningún otro camino produce None; `Some(0.0)` permitido; NINGÚN None se
convierte en fila cero. Ya presente (`:1003-1026`) — extender con el caso
`reason_non_finite_objectives` del contrato F4 (reservas ~1e200).

---

## 5. Degeneraciones y casos donde NSGA-II clásico muere (edges adversariales)

### 5.1 Degeneración a un objetivo — lemas de justificación

**(i) yield-only**: por Lema 1, f1 convexa separable ⇒ el óptimo del propio
problema es waterfilling marginal:
`d g_i/dx = r1_i·γ·r0_i/(r0_i+γ·x)² − 1 = 0 ⇒ x_i* = r0_i·(√(r1_i·γ) − 1)/γ`
(clampeado a [0, r0_i], solo si r1_i·γ > 1), asignando por retorno marginal
decreciente. INFERRED. **Decisión de diseño (F7)**: la implementación delega
en op_15 (pool primario) en lugar de waterfill — acepto la delegación como
CONVENIO DE COHERENCIA CANÓNICA (op_32 con w=[1,0,0] ≡ op_15, cero matemática
nueva, metadata honesta `degenerate_pools_ignored`), y documento que NO es el
argmax del f1 n-pool del propio módulo. Si el BOARD prefiere coherencia
interna, el waterfill de arriba es el contrato alternativo exacto (una línea
cerrada por pool). NO bloqueante.

**(ii) risk-only / latency-only**: por Lema 2 (f2 monótona no-decreciente) y
monotonía trivial de f3 (= k·ℓ), x=0 es el minimizador único ⇒ no hay plan que
transmitir ⇒ None `reason_degenerate_null_trade`. La implementación
short-circuita analíticamente (`:608`) — CORRECTO y justificado por el lema
(no es un atajo arbitrario).

### 5.2 Lista E1-E12 (edges matemáticos adversariales)

| # | Edge | Qué degenera | Contrato |
|---|---|---|---|
| E1 | NaN en fitness | Dominancia deja de ser orden parcial (ciclo verificado §2.1); NaN "domina" | `dominates` false si algún componente no-finito + filtro pre-sort con `reason_non_finite_objectives` |
| E2 | Overflow → ±∞ (reservas ~1e200) | `r1·γ·x` → inf; ±∞ participa del sort "ganándolo todo" | Mismo filtro E1; NO confiar en "finito por construcción" |
| E3 | Objetivos colineales (f2 ∝ f3 cuando ℓ domina) | Frente degenera a pocos puntos; crowding con span 0 | Ya manejado (contribución 0); selección invariante (normalizado 0) |
| E4 | Frente de 1 punto | Crowding ∞ trivial; selección índice 0 | Legal: `matrix_result` de 1 fila; smoke 1-pool ya existe (`:1030-1035`) |
| E5 | Duplicados exactos | Estricta dominancia no los separa; inflan el frente | Coexisten en el frente; truncado por crowding los poda primero (§2.2) — correcto |
| E6 | Padres idénticos en SBX | Hijos = padres; estancamiento | Mutación polinomial es la fuente de diversidad restante — estándar, documentado |
| E7 | Todo candidato no-rentable | Hoy: frente colapsa a {x=0} y scalar = Some(−gas) — gas cobrado a un plan que no se transmite | F3-fix: `f1 = −Y + gas·1{x≠0}`; null → `Some(0.0)` honesto |
| E8 | f3 discreta (n+1 valores) | Plateaus, empates de rank masivos | Crowding + desempate por índice estable — determinista |
| E9 | Bounds con escalas dispares (r0 1e2 vs 1e9) | SBX/mutación en unidades absolutas | Auto-escalantes por gen (δ·(hi−lo), spread ∝ \|p1−p2\|) — sin normalización global requerida |
| E10 | α de CVaR degenerado | α→0⁺ = peor pata (max); α=1 = media | Familia monótona, clamp [1e−9, 1], tail = clamp(⌈αk⌉,1,k) — ya correcto (`:247`) |
| E11 | Pesos degenerados (Σ=0, negativos, NaN) | Normalización divide por 0 / propaga NaN | Rechazo fail-honest `reason_invalid_preference_vector` (`:576-582`) — ya correcto |
| E12 | k=0 dentro de un plan mixto evaluado (x=0 en la población) | CVaR de soporte vacío | `k==0 ⇒ f2=0` (`:244-245`) — definido, no NaN; E7 gobierna su costo |

---

## 6. Modo-invariancia y doctrina

- **§34.1 mode-invariant**: el operador es una función pura de `MarketState` →
  `OperatorOutput`; no lee flags de modo, no toca executor/capital/broadcast.
  La misma matemática en PAPER/TESTNET/LIVE. ✓ (CANONICAL_REPO por lectura del
  archivo completo.)
- **Fees on-chain** (docs/ROUTES_CROWN_JEWEL_DOCTRINE.md:53-54 "fees SIEMPRE
  leídos de la cadena, jamás se hardcodean"): el CANAL canónico es
  `features["fee_bps"]` (productor: datos on-chain). El fallback 0.003
  (`:459-466`) hereda la convención LEGACY de op_15 (`op_15:41-50`) — en
  hot-path el productor DEBE pasar fee_bps; el contrato añade metadata
  `fee_source` para que un run con fallback sea VISIBLE (F6).
- **Dos capas discovery≠evaluation**: op_32 vive en EVALUATION (sizing/ranking
  de planes ya descubiertos) — coincide con el rol `USES_SECONDARY` que HP-04
  asignó a los 182 edges (HP-04-APPLY.md:34-37).

---

## 7. Auditoría adversarial de la implementación HP-03 (working tree)

La implementación existe (1036 líneas) y registra bien el trait. Auditoría
matemática línea a línea contra el contrato de §2:

### 7.1 F1 [BLOCKER] — los property tests NO COMPILAN

`cargo test -p math-engine --lib op_32` → E0631 ×2:

```text
error[E0631]: type mismatch in function arguments
  --> math-engine\src\operators\op_32_multi_objective.rs:974:45 / 975:45
    a.metadata.get("net_yield").map(f64::to_bits),
                                   ^^^^^^^^^^^^ expected &f64, f64::to_bits takes f64
```

`Option::<&f64>::map` recibe `&f64`; `f64::to_bits` como path de función exige
`f64` por valor. Es exactamente el gotcha del ledger ("cargo check ≠ compila
tests", PR #460): `cargo check -p math-engine` PASA (verificado hoy, 11.4s
tras lock) y los 6 tests declarados NUNCA han corrido. **Diff exacto
(requerido por HP-03, 2 líneas)**:

```rust
// HP-02 (2026-09-08) — fix mínimo F1 (aplica HP-03)
-            a.metadata.get("net_yield").map(f64::to_bits),
-            b.metadata.get("net_yield").map(f64::to_bits)
+            a.metadata.get("net_yield").map(|v| v.to_bits()),
+            b.metadata.get("net_yield").map(|v| v.to_bits())
```

Nota: la misma expresión en `:956` (`a.scalar_value.map(f64::to_bits)`) SÍ
compila porque `scalar_value: Option<f64>` entrega `f64` por valor — por eso el
error aparece solo en metadata.

### 7.2 F2 [HIGH] — semántica NaN de `dominates` contradice su contrato

Detalle completo en §2.1: doc-comment `:112-113` falso, ciclo de dominancia
verificado numéricamente, asimetría NaN-domina-finito. El camino in-vivo hoy
está cerrado por la finitud casi-siempre de `evaluate_objectives`, pero (a) el
módulo exporta el contrato de "defensivo" que no cumple, (b) F4 abre la puerta
de ±∞, y (c) P2/P1 usan `dominates` como oráculo — un oráculo con la misma
ceguera que el código bajo test no valida nada. **Diff requerido**: insertar la
línea `if !x.is_finite() || !y.is_finite() { return false; }` en `:99` + filtro
pre-sort de filas no-finitas con `reason_non_finite_objectives`.

### 7.3 F3 [HIGH] — artefacto de gas del plan nulo

`f1(x=0) = +gas` (`:236,261` — el término gas es incondicional). El plan nulo
(“no comerciar”) no se transmite ⇒ su costo físico es 0; cobrarle gas es un
dato fabricado (tensión RULE 00). **Alcance demostrado**: cuando todos los
candidatos son no-rentables (Y(x) < 0 ∀x≠0), f1(0) = gas < gas + pérdida =
f1(trade) y f2(0)=f3(0)=0 mínimos ⇒ el nulo DOMINA todo ⇒ frente = {nulo} ⇒
scalar = Some(−gas): el tablero leería "pérdida del gas" donde la verdad es
"no existe plan rentable; no transmitir (costo 0)". También alcanzable con
pesos riesgo/latencia-dominantes (w_y > EPS pero pequeño). **Diff de contrato**
(elige HP-03 una de dos, recomendada la primera):

```rust
// HP-02 (2026-09-08) — F3 opción A (recomendada): gas solo si hay transmisión
-    vec![-gross + ctx.gas, risk, latency]
+    let broadcast = k > 0;                       // plan nulo no se transmite
+    vec![-gross + if broadcast { ctx.gas } else { 0.0 }, risk, latency]
// consecuencia: plan nulo = (0,0,0); con candidatos rentables el frente gana
// la tensión correcta (el nulo ya NO es dominado en f1 por nadie); el nulo
// seleccionado reporta Some(0.0) — R8 honesto ("computado y exactamente cero").
// HP-02 (2026-09-08) — F3 opción B (parche mínimo): declarar la no-decisión
+    if k == 0 { return Self::none_out("null_plan_selected"); }  // tras selection
```

### 7.4 F4 [MEDIUM] — promesa de finitud con agujero de overflow

`evaluate_objectives` documenta "Devuelve [f1,f2,f3] finitos" (`:225-226`) —
cierto para NaN pero falso para ±∞: `r1·γ·x` con reservas del orden de 1e200
(productos finitos que exceden 1.8e308) hacen `gross = ±inf`, y SOLO la fila
seleccionada se chequea (`:704`): `matrix_result` puede emitir filas ±∞ y el
sort de §2.2 las trata como "ganadoras" de f1. Filtro requerido en el borde de
población (ver E1/E2). Los bounds del dominio no previenen overflow del
producto.

### 7.5 F5 [MEDIUM] — wiring de la Master Matrix abierto

`matrix/topology_map.rs:20 COLS: usize = 31` y `:33` doc "264x31" — la matriz
sigue 264×31; el grafo ya tiene 182 edges op_32 (HP-04). HP-04-APPLY.md:136-141
asignó este flip a HP-03 — **sigue abierto** (verificado hoy). También doc
stale: `mod.rs:90` "ID único del operador (1-31)". Ambos diffs son de HP-03
(COLS 31→32 + doc), NO míos (no edito producción).

### 7.6 F6 [LOW-MED] — fee fallback y observabilidad

`fee_fraction` (`:459-466`) degrada a 0.003 sin `fee_bps`/`pool_fee`. Herencia
de op_15 — pero la doctrina manda fees on-chain siempre. Contrato mínimo:
metadata `fee_source` (1.0 = features, 0.0 = fallback) para que un run
degradado sea VISIBLE en observabilidad; el productor hot-path DEBE pasar
fee_bps leído de la Variedad de Liquidez real.

### 7.7 Lo que está BIEN (verificado, para que HP-08 no lo re-audite de cero)

- `fast_non_dominated_sort` `:114-154` canónico, determinista, O(M N²).
- `crowding_distance` `:161-187`: convenciones correctas incl. m≤2 ⇒ ∞, span 0
  ⇒ 0, guard de finitos en la acumulación, `total_cmp`.
- `sbx_crossover` `:266-293` y `polynomial_mutation` `:296-313`: formas
  canónicas Deb (η=15/η_m=20, p_c=0.9, p_m=1/n), sin singularidades con
  u ∈ [0,1), clipping preserva bounds.
- Elitismo `:413-450`: μ+λ correcto, truncado por crowding desc estable.
- `select_by_preference` `:193-223`: argmin sobre enumeración ⇒ propiedad
  "siempre del frente" estructural (§2.6); span 0 ⇒ término 0.
- Seed determinista sin reloj `:657-663`; HashMap solo por `get`.
- Presupuesto: clamps [4,128]×[1,100], `evaluations` en metadata.
- CVaR de buffer de pila sin heap (`:231-258`), tail clamp correcto.
- Degeneraciones: delegación op_15 exacta + None analítico justificado por
  monotonía (Lema 2).
- R8: enumeración de reasons cerrada; registry integration tests añadidos en
  `real_ops_tests.rs` (dispatch 32 + smoke 1..=32 fail-honest).

---

## 8. GATE para HP-03/HP-08 (criterios de aceptación)

1. **G1 (compilación de tests)**: `cargo test -p math-engine --lib op_32`
   compila y PASA (hoy: BLOCKER F1). Clippy/fmt verde (`cargo clippy -p
   math-engine -- -D warnings`, `cargo fmt -p math-engine --check`).
2. **G2 (contrato NaN/∞)**: `dominates` retorna false con componente
   no-finita; el test del ciclo §2.1 (a=[NaN,0,5], b=[1,NaN,6], c=[2,−1,NaN])
   es un test REGRESIÓN nombrado (ninguna de las tres direcciones domina);
   filas no-finitas filtradas con `reason_non_finite_objectives` y un test con
   reservas 1e200 lo prueba.
3. **G3 (plan nulo verídico)**: con todos los candidatos no-rentables, el
   scalar del nulo NO es Some(−gas) (opción A: Some(0.0); opción B: None
   `reason_null_plan_selected`). Test dedicado.
4. **G4 (propiedades randomizadas)**: P1-P8 con ≥100 casos seed-fijos (no solo
   fixtures).
5. **G5 (wiring Matrix)**: `topology_map.rs` COLS=32 + doc `mod.rs:90`
   actualizada + smoke de TopologyMap compila (o BLOCKER declarado al BOARD si
   hay dependencias fuera de claim).
6. **G6 (presupuesto medido)**: un test de humo de latencia del perfil default
   (40×24) corre en < 10 ms de CPU en dev (orden de magnitud, no CI-gate
   estricto — el número duro lo mide HP-08 en el VPS).
7. **G7 (observabilidad de fees)**: metadata `fee_source` presente.

---

## 9. Reporte al BOARD (decisiones que exceden mi claim)

1. **HP-03 aterrizó ANTES que el diseño** (carrera paralela documentada). Sus
   property tests no compilan (F1) — el charter de HP-03 exige tests que
   corran: necesita un segundo pase mínimo (F1 2 líneas + idealmente F2/F3).
   Recomiendo que el orquestador re-dispare HP-03 con este documento como
   insumo directo: §4 invariante por invariante, §8 gate por gate.
2. **F3 opción A vs B** es una decisión de SEMÁNTICA del escalar (yield del
   plan vs yield de la decisión) — la recomiendo A, pero si HP-05 (UI de
   preferencias) ya diseñó contra "scalar = yield del plan", B es el parche
   coherente. HP-05 debe pronunciarse (HP-06-DESIGN.md no toca este punto).
3. **COLS=32** (F5) toca `matrix/topology_map.rs`, fuera del claim declarado
   de HP-02/HP-03 según el charter original — el BOARD debe adjudicarlo
   explícitamente (HP-04-APPLY.md:136-141 lo presupone en HP-03).
4. La extensión waterfilling (§5.1-i) queda como OPCIÓN documentada, no
   requisito — la delegación op_15 es coherencia canónica válida con metadata
   honesta.

## 10. Doctrina cumplida

- RULE 00: cero datos fabricados — el hallazgo F3 ES una acusación de dato
  fabricado (gas al plan no transmitido) con diff de corrección; este diseño no
  fabrica nada: el contraejemplo NaN fue VERIFICADO numéricamente antes de
  afirmarlo.
- §32/§33: read-only total — solo leí código de producción, ejecuté cargo
  check/test (verificación local, NO deploy) y un script Python de verificación
  matemática. Cero executor/capital/broadcast. VPS ni tocado.
- §34.3: nada de esto toca terminus de ejecución. §34.1: diseño mode-invariant
  por construcción (§6).
- NO-GIT: cero commit/push/PR. Único archivo escrito: este reporte.
- X10: verifiqué (cargo check PASS, cargo test FAIL documentado con errores
  exactos, contraejemplo ejecutado) antes de concluir; clasifiqué cada
  afirmación CANONICAL_REPO / PRIMARY_SOURCE / INFERRED / HYPOTHESIS.

— HP-02 (math-validator), 2026-09-08. // HP-02 (2026-09-08)
