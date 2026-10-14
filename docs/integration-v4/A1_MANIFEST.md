# A1 — MANIFEST REGENERADO (t6, regla de hash §11-bis)

> Generado por t6 (α10 SCRIBA) el 2026-10-13T16:0xZ sobre el release NUEVO
> `R-20261013T1545Z-t6` (estado RESUELTO: enmienda cond7 + promoción t10 en árbol + ADM-n admitidos).
> **POST-MERGE UPDATE (t31, 2026-10-14T05:4xZ)**: sha_head = **d3157c66e5299a53320d1ab4c79633835e4591ed**
> (MERGE COMMIT de PR #952 a main). Tag: v4.0.0-admitted (anotado, SIN firma, no verificado criptográficamente
> — DECLARADO). ℛ cita COMMITS no tags (regla). Split de releases: R-prod-20261014T0405Z (digest dffaf0e8…, VIVO
> con mitigación) ≠ R-20261014T0520Z (sello be0aefa3…, repo post-EOL). PRODUCCIÓN ≠ MAIN en ambas direcciones.
> A1-semilla: `.tmp-inbox/integrado-supremo/MANIFEST.txt` sha256 `45b3bb2e0e2281a986fd2edd1b037acc01e0ccb00494a4ed32ba29e0c35ed575`
> (== integration-manifest.md t2, fuente ÚNICA de g1-g6 §7 anti-circularidad — verificado por
> `tools/phi_static.py`: 7/7 PASS, exit=0).

## g1..g6 (del A1-semilla, verbatim)

```
counts: strategies=264 detectors=60 combos=1436 tests=264000
asserts 264/60/1436: OK
deriv_pos: 3560 (>0 OK)
base_pos: 19744
imp_pos: 31744
delta_pct: +60.8%
monotonic: true
hop_dist: 245/262/260/233/233/203
```

## Enmiendas y alertas que A1 carga (sin suavizar)

0. **A1 SUPERSEDE §10 (delta) — declaración explícita del operador (t19, higiene de reportes)**:
   el valor de delta que gobierna es **+0.60778 MEDIDO** (=(31744−19744)/19744, t8/t6; se escribe
   `+60.778%` o `+60,78%` — decimal PUNTO o coma decimal, JAMÁS separador de miles). El `'+0.604'`
   del texto maestro §10 queda SUPERSEDIDO por este A1; la prohibición contra `'+0.636'` SIGUE
   VIGENTE y ahora se aplica también al '+0.604' superseído (mismo tratamiento: el número que
   manda es el MEDIDO, no el del texto). Sin esta declaración, quedaría una prohibición vigente
   contra un número que el propio A1 emite. El `delta_pct: +60.8%` citado arriba es VERBATIM de la
   semilla A1 (redondeo a 1 decimal del mismo 60.778% medido).
1. **cond7 (α11 capitán)**: `mask(MEV-05-003)={2}` literal es errata de transcripción; θ tiene
   `{2,3,4,5,6}` ⊇ {2} ⇒ 𝒞 canónica=1 (`tools/c_evaluator.py --canonical` rc=0; literal rc=1 documentado).
2. **DFAC['D_']=0.80** materializado en `gen/detector_policy.rs` (t9, promocionado por t10 al árbol:
   sha 5698976f… — el MISMO archivo vive en el árbol compartido y en el release).
3. **DERIV_POS=3560** con definición F04/t13 (IMP familia D_ post-fix; 1982=BASE pre-fix; 12000=filas
   delta>0 en TESTS — tres métricas distintas).
4. **Alertas t8**: `monotonic: true` aparece SOLO en A1/A2 (ninguna otra fuente del paquete);
   "detectores==60" con definición AGREGADA (t8). Registradas, no resueltas por A1.
5. **base/imp MEDIDO**: 19744/31744 ⇒ **+60.778%** = (31744−19744)/19744 (t8; A1-semilla redondea +60.8%).
6. **Errata G13**: NO existe en el universo medido (G01-G11, t15/t17); el equivalente de releases es
   git state + `docker compose ls` (enmienda α11-D aplicada en vmap_delta.py).

## Tabla de hash §11-bis — sha256(A0..A9) ∈ A1

