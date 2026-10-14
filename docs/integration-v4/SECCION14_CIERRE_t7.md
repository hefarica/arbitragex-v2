# §14 — CIERRE FORMAL t7 (α8 NOTARIUS + auditor final) · CONSEJO DE VIGILANCIA v3.1 §9

> Productor: reviewer/t7 (α8 NOTARIUS) · attempt e0a4eda5-193d-4781-8906-dcea369f899f ·
> 2026-10-13 ~16:0x–16:2x UTC · RULING del capitán aplicado (estado terminal
> **LISTO-PARA-V6**; decisiones de operador registradas en supplementalEvidence t6
> ts=1791907852739; D1/drill/dump = ABIERTOS-PENDIENTES-DE-t18).
> Consejo: {α7(t17), α8(este), α9(t6), α11(capitán)}. Conflicto de interés DECLARADO:
> α7 y α8 son la misma persona en esta sesión — mitigación: la auditoría F1-F12 es de
> FORMA mecánica (existencia+salida literal+veredicto) y todos los hashes/payloads
> fueron re-verificados independientemente en esta tarea.

## RONDA k — α1..α11

| rol | tarea | estado | evidencia (re-verificada por α8 esta sesión) |
|---|---|---|---|
| α1 CUSTOS | t15 V-MAP | OK | A0a-A0f presentes, MAP.sha256 5/5, A0d 100% escrituras + camino SSH, A0e NO_TOCAR.md no vacío (3657 B); riesgos R1-R12 cargados → decisiones operador registradas (suppEv t6) |
| α2 FABER | t9 | OK | release t9 ℛ heredado: hashes verificados vía R-20261013T085048Z (0 mismatches); errata cond7 demostrada |
| α3 MENSOR | t16 | OK | XREF_BASELINE.md/.json sha == A1 (7a03e242…/089ba911…) |
| α6 LIMES | t9/t6 | OK | 𝒞 EJECUTABLE persistido (tools/c_evaluator.py): canónica rc=0 reproducido por α8; literal rc=1 (cond7 errata, enmendada α11) |
| α7 FALSATOR | t17 | NO_REFUTADO 12/12 | 12 evidencias existen con salida literal + veredicto (F1: marcadores VERDICT_S/D/OMEGA; F4: VERDICT_F4 línea 15 — falso negativo del check automático corregido a mano); R_alpha7.tsv 12 filas; F12-agente NO CONFORME (capitán, A1_MANIFEST.md:5) |
| α8 NOTARIUS | t7 | este reporte | hallazgos H1-H5 abajo |
| α9 VIGIL | t6 | OK | V-MAP-Δ ×3 DERIVA=0 reproducido con tools/vmap_delta.py --check-artifacts rc=0 |
| α10 SCRIBA | t6/t14 | OK con H1/H2 | A1 tabla §11-bis 16/16 hashes MATCH; ℛ canónico 39/44 bien formadas+verificadas, 5 malformadas (H1) |
| α11 ARBITER | capitán | OK | enmienda cond7 (𝒞 canónica), errata G13, ruling LISTO-PARA-V6 + decisiones + orden 1-6 t18 |
| α4/α5 | — | N/A | roles no materializados en el tablero visible de esta ronda |

## Medidas nucleares (re-mediciones α8 de esta sesión, no citas)

