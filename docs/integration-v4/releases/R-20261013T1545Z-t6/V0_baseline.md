# V0 — SNAPSHOT + BASELINE del sistema vivo (t6, antes de cualquier V6)

> Instrumento: HTTP público al borde (`https://arbx.ape-tv.net` / `https://edge-arbx.ape-tv.net`).
> Artefactos crudos: `../../t6-evidence/v0_status.json`, `v0_live.json`, `v0_runtime.json`, `v0_health.json`.
> Capturado 2026-10-13T15:37:40Z (ts del feed). VPS inalcanzable habría sido BLOQUEO (no ocurrió).

## Λ_vps — /api/status (canónico)

- HTTP 200 en 0.851 s. `ok:true`, **7/7 servicios OK** (selector-api, sim-ctl, recon, relays-client,
  searcher-rs, math-engine, token-enricher). `killswitch.enabled=false` (reason `VER`, admin,
  2026-10-07T01:14:38Z — PAPER ON + kill-switch intactos).
- `deploy.sha=5f8ea3d67c…8dfb` **== git HEAD del VPS (V-MAP-Δ p1-p3) == A0** ⇒ drift=0.
- `deploy.at=2026-10-13T06:16:24Z`, id 38353373370. `/api/health`: api-server v0.1.0 uptime 31825 s.

## Feed /api/opportunities/live (ventana latest, max_age 300 s)

- `count=35`, `window_total=35`, `window=latest`. **Viables=0** (`?viable_only=true` → count=0).
- Histograma de rechazos (35/35): spread_negative_round_trip=15, non_positive_profit=6,
  **v3_multileg_budget_exhausted=6**, single_pool_no_spread=4, no_tradable_size=1,
  TokenNotAllowed:0x236e…=1, v3_pair_no_pools=1, v3_pool_not_catalogued=1.
  (vs x_ref t16: aparecen 3 razones NUEVAS de universo v3 — el kernel V3MultilegUnsupported
  documentado por t10 como hallazgo estructural se manifiesta en el wire; registrado, no adaptado.)
- `pipeline_latency_ms`: n=35, p50=1405, p99=2449, max=2449.
- `gas_usd` (breakdown): n=19/35, constante 0.62745945 USD/ruta (x_ref t16: 0.6324159 — el valor
  es estimación interna dinámica; delta −0.0050 ≈ δ_abs=0.005 — DENTRO de tolerancia).
- Runtime-status (edge): HTTP 200 en 8.98 s (lento; registrado), postgres ok, redis ok.

## CONDICIÓN F4 CONOCIDA DEL BASELINE (V7 compara contra ESTO)

- **Heredada (t17 F4)**: consistencia interna del wire — recomputando
  `amount_out_usd := legs[-1].amount_out_wei / 10^dec × token_prices_usd[sym]` sobre el snapshot
  t16: **3/26 items con discrepancia >0.005 USD (máx 9.15 USD)**, rutas triangulares que cierran
  en token no-base (HEX×2, EURC×1). Semántica posiblemente de rutas abiertas; discrepancia medible
  y reproducible (`FALSATION_REPORT.md` F4 hallazgo secundario).
- **Re-verificado en V0 (t6)**: MISMO recomputo sobre los 35 items de `v0_live.json` →
  **0/35 discrepancias >0.005 USD** (script de sesión, salida en t6-evidence). La condición F4 es
  INTERMITENTE (depende de qué rutas no-base estén en la ventana). V7 debe comparar el histograma
  contra V0 ACEPTANDO la presencia conocida de esa clase de items, no como fallo nuevo.
- Estado para el quórum: **L3/F4 = "no refutado, NO MEDIBLE" hasta instrumento on-chain** (t17).

## x(θ) V0 (referencia para V5/V7, δ_abs=0.005)

| coordenada | V0 | x_ref t16 (pre-release) |
|---|---|---|
| viables (ventana) | 0 | 0 |
| rechazo % | 100.0 (35/35) | 100.0 (26/26) |
| p50 pipeline_latency_ms | 1405 | 1843 (m1) / 2713 (m2) |
| gas_usd | 0.62745945 | 0.6324159 (Δ=0.0050 ≤ δ_abs) |
| HTTP error rate | 4/4 endpoints 200 | 0.0% (20/20) |