| Artefacto | Archivo | sha256 |
|---|---|---|
| A0 (V-MAP congelado) | `vps-map/VPS_MAP.json` | `70e6414262df63476495b2716f5beff1ad78b39c0f66540ccf07fa5860a23346` |
| A0-doc | `vps-map/VPS_MAP.md` | `6691b5875d239c6413209109e4549dbaea5ed66ede3cfa0413b245d5889f2d5e` |
| A1-semilla | `.tmp-inbox/integrado-supremo/MANIFEST.txt` | `45b3bb2e0e2281a986fd2edd1b037acc01e0ccb00494a4ed32ba29e0c35ed575` |
| A2 (264 estrategias) | `.tmp-inbox/integrado-supremo/06_CLASIFICACION/clasificacion_264.csv` | `38f038a82b5d16979e17d002c406d249d9979d164166e2cdaac063d0b80a7786` |
| A3 (criterios T6, MONTADO) | `docs/integration-v4/CRITERIOS_T6.md` | `919faf3eaffc845104e403819301ce97d00d2567248c763fbebe76df1a228d7c` |
| A4 (plan) | `docs/integration-v4/PLAN_T1_T8.md` | `f27d869ce8bacd1b61af968affa94191859103ece338422706fc4cde7b44f2cd` |
| A5 (fórmulas) | `.tmp-inbox/integrado-supremo/03_FORMULAS/FORMULAS_TARJETAS.md` | `fc50375bc58d459751e0264a2bdaf9b9d4d767cb76a34a75fb05fa04a775c2cc` |
| A6 (field registry) | `.tmp-inbox/integrado-supremo/03_FORMULAS/FIELD_REGISTRY.md` | `61e8633d205d110419b17fe0e181fbc2dc006b0a237fdfbef97952483ee12f29` |
| A7 (59 inclusiones) | `docs/integration-v4/REGISTRO_INCLUSIONES.md` | `803ef9ed582c1d8cc37595a405661e6200fdf2cfcb040bcb6a64d511849c9254` |
| A8 (60 hojas) | `docs/integration-v4/A8_final.xlsx` | `7573cdb70c74ed9bb651cc7902619d40d2c9910cfdc26984ffe064dc04895a34` |
| A9 (= release R-nuevo, sello V1) | `…/R-20261013T1545Z-t6/V1_hashes.txt` | `ab9b54d427f140dbe82ca9c830d347debdaa7621fcd269fbebbd4ee347ae9f43` |
| x_ref (línea base ℳ) | `…/R-20261013T1545Z-t6/x_ref.json` | `a6d521ea13fc475ff4f3a6f4b3acf98e5018ded5d816430b5a0f882bb4c26539` |
| A10-imagen (release unit, t20 build) | `docker image inspect {{.Id}}` en VPS → `sha256:a9b3ef50a8b326cddcacb1d2b630e3968c57a936e7d7f5fbb254a537e6a773f1` (185MB; build log `t20-evidence/build_finish.txt`) | `sha256:a9b3ef50a8b326cddcacb1d2b630e3968c57a936e7d7f5fbb254a537e6a773f1` |
| A4-manifest F0 (t2) | `docs/integration-v4/integration-manifest.md` | `c622a0068d3c3066aaebe41ba940d35b426e12d48fa1b9ad44158ef4a9bda034` |
| α7 (F1-F12) | `docs/integration-v4/FALSATION_REPORT.md` | `779bea98a141afe527fa62c6fb8a1c2ac531b59638035f9c7ef004ed14350cea` |
| x_ref crudo t16 | `XREF_BASELINE.md` / `.json` | `7a03e242…31d42` / `089ba911…bb6a` |

## δ_abs (exigencia §11-bis, H2-t7)

**δ_abs = 0.005** (= `LEDGER_TOLERANCE_USD`, A5 `FORMULAS_TARJETAS.md:4`; origen x_ref.json t9/t16).
Tolerancia aplicada en V5 (x(θ)) y V7 (histograma vs V0 con condición F4 registrada).

## Glosario ¬prosa (H3-t7) — definiciones operativas con artefacto (CLAIM → verificable)

- **L1 (x_ref)**: línea base ℳ del sistema vivo = `XREF_BASELINE.json` sha `089ba911…bb6a` (x1-x6 del wire público).
- **L2 (𝒞)**: evaluador del contrato = `releases/R-20261013T1545Z-t6/tools/c_evaluator.py` sha `66e591d0…9ad2` sobre `SSOT/` congelado.
- **L4 (V-MAP-Δ)**: `tools/vmap_delta.py` sha `97615fce…a5be` contra A0 (`vps-map/MAP.sha256`); enmienda α11-D (G13→git/compose).
- **L5 (Λ_vps)**: `GET https://arbx.ape-tv.net/api/status` → 7 servicios + deploy.sha + killswitch.enabled (PAPER ON ⇔ false).
- **L6 (Φ)**: ⋀g1-g7 = `tools/phi_static.py` (g1-g6, exit=0) ∧ `CRITERIOS_T6.md` g7 (INSPECCIÓN, adm/ t13).
- **I1**: 0 bytes de hot path sin ADM-n admitido (A7 filas veredicto 1; I1 del contrato de tracks).
- **I2**: un término por archivo por incremento (disciplina t13 adm/).
- **I3 (δ_abs)**: 0.005 USD (arriba, H2).
- **I4 (P9)**: contador de pasadas limpias de validación (t6: Δ×3); el drill de rollback del operador es requisito ADICIONAL, no una de las 3 limpias de publicación.
- **I5 (P10)**: piso de retención ≥2 releases + 1 dump restaurable, con guard de disco (orden operador V6).