| condición | resultado | comando/salida |
|---|---|---|
| Φ = ⋀g₀..g₇ | **1** | `python tools/phi_static.py MANIFEST.txt` → 7/7 PASS, PHI_STATIC=1, exit=0 (salida literal en t7-evidence); g7 inspección 10/10 (V5_shadow) |
| 𝒞 canónica | **1** | `python tools/c_evaluator.py SSOT --canonical` → C(theta)=1, exit=0 (reproducido) |
| Λ_vps (pre-swap) | **1** | GET https://arbx.ape-tv.net/api/status @2026-10-13T16:08:50Z → deploy.sha 5f8ea3d67… == origin/main (git rev-parse) == A0; 7/7 servicios OK; killswitch enabled=false (PAPER ON); divergencia /health=404 declarada (endpoint canónico=/api/status, t1) |
| DERIVA (V-MAP-Δ) | **0** | `tools/vmap_delta.py --check-artifacts p1 p2 p3` → total_drift=0, exit=0 (3/3 limpias, P9=3) |
| quórum §7 | **6/7** | lock L (flock) se adquiere al ejecutar V6 → t18; α6 ✓ α7 ✓ α9 ✓ α8 ✓(con H1 forma) L3-registrada ✓ ejecutables-persistidos ✓ |
| A1 §11-bis | **16/16 MATCH** | re-hash de A0,A0doc,A1semilla,A2,A3,A4(f27d869c…),A5,A6,A7,A8,A9,x_ref,A4f0,α7,xrefraw×2 — 0 mismatches |
| A7 | **59 filas; 9×'1', 50×'0'** | conteo propio: veredicto 1: 9, veredicto 0: 50, otro: 0; |A7|>0 ✓; sha(A7)∈A1 ✓ |
| F1-F12 forma | **12/12** | t7-evidence/f1_f12_form_audit.txt (+ corrección F4 manual) |
| 3 limpias / 0 regresiones / 0 rollbacks | **P9=3 (Δ); publicaciones: 0/3** | V-MAP-Δ 3/3 limpias; escaleras completas: 1 (hasta V5); NINGUNA publicación ⇒ 0 regresiones/0 rollbacks en producción (condición pre-swap trivial); el piso del operador (≥2 releases + 1 dump restaurable) es t18 |
| PAPER ON | **sí** | killswitch.enabled=false (medido 16:08Z) + t16 paper_rejected 26/26 en el feed |

## Hallazgos (contradichos con artefactos, no con prosa)

- **H1 [medio — VINCULANTE pre-publicación, reparable en minutos]**: ℛ.tsv canónico
  (R-20261013T1545Z-t6) filas 41-45 MALFORMADAS: fila 41 sin sha256 directo (referencia
  indirecta "ver p1=… p2=… p3=…" ⇒ C2 violado en forma); filas 42-45 con 3 columnas (sha en
  columna artefacto, "qa/t6 <ruta>" en productor, verificador degenerado "/t7" pre-asignado
  por t6 ANTES de que t7 existiera ⇒ C3 violado en forma). PAYLOAD correcto: α8 verificó
  hoy los 5 hashes contra sus archivos (p1=fc8e71e0/p2=f1d5baa2/p3=d6e0297a,
  V2_static=7c8a4e46, V3_build=57cb51a1, R_report_T6=f00c0921, ESCALERA=8c830f40 — 5/5 OK).
  Reparación pre-V6: reescribir las 5 filas en formato 4-col canónico con verificador=t7
  (esta auditoría las convalida) — el quórum α8 "hashes∈ℛ" exige el registro legible.
- **H2 [medio]**: δ_abs=0.005 NO está en A1_MANIFEST.md (exigencia §11-bis del cierre:
  "incluir … sha256(x_ref) y δ_abs en A1"). Hoy vive en x_ref.json (hasheado en A1) y en
  V5_shadow.md. Reparación: 1 línea en A1 + regenerar sha(A1) en ℛ.
- **H3 [bajo, contractual]**: L1,L2,L4,L5,L6 e I3,I4,I5 SIN definición persistida en
  A0..A9/docs (grep propio; solo rastros: L3 en contrato t17, I1/I2 citados por t11/t13).
  Bajo §8 (¬prosa como fuente) son CLAIM. Estado medido: L3 = NO_REFUTADO-no-medible
  (t17-F4) + tolerancia Δ≤δ_abs aplicada en V5; I1 = verificado (t11/t13/Q1-t6: 0 bytes hot
  path); I2 = verificado (t13 unittest 10/10). Recomendación: glosario persistido en A1/A7.
- **H4 [ABIERTO-PENDIENTE-DE-t18, ruling]**: D1 igualdad de inputs de build t10→R-t6:
  la aserción se CITÓ (detector_policy 5698976f byte-exacto + "conjunto byte-idéntico") pero
  hashes_maestro_t6.txt (52 hashes) cubre el release, NO los inputs del crate ⇒ prueba
  completa pendiente (`git diff --stat <t10-state>..R-t6 -- '*.rs' 'Cargo.toml' 'Cargo.lock'`,
  según el propio operador). Drill swap→rollback→swap y dump+restore-test: pendientes.
