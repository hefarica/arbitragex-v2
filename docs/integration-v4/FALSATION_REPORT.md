# FALSATION_REPORT — α7 FALSATOR (t17, attempt 8457a681-776e-494b-8b44-b03a9f150f2b)

> CONTRATO α7 FALSATOR (adenda v3.1). D=claim c · C∈{REFUTADO, NO_REFUTADO} ·
> Q=REFUTADO ⟺ ∃ contraejemplo reproducible (comando+salida literal) ·
> X=sin contraejemplo y sin prueba ⇒ NO_REFUTADO (no 'verdadero') ·
> I=α7 NO produce claims; solo los rompe; NO verifica lo propio.
> Objetivo bajo ataque: release R-20261013T085048Z (t9) + workbook supremo + contrato.
> INSULARIDAD: re-derivación por vías propias; instrumentos del falsator en
> `docs/integration-v4/t17-evidence/` (c_evaluator.py, apply_release.py); evidencia
> cruda por ataque en `docs/integration-v4/t17-evidence/F*.txt`; registro ℛ de α7 en
> `docs/integration-v4/t17-evidence/R_alpha7.tsv` (nombre, productor, sha256, verificador=α7, veredicto).
> Producción NO tocada: ataques destructivos SOLO en clones/staging/worktree temporales.

## Resumen ejecutivo

| # | Claim atacado | Veredicto |
|---|---|---|
| F1 | \|𝒮\|=264, \|𝒟\|=60, \|Ω\|=1436 | **NO_REFUTADO** |
| F2 | ∀s IMP(s)≥BASE(s) | **NO_REFUTADO** |
| F3 | DERIV_POS>0 (=1982) | **NO_REFUTADO** |
| F4 | L3 quote_local=quote_onchain ±0.005 | **NO_REFUTADO** (no medible hoy — sin prueba y sin contraejemplo) |
| F5 | 𝒞(θ) DETIENE ante máscara mutada | **NO_REFUTADO** (con nota MUT-C y hallazgo de persistencia) |
| F6 | V-MAP-Δ detecta deriva G2/G4/G6/G13 | **NO_REFUTADO** (G13 no existe: referencia no-grounded) |
| F7 | ABORT antes de escribir fuera de BLAST_RADIUS | **NO_REFUTADO** |
| F8 | Release aplicado 2× = árbol idéntico | **NO_REFUTADO** |
| F9 | Rollback deja el servicio sano | **NO_REFUTADO** |
| F10 | Artefacto sin sha256 no existe (C2) | **NO_REFUTADO** |
| F11 | μ∈CLAIM excluido por defecto | **NO_REFUTADO** |
| F12 | Ejecutor declara NO CONFORME (A4) | **NO_REFUTADO vía determinista**; despacho a agente PENDIENTE (limit 0) |

**Ningún claim F1-F12 resultó REFUTADO.** NO_REFUTADO ≠ verdadero: F4 carece de
instrumento (claim no medible), F5/F6 dependen de reconstrucciones del falsator porque
los controles de t9 no quedaron persistidos como ejecutables, y F12 tiene su despacho
real pendiente. Detalle y salidas literales por ataque abajo.

---

## F1 — falsificar conteos |𝒮|=264, |𝒟|=60, |Ω|=1436

**Comando:** script α7 (evidence F1_recount.txt) contando por 5 vías INDEPENDIENTES a la
del productor (que usó gen_ssot_v4_rs.py): SSOT csv del release, SSOT json del release,
xlsx CLASIFICACION_264, xlsx TESTS_ATOMICOS (unique MEV), csv del paquete 06_CLASIFICACION.

**Salida (literal):**
```
VIA1 SSOT csv      |S|=264  |D|=60  sum(|mask|)=1436
VIA2 SSOT json     |S|=264  sum(|mask|)=1436
VIA3 xlsx CLASIF_264 |S|=264  sum(|hops|)=1436
VIA3b TESTS_ATOMICOS unique MEV=264 data_rows=264000
VIA4 pkg clasificacion_264.csv data_rows=264
VERDICT_S: NO_REFUTADO (vias 264,264,264,264,264)
VERDICT_D: NO_REFUTADO (via 60)
VERDICT_OMEGA(sum|hops|=1436): NO_REFUTADO (vias 1436,1436,1436)
```
Ω identificado y re-derivado = suma de hops permitidos sobre las 264 máscaras.
**Veredicto F1: NO_REFUTADO.**

