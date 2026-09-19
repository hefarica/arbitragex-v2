# BOARD — PROGRAMA HFT-1000% (orden del operador 2026-09-17)
> /goal: cobertura de red entera + etiquetado a escala + dataset académico + base searcher
> real + rigor + paridad backtest/live. Integración base+delta (§9 live-eng), NADA se rompe.
> Promesa ajustada por el orquestador: velocidad y letalidad SÍ; "riesgo cero" = riesgo
> acotado por gates (canary, kill-switch, G1-G8). Doctrina §12: Hermes convoca los gangs.

## Reconciliación plan ↔ repo REAL (hecha por el orquestador)
- Fase 5 A.6/A.7/A.9: LEDGER dice CERRADOS 2026-09-07 (7 PRs, dashboard 1 blocker).
  → WO-R5a: RE-VERIFICAR con evidencia fresca (no re-implementar a ciegas).
- Paths del plan que NO existen: backend/crates/collector-rs, services/*, crates/*,
  crucible/, pipelines/, schemas/. Mapa real: backend/searcher-rs (collector+detección),
  backend/math-engine (31 ops), backend/sim-ctl|sim-core|simulator-v2 (sim), edge/worker,
  backend/api-server. El builder de cada WO mapea plan→real ANTES de tocar.
- Crucible 72h → en este repo = soak con SIM_BACKEND + monitoreo (equivalente a armar).
- Stage 2b labels/calibración → backend/prioritization-spine + recon/stage2_calibration
  (existe: stage2_calibration.rs citado en fichas WO-02).

## WOs (secuencia del plan, ajustada)
- WO-R5 · Rigor primero: re-verificar A.6/A.7/A.9 con evidencia + sign-off script + soak 72h
  + LABEL_SOURCE=live cuando los labels fluyan. Owner: gang (Hermes convoca).
- WO-F1 · Cobertura red: registro de pools multi-chain (mapear a pool_sync_worker +
  recon + enum reactivo; INV: pools EVM indexados). Gate: conteo por chain con evidencia.
- WO-F1b · RPC fan-out multi-chain (rpc_failover existe — extender, no reescribir).
- WO-F2 · Etiquetado a escala (schema opportunities_labeled + labeler en hot-path +
  agregadores Redis + PG particionado + bench p99).
- WO-F3 · Dataset académico (snapshots → Parquet arrow-rs + manifest SHA-256 + CI verify).
- WO-F4 · Base searcher real (monitor bundles Flashbots/MEV-Share/Titan + perfiles +
  señal competencia e32 al scoring).
- WO-F6 · Paridad backtest/live (modo historical en sim + parity_check + gate CI KS).
- REGla anti-daño: cada WO = PR propio con ID (§37), cargo test suite completa, cero
  reformat ajeno, Rust serie (§36.4), gates §34 intactos.

## En vuelo (NO se detiene por este programa)
- Builder FEE-TIER-AWARE-QUOTING (cuello #1 actual) corriendo → es prerrequisito de TODO
  lo anterior (sin accepts no hay labels que etiquetar a escala).