> sha256(ESTE A1) queda en ℛ.tsv (re-generado H2-t7 2026-10-13; el A1 no se auto-referencia — mismo criterio que t14 para A7/A8).`n`n## WATCH-V7-001 (corrección V1 del operador, t28)`n`n- señal: cluster 500 en endpoints control-plane | trigger: cualquier 500 en {paper-mode/state, readiness*, strategies/runtime-status, scanner/heartbeat, opportunities/live} | umbral: **>0 durante V7 ⇒ NO transitorio, reproducible ⇒ investigar antes de V8** | evidencia base: T28_triaje.md (35/35 200, 0 RestartCount, sin recurrencia 4h — ``no activo ahora`` ≠ ``no vuelve``). Viaja al protocolo V7 de t27.

| A11-imagen (t27 mitigation release, base main 5f8ea3d67) | `docker image inspect {{.Id}}` en VPS → `sha256:f6686ac0d8ee08bb49eb9b80e5aa68198c0163c8a6babb53ad14139453802436` (185MB; T1+T2+T3+T6; tests 1738P/0F main + 287P/0F shared; boot real probado con config) | `sha256:f6686ac0d8ee08bb49eb9b80e5aa68198c0163c8a6babb53ad14139453802436` |

## KNW-001 — cola de eval de cartuchos (t26, orden firmado del operador; campos EXACTOS)

- **estado producción ACTUAL**: binario previo `dfd71012e1dc` (VIEJO post-rollback, 7/7, PAPER ON).
- **métrica**: eval enter→summary (`cartridge.active_eval_enter` → `cartridge.active_eval_summary`).
- **magnitud**: bulk p50=2891ms / p90=3207ms; max=290.9s; n=549 pares (**DECLARADO CHICO**);
  outliers >10s: 19 tx (3.46%) = 68.3% del tiempo total de eval (bloqueo+re-entrada ≤124 enters).
- **causa dominante**: DATA_GAP 65-66% por INCIDENCIA; **% TIEMPO NO-IDENTIFICABLE** (OLS colineal;
  banda [0%, 31.7%]; estimación uniforme ~21% — supuesto declarado).
- **cierre**: mitigación A01 prefilter marginal (fail-open) + fail-fast-classify SOLO si instrumento
  por-cartucho certifica Σtiempo(DATA_GAP)≥30% + control de outliers (cap re-entrada + timeout/eval)
  en release R-<ts2> con protocolo completo (drill real + swap + V7).
- **REAPERTURA**: si Σtiempo(DATA_GAP) < 30% ⇒ REDISEÑAR antes de implementar — **HOY NO está
  certificado ⇒ KNW-001 NACE EN REAPERTURA/REDISEÑO** (baseline de delta: 2.94× t24 n-grande).

## DEC-001 — Dos caminos de deploy (registro obligatorio t31)

- **causa raíz**: auto-deploy-vps.yml sin `environment:` ⇒ deploy sin aprobación humana (hallazgo revisor F-1)
- **Capa A (gating)**: `environment: production` + `required reviewers` (~3 líneas YAML) = aprobación humana sin deshabilitar
- **Capa B (mecanismo)**: workflow construye y publica imagen (build-en-CI); swap MANUAL por §G (drill+W+rollback). **BONUS**: build en CI resuelve disco VPS (10.4GB vs build 15-25GB — VPS sólo docker pull)
- **evidencia A6**: el swap real salió por camino directo (ssh+flock) SIN main — confirmado R-prod-20261014T0405Z
- **estado**: auto-deploy DESHABILITADO desde 2026-10-14T05:30Z (disabled_manually verificado)
- **decisión vence**: 2026-10-15 (propuesta capitán — ajustable por operador)
- **implementación**: propuesta 2026-10-17; re-enable = parte de la implementación, NO antes
- **riesgo abierto**: ensancha escisión main↔producción = deuda

## D2-D5 — Declaraciones definitivas (t31)

- **D2**: hashes canónicos SÓLO desde C7b (4ed4626a9) — checkout de commits previos devuelve CRLF (mismatch esperado). Invariante = .gitattributes.
- **D3**: phantom-diff post-C7a — colaboradores con worktree sucio verán todo modificado (esperado; se resuelve con --renormalize).
- **D4**: append-43 NO toca historial — blobs previos siguen CRLF; supersesión cubre ℛ, NO el historial.
- **D5**: (ya declarado) cero fuentes \r ⇒ escalera NO falsificada.