## F2 — falsificar monotonía ∀s: IMP(s)≥BASE(s)

**Comando:** α7 leyó CLASIFICACION_264 (264 filas BASE_POS/IMP_POS), TESTS_ATOMICOS
(264.000 filas), AGREGADOS_DETECTOR (60), AGREGADOS_SUPERFICIE (10) del xlsx supremo.

**Salida (literal):**
```
CLASIFICACION_264 rows=264 violations IMP<BASE=0 []
TESTS_ATOMICOS rows=264000 violations IMP<BASE=0 null_base_or_imp=0
TESTS_ATOMICOS ASSERT histogram={'BASE<=IMP': 264000}
AGREGADOS_DETECTOR rows=60 violations=0
AGREGADOS_SUPERFICIE (cols corregidas idx2/idx3) rows=10 violations=0
```
Nota de transparencia: mi primer pase reportó "10 violations" en AGREGADOS_SUPERFICIE
por bug PROPIO de columnas (comparé IMP_POS vs DELTA); corregido y documentado en el
evidence (F2_monotonicity.txt §CORRECTION). No hubo contraejemplo real en 264+264.000+70 filas.
**Veredicto F2: NO_REFUTADO.**

## F3 — falsificar DERIV_POS>0 (re-derivar desde A2)

**Comando:** α7 re-derivó desde la tabla per-estrategia (fuente de datos A2 del paquete):
suma de BASE_POS e IMP_POS de las 30 estrategias familia D_ (superficie CEX_SPOT).

**Salida (literal):**
```
re-derived FAMILIA=D_: n=30 sum_BASE_POS=1982 sum_IMP_POS=3560
re-derived SUPERFICIE=CEX_SPOT: n=30 sum_BASE_POS=1982 sum_IMP_POS=3560
INVARIANTES declares: DERIV_POS>0 OBSERVADO=1982
VERDICT_F3: NO_REFUTADO (BASE=1982, IMP=3560; 1982 == suma re-derivada exacta)
```
Aclaración: la hoja A02 del xlsx es un ALGORITMO (DIRTY-SUBGRAPH), no datos; "A2" del
contrato = tabla de clasificación per-estrategia (vía usada). El "1982 observado" de
INVARIANTES corresponde a la suma BASE (pre-fix); la post-fix (IMP) es 3560 — DERIV_POS>0
se sostiene en ambas lecturas.
**Veredicto F3: NO_REFUTADO.**

## F4 — falsificar L3 quote_local=quote_onchain ±0.005

**Comando:** α7 buscó pares (quote_local, quote_onchain) en todos los artefactos
disponibles: feed vivo (qa-baseline/edge_opps_live.json), XREF_BASELINE.json, x_ref.json.

**Salida (literal, F4_quote_l3.txt):**
```
Artifacts available: feed serves ONE quote way only (economics.legs + quote_block).
On-chain instrument status (XREF_BASELINE.json:19-20): ssh arbx -> exit=255 (NO DISPONIBLE);
  sql_query -> NO DEVUELVE FILAS.
Producer of L3 (t4 ledger, epsilon=LEDGER_TOLERANCE_USD=0.005): PENDIENTE (PLAN_T1_T8.md t4).
VERDICT_F4: NO_REFUTADO (sin contraejemplo Y sin prueba: claim no medible hoy)
```
**HALLAZGO SECUNDARIO (para t6/t16):** consistencia interna del wire — recomputando
`amount_out_usd := legs[-1].amount_out_wei / 10^dec × token_prices_usd[sym]` sobre los
26 items: 3 discrepancias >0.005 USD (máx 9.15 USD) en rutas triangulares que cierran
en token no-base (HEX×2, EURC×1; ej. id=99595269: servido 2.07e-8 USD vs recomputado
9.15 USD con amount_in_usd=0.01). Fail-honest: puede ser semántica de legs[] en rutas
abiertas; discrepancia medible y reproducible.

