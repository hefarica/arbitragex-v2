# HP-03 — CROSS-EXAMINATION (par adversarial, Gang Omniscience 2026-09-08)

**Objeto**: refutar el entregable de HP-03-APPLY.md (op_32 NSGA-II en math-engine).
**Método**: verificación re-ejecuta, no hereda — leí el 100% de
`op_32_multi_objective.rs` (1,037 líneas) + diffs `mod.rs`/`real_ops_tests.rs` +
`Cargo.toml`; re-ejecuté `cargo check -p math-engine` (PASS, 9m49s tras lock) y
repliqué NUMÉRICAMENTE (Python independiente, `py`) la semántica exacta de
`dominates`/`evaluate_objectives`/`select_by_preference` para los hallazgos que
impugnan. Leí el board completo + HP-01/HP-02/HP-04/HP-08 (ambas mitades).

## 1. Veredicto: **GAPS** (trabajo sólido en SU fila; cierre de mesa incompleto)

HP-03 cumple su charter LITERAL (GOAL-WORKORDERS.md:31): op_32 + registro + los 6
property tests + registry verde + check/clippy/fmt + tests vía exe (118/118
re-ejecutado por HP-08-A §1.9 con mismo hash de exe; mi `cargo check` PASS).
Higiene verificada por mí: diff exacto `mod.rs +8/−3`, `real_ops_tests.rs +69/−0`,
`op_32` untracked, `Cargo.toml` SIN cambios (mtime Aug 16, `rand/small_rng`
preexistente línea 26), `lib.rs` intacto (sin diff), marcadores `// HP-03
(2026-09-08)` en los 3 archivos, smoke 1..=31 (real_ops_tests.rs:416) intacto.

**PERO** el entregable deja ABIERTOS los hallazgos adversariales de su par de
diseño — **HP-02-DESIGN.md aterrizó 23:05 y el reporte de HP-03 (23:16) aún dice
"NUNCA aterrizó"** (verificado con ls a las 22:49) — y dos de ellos son
refutación-grade contra el claim de R8/RULE 00 del propio HP-03. La contradicción
de mesa HP-02 (F2/F3/F4 ABIERTOS) vs HP-08-A ("HP-03 PASS, 0 bloqueantes",
emitido 23:28 — DESPUÉS de HP-02) quedó sin reconciliar: HP-08-A verificó contra
el charter original, no contra los gates G1-G7 que HP-02 §8 dirigió a
"HP-03/HP-08".

## 2. Hallazgos refutación-grade (verificados por mí, no heredados)

