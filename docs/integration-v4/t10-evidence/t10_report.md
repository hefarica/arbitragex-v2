# t10 — 𝒯-A/B: ciclos/rutas y AMM exacta → ADM-n (A7)

Attempt 1 c9445ad8-1d1e-4d63-b342-d40a24625518 · 2026-10-13 · θ = árbol compartido feat/exp-20261013
(+promoción ADM-0) · x_ref = XREF_BASELINE t16 (7a03e242…/089ba911…) · δ = max(0.02·|J|, 0.005).

## Primera acción (orden capitán, enmienda α11-20261013): PROMOCIÓN ejecutada

`gen/detector_policy.rs` (+134/−0, DFAC['D_']:=0.80) promovido al árbol compartido con
tmp→fsync→rename; sha256 promovido `5698976fd2573be7…` == generado; fila ℛ.tsv appended;
`git status --porcelain` = ` M backend/searcher-rs/src/detector_policy.rs` (único cambio intencional
mío en el árbol). V1..V4 de esta promoción: V1=ℛ; V2=V2_static.txt de t9 (artefacto idéntico);
V3=re-ejecutado sobre worktree limpio 21feefb32 + archivo promovido (salidas en V3/, gate
**CERO FALLOS NUEVOS** vs baseline 3 liquidation_snipe exonerados); V4=V4_dryrun.md t9 sigue
válida (mismo destino, creación limpia).

## Principio de medición (aplica a TODOS los candidatos §12-A/B, 3 vías §D exploradas)

μ(t;θ)=J(θ⊕π(t))−J(θ) **vs x_ref del wire**. Sin deploy no existe θ⊕π(t) en el wire y P1/P5
prohíben desplegar (¬flip, árbol local = fuente de verdad). Las 3 vías: (i) medir contra el wire
— imposible sin deploy; (ii) proxy local (benches/discovery_matrix: ns/iter del discovery; tests
de quote) — mide el componente, NO x₁/x₂ del x_ref; (iii) RPC público — mide exactitud de quote,
no x₂ (RTT HTTP del borde) ni x₁. ⇒ μ NO COMPUTABLE para todo candidato §12-A/B bajo P1/P5.
Fail-honest: NO COMPUTABLE se declara como tal (no se rellena con 0 ni con proxy).

## Por qué además §12-A no movería x_ref aunque μ fuera computable

x_ref (t16): motor 196.956 candidatos/h (dex_arb 190.626 + triangular 6.330), **100% rechazo
ECONÓMICO** (`spread_negative_round_trip` dex_arb, `non_positive_profit` triangular; feed latest
26/26 rechazos), x₁ viables=0, bridge budget saturado (31.556 cap-rechazos/20 min con cap 12/block
— hop_cycle_bridge.rs:50-56). El cuello del sistema vivo es la ECONOMÍA (spread/profit), no la
generación de ciclos: ↑ciclos o ↓nodos/ciclo no cambian x₁_viables con el mismo estado de mercado.

## Tabla (comando+salida por fila en ADM-n_A7.tsv)

- ADM-0 promoción: **ADMITIDO (por enmienda α11)** + V1..V4.
- ADM-1 (BF negcyc) y ADM-7 (xyk+fee): **PRESENTE** en θ (spanning_tree_engine.rs;
  amm_math.rs:96 + tests exactos) — verificación, no candidatos.
- ADM-2..6 (SPFA/GR, Johnson SCC, Yen/Eppstein, Hawick-James, DP bitmask): **0** — μ NO
  COMPUTABLE + discovery no es cuello (arriba).
- ADM-8..13 (v3 tick local, LiquidityAmounts, v4 hooks, stableswap, Balancer, Solidly): **0** —
  μ NO COMPUTABLE (x₂ inamovible sin deploy). Tres HALLAZGOS estructurales documentados como
  especificación (NO implementados sin μ): (1) kernel `V3MultilegUnsupported` con producción
  585/585 ciclos V3-bearing unpriceable (hop_cycle_bridge.rs:54-57); (2) Curve/Balancer
  aproximados como V2V2 hoy (dex_engine.rs:653-655); (3) v4 hooks: quote local indecidible por
  diseño (hook arbitrario ⇒ solo simulación).

## Licencias (λ informativo para los 0)

uniswap-v3-math MIT · curve-math (vyper) MIT · BalancerMath MIT — ports futuros bajo
docs/security/FUSILE_SOURCE_POLICY.md (port-with-validation). Ningún port se ejecutó en t10.

## Parada 𝒜(θ)=∅ ∨ |μ(t*)|<ε — aporte para t14

Ningún t* con μ demostrable ≥ δ: |A7_admitidos_§12| = 0 (solo ADM-0 por enmienda, fuera de §12).
