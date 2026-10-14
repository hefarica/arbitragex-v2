# V5 — SHADOW (t6): Φ(shadow)=1 ∧ x(θ) dentro de tolerancia. NADA PUBLICADO.

> Definición operativa de shadow en esta escalera: el paquete R-20261013T1545Z-t6 se valida
> INTEGRAMENTE en staging local (worktree limpio + herramientas del release) contra el sistema
> vivo θ por lectura; **0 bytes escritos en el VPS** (confirmado por probe: deploy.sha sin cambio).

## Φ(shadow) = g1∧…∧g7 = 1

- **g1..g6** (SOLO del A1 origen, anti-circularidad §7): `python tools/phi_static.py
  .tmp-inbox/integrado-supremo/MANIFEST.txt` → 7/7 PASS, `PHI_STATIC(g1..g6)=1`, exit=0
  (salida literal en V2_static.txt).
- **g7** (cap nuevas +3, INSPECCIÓN): 3 capacidades admitidas e implementadas en `adm/`
  (ADM-52 sizing-AMM-cerrada+Kelly, ADM-54 staleness-oráculo, ADM-55 ventanas-no-atómicas),
  re-test propio `python -m unittest test_adm -v` → **Ran 10 tests … OK** (exit 0).
- **𝒞(θ-SSOT)**: `python tools/c_evaluator.py SSOT --canonical` → `C(theta)=1`, exit=0
  (modo literal exit=1 por la errata cond7 documentada — enmienda α11 capitán aplicada).

## x(θ) tolerancia (δ_abs=0.005, t4/t9)

| coordenada | V0 (15:37Z) | probe V5 (15:5xZ) | Δ | veredicto |
|---|---|---|---|---|
| viables (ventana) | 0 | 0 | 0 ≤ 0.005 | PASS |
| rechazo feed | 100.0% (35/35) | 100.0% (26/26) | 0 | PASS |
| deploy.sha (activo) | 5f8ea3d67 | 5f8ea3d67 | 0 (sin deploy, por construcción) | PASS |
| killswitch.enabled | false | false | 0 | PASS |
| gas_usd | 0.62745945 | (misma fuente interna) | Δ_v0_vs_xref=0.0050 ≤ δ_abs | PASS |

Nota fail-honest: primer intento del probe → timeout 60 s (borde lento: runtime-status 8.98 s
en V0); reintento por partes: live exit=0 en 11.4 s totales. Registrado, no oculto.

## Harness t3/t4/t5 (insumos de la escalera)

- t3: harness schema/mask/ledger/deriv/gated montado (contrato t3 ✅; guard pii 6 timeouts
  aislados — baseline compuesto §17).
- t4: seam SSE (D0193916) — el único admitido de t4; **△ t14 CERRADO por re-ejecución**:
  este release ejecuta V1-V4 sobre el conjunto completo y la fila ADM-1 referencia ESTA
  ejecución (measure_mu_output.json C2280DB3… queda como antecedente, slice capitán).
- t5: dispatcher+guard (3C26994A) + vitest 1418/1418 frontend (t5 §7) — citado.

## Conclusiones de V5

1. **V5 VERDE** ⇒ la escalera puede avanzar a V6 SOLO con: quórum §7 completo + lock
   `/tmp/arbx-deploy.lock` + decisión del operador (topología + backup R9).
2. P1 respetado: 0 commits/push/pull; staging local. P5: PAPER ON + kill-switch intactos.