### X-1 [HIGH] — F3 de HP-02 CONFIRMADO end-to-end: gas cobrado al plan nulo
`op_32:261` aplica `+ ctx.gas` INCONDICIONAL: `vec![-gross + ctx.gas, risk, latency]`.
El plan nulo (sembrado en la población inicial, `:343`) NO se transmite ⇒ su costo
físico es 0, pero el código le asigna f1=+gas. Réplica numérica mía (fee 30bps,
edge 0.01%, gas 20 gwei): f1(null)=+0.0004242, f2=f3=0 ⇒ **el nulo domina todo
candidato no-rentable** ⇒ en un estado sin candidatos rentables el frente colapsa
a {nulo} y el escalar retornado es `Some(−0.0004242)` con `computed=1.0` — el
tablero lee "pérdida del gas" donde la verdad es "no hay plan rentable; no
transmitir (costo 0)". Tensión RULE 00 nombrada por el diseñador (HP-02 §7.3,
opciones A/B pre-diseñadas). HP-03 no aplicó ni respondió — no podía saberlo, pero
su reporte §2 sí ADJUDICA la semántica ("negativo = computado y honestamente NO
rentable, misma semántica que op_15") sin citar que su par de diseño la disputa.

### X-2 [HIGH] — F4 de HP-02 CONFIRMADO end-to-end: frente envenenado con ±∞
Solo la fila SELECCIONADA se valida finita (`:705-707`); `matrix_result` se
construye SIN filtro (`:676-694`). Réplica mía (reservas finitas 1e200/1.0001e200
— PASAN el filtro `:551-556` por ser finitas > 0): `r1·γ·x` desborda a +inf ⇒
fila full-focus f1=−inf. La fila −inf NO domina al nulo (su f2≈0.9 > 0) ⇒
**front0 = {nulo, filas −inf} coexistiendo**; en `select_by_preference` el span de
f1 = −inf−(−inf) = NaN ⇒ contribución 0 ⇒ U_nulo = 0 = mínimo ⇒ nulo seleccionado
(finito) ⇒ `:705` PASA ⇒ salida `computed=1.0`, escalar finito, y
**matrix_result con filas net_yield=+inf**. Output literal de mi réplica:
`utilities: {'null': 0.0, 'full': 0.2, 'half': 0.2} -> selected: null;
matrix_result rows carry net_yield: [-0.0004242, inf, inf]`. El doc-header
`:39-40` ("frente vacío tras filtrar no-finitos") promete un filtro que NO existe
— HP-08-A lo notó como R-2/LOW subestimando que el escenario es alcanzable de
 punta a punta y combina con X-1.

### X-3 [MEDIUM] — F2 de HP-02 CONFIRMADO: contrato NaN de `dominates` roto
`:96-108` SIN el guard `if !x.is_finite() || !y.is_finite() { return false; }`.
Réplica exacta de la semántica Rust: ciclo a≺b≺c≺a **True/True/True** con
a=[NaN,0,5], b=[1,NaN,6], c=[2,−1,NaN]; y NaN-domina-finito **True** (reverso
False). El doc-comment `:110-113` ("NaN ⇒ no-dominancia ⇒ cae al frente 0") es
FALSO en ambas mitades (cae por DOMINAR). Hoy latente en-vivo (los objetivos son
NaN-solo-globales, atrapados por `:705` — el claim §4 de HP-03 es correcto para
ese caso), PERO: (a) X-2 abre ±∞, (b) el oráculo de los tests P1/P1b USA la misma
`dominates` ciegas — una propiedad que comparte el bug que dice cazar no valida
el contrato NaN, y (c) el gate G2 de HP-02 exige el test de regresión del ciclo,
que no existe.

### X-4 [MEDIUM] — mesa: reporte de HP-03 stale sobre HP-02 + HP-08 sin reconciliar
(a) HP-03-APPLY.md §0.1 afirma "HP-02-DESIGN.md NUNCA aterrizó" — cierto a las
22:49, FALSO a las 23:05; el reporte se escribió 23:16 sin re-verificar el dir.
El §0.1 intentablindar con "Si HP-02 respawnea, debe AUDITAR contra este
archivo" — HP-02 SÍ auditó (F1-F8 + gates G1-G7), pero la dirección del cierre
quedó invertida: los hallazgos del diseñador sobre el apply siguen sin respuesta.
(b) HP-08-A §0 tabla "HP-02 aterrizó 23:05" y §4 da HP-03 PASS — sin nombrar F2/F3
ni reconciliar con HP-02 §0 ("uno BLOCKER: los property tests no compilan",
resuelto, y F2/F3 HIGH, no resueltos). Dos veredictos contradictorios coexisten
en el board sin nombrarse mutuamente.

### X-5 [LOW] — gaps menores del contrato HP-02, todos abiertos en el archivo final
- G4: property tests son fixtures únicos, no las propiedades randomizadas ≥100
  casos con seed fija que HP-02 §4 exige ("un fixture solo es un ejemplo").
- G7/F6: metadata SIN `fee_source` (`:710-729` — verificado por lectura); el
  fallback 0.003 heredado de op_15 sigue invisible por-run.
- G6: sin smoke de latencia del perfil default 40×24.
- F5 (mitad doc): `mod.rs:90` "(1-31)" stale — archivo que HP-03 SÍ editó (arregló
  el header :1,:6-7 pero dejó :90). `lib.rs:4` stale está fuera de claim (OK §7.3).

## 3. Lo que RESISTE la refutación (crédito donde corresponde)

- Matemática del núcleo canónica (Deb 2002): verificado por lectura + HP-02 §7.7
  + HP-08-A §1.1 — convergencia triple independiente. No encontré errores nuevos.
- 118/118 y la cadena cargo: HP-03 §6 + re-ejecución HP-08-A §1.9 (mismo hash
  9445aada26a795ef) + mi `cargo check` PASS. Aceptado como fact.
- Cirugía e higiene (diffs, marcadores, Cargo.toml 0, lib.rs 0, NO-GIT, §34.3
  cero matches): verificado por mí §1. Sin regresiones fuera de claim detectadas.
- Provenance del respawn (E0631/clippy/comentario falso): fix verificado en el
  archivo final `:974-975`; narrativa consistente con el snapshot de HP-08-A §1.10.
- La decisión de NO tocar `topology_map.rs` COLS=31 fue correcta como contención
  de scope (consumidores cruzados; confirmado no-break por HP-08-A R-4) — el
  milestone 264×32 queda como WO quirúrgico pendiente de ASIGNACIÓN.

## 4. Clasificación de gaps (para el BOARD)

| Gap | Fix | Clase |
|---|---|---|
| X-1 (F3) | Aplicar HP-02 opción A (`gas·1{x≠0}`) + test G3 dedicado (estado todo-no-rentable ⇒ null ⇒ Some(0.0), nunca Some(−gas)). HP-05 no depende del escalar (panel muestra pesos, no escalar — HP-08-B §8.2) ⇒ opción A sin colisión | **agent-fixable** |
| X-2 (F4) | Filtro pre-sort de filas no-finitas (`reason_non_finite_objectives`) + test reservas 1e200 (gate G2 segunda mitad) | **agent-fixable** |
| X-3 (F2) | Guard is_finite en `dominates` + test de regresión del ciclo (G2) | **agent-fixable** |
| X-4 | Amendment de HP-03-APPLY.md §0.1 (HP-02 SÍ aterrizó) + nota de reconciliación en HP-08 §4 (PASS condicionado a X-1..X-3) | **agent-fixable** |
| X-5 (G4/G7/G6/mod.rs:90) | Randomización ≥100 casos + metadata fee_source + smoke latencia + doc :90 | **agent-fixable** |
| COLS 31→32 + flip OPERATOR.json (engine_present + fix D-1 filename `op_32_nsga2.rs`→`op_32_multi_objective.rs`) | WO quirúrgico YA identificado por HP-03 §7.1-7.2 / HP-04 §6.3 / HP-08 §3.4 — requiere asignación de orquestador; el flip del espejo DEBE ir después de cerrar X-1..X-3 (si no, el canon certificaría un motor con los bordes abiertos) | **operator-gated** (asignación; commits/PR del gang = operador por NO-GIT) |
| Fila op_32 en workbook 12_OPERATOR_CONTROL + columna 13_STRAT_OP_MATRIX | Solo el operador puede editar el workbook (HP-04 §6.2) | **operator-gated** |

## 5. Reglas duras

- No muté código de producción, no git, VPS ni tocado, cero broadcast/executor.
- Escritos: SOLO este archivo (los scripts de réplica corrieron inline vía `py`,
  sin archivos temporales al repo).
- Cada impugnación cita file:line del archivo FINAL + salida literal de mi
  réplica ejecutada. Clasificación: X-1..X-5 = INFERRED-VERIFICADO (ejecutado),
  creditos §3 = CANONICAL_REPO (file:line + re-run).

— CROSS-EXAMINER de HP-03 (par adversarial, Gang Omniscience), 2026-09-08.
// HP-03-CROSSEXAM (2026-09-08)