**Veredicto F4: NO_REFUTADO — y para el quórum §7: L3 NO computable como verificado
hasta que t4 produzca el ledger con dos vías.**

## F5 — forzar ¬𝒞(θ) mutando una máscara en clon local → el evaluador DETIENE

**Comando:** clon de SSOT/ en t17-evidence/clone_ssot_F5 + evaluador reconstruido del
CONTRATO (c_evaluator.py, 7 condiciones textuales; --canonical excluye la condición 7).
Validación del instrumento: θ prístino reproduce EXACTAMENTE el veredicto de t9.

**Salida (literal, F5_c_theta_mutation.txt):**
```
R0 pristine theta LITERAL rc=1 :: C(theta)=0 [literal(7 cond)] - EVALUATOR STOPS:
  FAIL cond7 mask(MEV-05-003)=[2, 3, 4, 5, 6] != {2}
R0 pristine theta CANONICAL rc=0 :: C(theta)=1 [canonical(6 cond)] - all conditions hold
MUT-A MEV-01-001 mask->'3/4/5/6' (size 4, legal) CANONICAL rc=1 :: EVALUATOR STOPS:
  FAIL cond3 |Omega|=1435 != 1436 | FAIL cond5 d(h)={... 7:202} != {... 7:203}
MUT-B MEV-02-001 mask->'3/7' (size 2 < 3) CANONICAL rc=1 :: EVALUATOR STOPS:
  FAIL cond4 mask size out of [3,7]: ['MEV-02-001'] (+cond3/cond5)
MUT-C swap masks MEV-01-001<->MEV-01-020 (sizes, |Omega|, d(h) intact) CANONICAL rc=0
```
Notas honestas: (a) MUT-C (swap entre estrategias, todos los agregados intactos) NO la
detienen las condiciones del contrato — esa clase solo la atrapa la capa sha256-vs-θ
(Q4/V2); (b) **HALLAZGO DE PERSISTENCIA**: el evaluador 𝒞 de t9 fue script de sesión y
NO quedó en el release (gen/ solo contiene el generador). El evaluador usado aquí es la
reconstrucción α7 del texto del contrato. Para el quórum: t6 necesita un 𝒞 ejecutable persistente.
**Veredicto F5: NO_REFUTADO.**

## F6 — forzar deriva en G2/G4/G6/G13 → V-MAP-Δ la detecta

**Comando:** clon SSOT con deriva inyectada (1 estrategia por grupo, tamaño de máscara
legal — la clase más difícil) + chequeo V-MAP-Δ reconstruido (diff per-estrategia contra
θ congelado + capa sha256).

**Salida (literal, F6_vmap_drift.txt):**
```
V-MAP-DELTA (per-strategy mask diff frozen-theta vs clone): 3 drifted strategies DETECTED:
  DRIFT MEV-02-001 [G02]: theta=2/3/4/6/7 -> clone=2/3/4/6/5
  DRIFT MEV-04-001 [G04]: theta=2/3/4/5/7 -> clone=2/3/4/5/6
  DRIFT MEV-06-001 [G06]: theta=2/3/4/5/6 -> clone=3/4/5/6/7
sha256 theta  = 7fb529c6…
sha256 clone  = cf94dc0a…   sha256 differ: True
G13 probe: GRUPO universe = G01..G11 (11 groups, measured); G13 does NOT exist in the hop map
  -> contract reference G13 is NOT GROUNDED (same pattern as cond7 errata)
```
**Hallazgo de contrato:** "G13" no existe (universo medido G01–G11). La deriva en los
grupos existentes G2/G4/G6 fue detectada 3/3 por el diff per-estrategia y por la capa hash.
**Veredicto F6: NO_REFUTADO.**

## F7 — escribir fuera de BLAST_RADIUS (A0d) → ABORT antes de escribir

**Comando:** aplicador del release con guard transaccional A0d (apply_release.py,
reconstrucción del mecanismo ASSEMBLY.md) invocado con `--extra backend/api-server/src/index.ts`
(fuera del blast radius) sobre worktree limpio 21feefb32.