- **H5 [informativo]**: coherencia ℛ viejo(t9, 28 filas con PROMOTED t10) ↔ ℛ canónico
  (fila 29) ✓; ℛ mutó 27→28 durante t17 (documentado entonces) — sin pérdida.

## §10 + §8

- ¬prosa como fuente: verificado por muestreo en §E/V5/Q1-Q8/A1 (todo número cita artefacto);
  las excepciones persistidas son H3 (lemas/I sin definición) — reportadas, no suavizadas.
- ¬auto-aprobación: 39/44 filas con verificador≠productor ✓; EXCEPCIÓN de forma: filas 42-45
  (H1) — verificador pre-asignado; convalidación a posteriori por α8 registrada aquí.
- ¬despacho documento completo (A4): CUMPLIDO — F12-agente ejecutado por el capitán →
  NO CONFORME (A1_MANIFEST.md:5; quórum α7); paquete pre-enmienda NO publicado.

## Escalera auditada

Nada publicado hasta V5 verde + QUÓRUM: **cumplido de hecho** (V5 verde; publicación
espera t18 con quórum 6/7 + lock L). V6: runbook con flock + RULE 03 (--no-cache
--env-file) + W=180s ✓. V7 sobre ACTIVO (RG-01 card servida) en runbook ✓. §B sin
'esperemos': V8 armado "sin esperar" (§E:20, runbook paso 6) ✓. §I Q1-Q8: 8/8 OK
(Q8=NO APLICA con motivo exacto) ✓. Riesgos α1 antes de V5: V5 fue shadow 0 bytes
(riesgo material nulo); riesgos de mutación quedaron en el bloqueo de V6 y HOY tienen
decisiones registradas (topología (a) imagen única arbx:R-<ts> con digest en A1;
backup 3 condiciones; riesgo residual condicionado a drill) — coherente con el ruling.

## ESTADO TERMINAL: **LISTO-PARA-V6** (ruling capitán; §7 DONE requiere Λ post-swap)

## RONDA k (cierre)

| campo | valor |
|---|---|
| α1..α11 | α1 OK · α2 OK · α3 OK · α6 OK · α7 NO_REFUTADO 12/12 · α8 H1-H5 (este) · α9 OK · α10 OK-con-H1/H2 · α11 OK (ruling) · α4/α5 N/A |
| 𝒞 | canónica=1 (reproducido rc=0); literal=0 por errata cond7 (enmendada α11) |
| Φ | 1 (g1-g6 reproducidos exit=0; g7 inspección 10/10) |
| deriva | 0 (V-MAP-Δ 3/3, reproducido) |
| ℛ | 44 filas: verificadas=39, mismatches=0, malformadas=5 (payload 5/5 correcto) → refutados=0, pendientes-de-forma=5 (H1) |
| DESPACHO/INSULARIDAD | F12-agente NO CONFORME ✓; α8 re-derivó Φ/𝒞/Δ/Λ/hashes/A7 por vía propia |
| QUÓRUM §7 | 6/7 (lock L ⇒ V6/t18); publicación exige H1+H2 reparados antes del swap |
| ABORT | no aplica (0 publicaciones; adversarial V6 EMPATE 3-3 documentado, se rompe con el drill) |
| SIGUIENTE | t18 (qa) orden 1-6: transporte → D1 (git diff inputs) → dump+restore-test → drill swap→rollback→swap → swap único+V7 (Λ post-swap, Φ activo, RG-01) → V8 si §B; PRE-V6: reparar H1 (ℛ filas 41-45) y H2 (δ_abs∈A1) |

## VEREDICTO t7: **needs_revision** (del OBJETO auditado, no del trabajo de auditoría)

Findings: H1 (ℛ filas 41-45 malformadas — C2/C3 forma; payload correcto) y H2 (δ_abs
ausente en A1) son reparaciones concretas PRE-V6; H3 (lemas/I sin definición persistida)
y H4 (D1/drill/dump) quedan como ABIERTOS-t18 por ruling. Nada de esto invalida las
mediciones nucleares: Φ=1, 𝒞=1 canónica, Λ_vps drift=0 (pre-swap), DERIVA=0 ×3,
A1 16/16, A7 59/9/50, F1-F12 12/12 — todo RE-MEDIDO por α8 en esta sesión.