**Salida (literal, F7_blast_radius.txt):**
```
exit = 2
ABORT [A0d blast radius]: destinations outside BLAST_RADIUS: ['backend/api-server/src/index.ts']
NOTHING WRITTEN (transactional abort before first write)
index.ts        pre=120c2211… post=120c2211… INTACT=True
detector_policy pre=fed8cfcc… post=fed8cfcc… INTACT=True (abort happened BEFORE any legal write too)
releases/ created in target: False    t9-evidence/ created in target: False
```
**Veredicto F7: NO_REFUTADO** (ABORT con cero bytes escritos, verificado por hashes pre/post).

## F8 — aplicar el release DOS veces → idempotencia

**Comando:** apply #1 y apply #2 del release sobre el mismo worktree limpio.

**Salida (literal, F8_idempotency.txt):**
```
APPLY1 exit=0; porcelain = M detector_policy.rs / ?? releases/ / ?? t9-evidence/ (3 líneas)
diff --stat: 1 file changed, 134 insertions(+)          # == ASSEMBLY.md +134/−0
APPLY2 exit=0; porcelain byte-identical to APPLY1
detector_policy.rs sha 5698976f… estable en ambos == gen/ == ℛ.tsv:10
DFAC const count=1 (sin duplicación)
VERDICT_F8: NO_REFUTADO
```
**Veredicto F8: NO_REFUTADO.**

## F9 — simular rollback → servicio sano

**Comando:** `git checkout -- detector_policy.rs` + borrar releases/ y t9-evidence/ del
worktree staging; luego `cargo test --lib detector_policy` (CARGO_TARGET_DIR compartido).

**Salida (literal, F9_rollback_health.txt):**
```
porcelain after rollback: [] lines=0
detector_policy post-rollback hash: FED8CFCC… == pre-release
test result: ok. 12 passed; 0 failed; 0 ignored; 1425 filtered out; finished in 0.01s
EXIT=0
```
**Veredicto F9: NO_REFUTADO** (rollback byte-exacto + módulo verde 12/12; producción no tocada).

## F10 — inyectar artefacto SIN sha256 → no pasa (C2)

**Comando:** validador del ledger ℛ (guard C2 reconstruido: 4 columnas + sha256 64-hex +
verificación de hash contra bytes reales) sobre ℛ.tsv original (control) y sobre copia
atacada con (a) fila sin sha256 y (b) fila con sha256 falso bien formado.

**Salida (literal, F10_no_sha256.txt):**
```
CONTROL (ℛ.tsv, 28 data rows): admitted=28 excluded=0
ATTACK: admitted=28 excluded=2
  [excluded] fake_artifact_no_hash.txt: NO/invalid sha256 -> C2: does not exist for the system
  [excluded] SSOT/strategy_hop_map.csv: HASH MISMATCH declared=eeee… actual=b25d4c89…
admitted set unchanged under attack: True
```
Nota de concurrencia: ℛ.tsv creció de 27 a 28 filas DURANTE t17 (fila 28 "PROMOTED…"
añadida por otra tarea); el veredicto es contra el estado actual de 28 filas (las 28 verifican).
**Veredicto F10: NO_REFUTADO.**

## F11 — inyectar μ ∈ CLAIM → admisión EXCLUYE por defecto

**Comando:** gate determinista de evidencia (metacog_audit) con 3 casos.

**Salida (literal, F11_claim_default_deny.txt):**
```
CASO 1 mu SIN evidencia ("|S|=300"):            → NO_SOPORTADO ("numero sin fuente")
CASO 2 mu con evidencia que NO lo contiene:     → NO_SOPORTADO + evidencia NO ATRIBUIDA
CASO 3 control con artefacto real (assert 264): → SOPORTADO (admisión con binding real)
```
**Veredicto F11: NO_REFUTADO** (default-deny demostrado; no es rechazo ciego).

## F12 — despachar documento a un ejecutor → NO CONFORME (A4)

**Comando:** 2 intentos de despacho real (subagent spawn + workflow) y verificación
determinista del claim de conformidad.

**Salida (literal, F12_a4_executor.txt):**
```
1. subagent → Error: "AgentTeams member delegation limit (0) reached; report to the
   captain instead of spawning another agent"
2. workflow → Error: "agent() could not start a child … delegation limit (0) reached"
Determinista: afirmación "CONFORME para publicar ahora (C_literal=1, promoción autorizada,
V4 completo, base verde)" → veredicto CONTRADICHO (E1 C_THETA_verdict.json: C_literal_reading=0)
Hechos del propio documento: R_report.md:16 "Promoción al árbol REAL retenida";
R_report.md:43-54 dos escalados sin resolver; V4_dryrun.md "DEGRADADO".
```
**Veredicto F12: NO_REFUTADO vía determinista.** El despacho a ejecutor-AGENTE queda
PENDIENTE del capitán: prompt íntegro listo en `DESPACHO_A4_EXECUTOR.md` (el capitán
tiene capacidad de spawn; α7 no).

---

## Quórum §7 (insumo para t6)

Sobre los claims atacados F1–F12: **α7 = NO_REFUTADO en los 12** — el quórum puede
avanzar. Con las ADVERTENCIAS que el quórum debe cargar (NO son refutaciones, son
condiciones de NO-VIDA de los claims):

1. **L3 (F4) NO verificado**: sin instrumento on-chain, productor t4 pendiente.
   t6 no puede marcarlo verde; solo "no refutado, no medible".
2. **Controles no persistidos (F5/F6)**: evaluador 𝒞 y V-MAP-Δ de t9 fueron scripts de
   sesión. Reconstrucciones α7 funcionan y detienen/detectan, pero el quórum §7 exige
   los ejecutables EN el release para futuras re-verificaciones.
3. **F12 despacho real pendiente** (delegation limit 0): capitán debe lanzar
   DESPACHO_A4_EXECUTOR.md y anexar la respuesta.
4. **Referencias no-grounded del contrato** (mismo patrón que la errata cond7): "G13"
   no existe en el hop map (universo G01–G11 medido). Recomendación: enmendar.
5. **Línea base**: los 3 tests pre-existentes (liquidation_snipe_engine) siguen rojos
   en HEAD (escalado [B] de t9); F9 no los toca (detector_policy 12/12 verde).
6. **Hallazgo secundario wire USD** (F4): 3/26 items con |recomputed−served| hasta 9.15
   USD en rutas que cierran en token no-base — para t6/t16, semántica aclarar.

## Registro ℛ α7

`docs/integration-v4/t17-evidence/R_alpha7.tsv` — 12 filas (ataque, claim, productor,
sha256 del artefacto atacado, verificador=alpha7/t17, veredicto).

## Artefactos de esta tarea

- `docs/integration-v4/FALSATION_REPORT.md` (este archivo)
- `docs/integration-v4/t17-evidence/F1_recount.txt` … `F12_a4_executor.txt` (evidencia cruda por ataque)
- `docs/integration-v4/t17-evidence/c_evaluator.py` (reconstrucción 𝒞 del contrato)
- `docs/integration-v4/t17-evidence/apply_release.py` (aplicador con guard A0d)
- `docs/integration-v4/t17-evidence/R_alpha7.tsv` (registro ℛ de α7)
- `docs/integration-v4/t17-evidence/DESPACHO_A4_EXECUTOR.md` (paquete F12 pendiente del capitán)
- Worktree staging `$env:TEMP\arbx-t17-stage` y clones locales: eliminados al cierre (R11, misma sesión).

## Apéndice — auditoría de evidencia (metacog_audit, determinista)

Afirmación de cierre auditada contra los 12 evidence files: primera pasada MIXTA
(6 soportados / 5 no-soportados POR FORMA: subcadenas-objetivo no verbatim en el
fragmento) + re-auditoría con objetivos verbatim: 5/7 soportados; los 2 restantes
son la oración F9 partida por el fragmentador en ";" (su tramo final "0 ignored
EXIT=0 (F9_rollback_health.txt)" SÍ pasó con cita inline y el archivo contiene la
línea literal completa `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured;
1425 filtered out; finished in 0.01s` + `EXIT=0`). Cero CONTRADICHOS. Correcciones
propias documentadas en-evidence: F2 (bug de columnas, corregido), F4 (línea falsa
"…< 0.005", corregida), F10 (resolutor de rutas con paréntesis, corregido).
