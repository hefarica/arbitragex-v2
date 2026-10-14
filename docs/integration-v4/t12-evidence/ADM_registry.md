# ADM Registry — Track 𝒯-D/E (t12) · familias §12-D (ejecución atómica) + §12-E (mempool/bundles)

> Productor: t12/edge-dev, attempt be083f61-c870-4f55-8ec9-1722c04fd4f5 (2026-10-13).
> Continúa la numeración global de ADM (t4 = ADM-1..4). Disciplina 𝒯 idéntica: π(t) → μ vs x_ref (t9) → λ → admisión μ≥δ ∧ 𝒞 ∧ λ∈{P,R}.
> LÍMITES DOCTRINALES vigentes: PAPER ON — código/detección defensiva ÚNICAMENTE; ¬broadcast, ¬firma, ¬capital; I1 (no tocar hot path fuera de t9); P5 (¬tocar MEV detection/searcher P&L).
> Fail-honest: toda afirmación cita ruta:línea o comando; μ no computable se declara como tal, jamás como 0 medido.

## 0. Insumos congelados

| Insumo | Valor | Fuente |
| --- | --- | --- |
| x_ref | igual que t4 (`releases/R-20261013T085048Z/x_ref.json`, sha a6d521ea…, ℛ.tsv:7) | t9 |
| x₄ baseline | gas_usd = 0.632416 **CONSTANTE** (n=17/26, valor único; "sesgo interno de estimación NO computable desde el wire") | x_ref.json `x4_gas` |
| x₆ baseline | −17,915.4383 USD suma de `simulated_net_profit_usd` (candidatos RECHAZADOS simulados); **ledger ejecutado NO medible** (3×404 + ssh 255 + sql sin filas) | x_ref.json `x6_pnl_simulado` |
| δ | Para coordenadas en ms: 0.02·\|J(θ)\|. Para x₄ (USD): δ = max(0.02·\|J(θ)\|, 0.005); x₄=0.632416 ⇒ δ=0.0126483 USD. Para x₆: J(θ) es suma de ventana ⇒ δ=0.02·17915.44=358.3 USD — pero μ(x₆) requiere ledger ejecutado (NO COMPUTABLE en paper). | x_ref.json + derivación |
| 𝒞(θ) | canónica=1 (transport/infra no toca SSOT ⇒ 𝒞(θ⊕π)=𝒞(θ) para todo candidato de esta familia) | C_THETA_verdict.json:67 |

## 1. Resultado de inventario θ (grep 2026-10-13, sesión t12)

Comandos (salidas en transcripción t12 + extractos en `ADM_inventory_greps.txt`):
- `grep "flashLoan|flash_loan|flashloan|executeOperation|onFlashLoan|flashLoanSimple" backend` → 602 matches (250 mostrados)
- `grep "UserOperation|EntryPoint|account_abstraction|erc4337|TSTORE|TLOAD|transient_storage" backend` → **0 matches**
- `grep "flashbots|mev-share|mevshare|mev_share|mev-boost|mevboost|jito|bloxroute|beaverbuild|titan" backend` → 147 matches
- `grep "nonce|priority|gas_price|max_fee" backend/relays-client/src` → 92 matches
- `grep "sandwich|backrun|frontrun_detect|mempool_listener|pending_tx" backend/searcher-rs/src` → 61 matches
- `grep "mev_blocker|mevblocker|MEV_BLOCKER" backend/relays-client/src` → 23 matches
- `Get-ChildItem contracts\src -Recurse -Filter *.sol` → 27 archivos (FlashLoanExecutor, ArbitrageExecutor, 4 flashloan adapters, DeterministicFactory, 9 DEX adapters)

## 2. Veredictos D (ejecución atómica)

| ADM-n | t | presencia en θ | π(t) | λ | μ | veredicto |
| --- | --- | --- | --- | --- | --- | --- |
| ADM-5 | Flashloan Aave v3 + Balancer + dYdX + UniV3 + callbacks | **PRESENTE**: `contracts/src/flashloans/{AaveV3FlashAdapter,BalancerFlashAdapter,DyDxFlashAdapter,UniV3FlashAdapter}.sol`; math `backend/shared-rs/src/flashloan_math.rs:19-218` (premium Aave/Balancer, capacity, repayment); encoder `backend/prioritization-spine/src/execute_arbitrage_encoder.rs:238` `build_request_flash_loan_calldata`; worker `backend/searcher-rs/src/workers/flashloan_arb_worker.rs`; fee knob `flashloan_fee_pct=0.0009` (trading_config.rs:766) | n/a (π∈θ) | MIT/adaptadores propios | **μ incremental = 0** | **0 (ya presente)** |
| ADM-5b | Flashloan Morpho + Euler (sub-proveedores) | **AUSENTE** (no hay adaptadores; registry-engine.ts:546 lista sólo aave/balancer/dydx/uniswap_v3) | π apunta a ↓fee-flashloan (NO x₄; el fee es knob de config, no gas) ⇒ sin coordenada medida afectada | P (ambos MIT/Apache, ERC-4626 estándar) | μ NO COMPUTABLE: exige adaptador nuevo + contrato desplegado + fork-sim; I1 congela el hot path y el terminus | **0 (bloqueado: π sin coordenada medida ∧ μ no computable sin deploy/fork ∧ I1)** |
| ADM-6 | Flash-mint | **PRESENTE** (estructura): `backend/searcher-rs/src/cartridge_boot.rs:489` "flashloan_atomic / flashmint_atomic arms" (brazos declarados, estructuralmente muertos sin cartridge); categoría flashmint_atomic en route_applicability.yaml:61-66 | n/a | — | μ incremental = 0 (el brazo existe; activarlo = producto de cartridge, no de este track) | **0 (ya presente el brazo; activación es vía cartridge, no 𝒯-D)** |
| ADM-7 | EIP-1153 (transient storage) | **AUSENTE** (grep TSTORE/TLOAD/transient = 0) | ↓x₄ (gas) | P (estándar EIP) | μ NO COMPUTABLE: el delta de gas exige contrato nuevo desplegado + tx real/fork; PAPER ON ¬broadcast ∧ I1 hot path | **0 (bloqueado: μ no computable en paper)** |
| ADM-8 | Minimal proxies / deployment factory | **PRESENTE**: `contracts/src/core/DeterministicFactory.sol`; FLE desplegado por spec en paper (`backend/sim-core/src/paper_executors.rs:347` "paper_deploy_flashloan_executor", grantRole :385-386) | n/a | — | μ incremental = 0 | **0 (ya presente)** |
| ADM-9 | ERC-4337 (account abstraction) | **AUSENTE** (grep UserOperation/EntryPoint/4337 real = 0; los 43 matches previos eran coincidencias numéricas sqrtPriceX96) | ↓tx/ruta teórico | P (EntryPoint MIT) | μ NO COMPUTABLE: infra bundler + broadcast; fuera del terminus paper | **0 (bloqueado: μ no computable en paper ∧ fuera de universo searcher)** |
| ADM-10 | EIP-7702 | **DEPENDENCIA presente, CAMINO ausente por doctrina**: `alloy-eip7702` en Cargo.lock:230; awareness `backend/simulator-v2/src/sequence_runner.rs:554` ("EIP-7702 delegation"), `backend/api-server/src/routes/readiness-extras.ts:335` ("7702-safe EXECUTOR_1 doctrine note") | ↓x₄ (warm SLOADs) | P (estándar, crate Apache/MIT) | μ NO COMPUTABLE: delegación 7702 exige firma+broadcast de txs reales (¬firma ¬broadcast PAPER ON) | **0 (bloqueado doctrinal: PAPER ON ¬firma; I1)** |
| ADM-11 | Nonce manager | **PRESENTE**: `backend/relays-client/src/nonce_manager.rs:1-82` ((chain,addr)→next_nonce, semaphore, refresh RPC); orphan-nonce resync `backend/relays-client/src/submit_engine.rs:1016-1038` ("never fabricates a nonce") | n/a | — | μ incremental = 0 | **0 (ya presente)** |
| ADM-12 | Priority fee | **PRESENTE**: `backend/relays-client/src/bundle_builder.rs:131-183` (validación priority_fee_gwei, `max_priority_fee_per_gas`/`max_fee_per_gas`, max_fee≥priority), WO-04 `submit_engine.rs:495` | n/a | — | μ incremental = 0 | **0 (ya presente)** |

## 3. Veredictos E (mempool/bundles)

| ADM-n | t | presencia en θ | π(t) | λ | μ | veredicto |
| --- | --- | --- | --- | --- | --- | --- |
| ADM-13 | Flashbots | **PRESENTE**: `backend/relays-client/src/relay_flashbots.rs` (cliente completo, auth header `signer.rs:38`, callBundle staging :594); submit_engine.rs orquesta; dry-run sin firma `backend/sed-core/src/connectors/flashbots_simulator.rs:8` ("Dry-run via eth_callBundle (NO signing)") | n/a | — | μ incremental = 0 | **0 (ya presente)** |
| ADM-14 | mev-share | **PRESENTE** (cartridge): `mev_09_018_mev_share_backrun_arbitrage` (cartridge_wave_de_test.rs:2145, origen "mev_share" :972) | n/a | — | μ incremental = 0 | **0 (ya presente vía cartridge)** |
| ADM-15 | mev-boost | **NO APLICA**: mev-boost es middleware validador (PBS); el searcher se integra a los BUILDERS vía relays (flashbots/titan/bloxroute = ecosistema mev-boost ya cubierto). π=∅ en el plano searcher | n/a | Apache-2.0 | μ=0 (sin coordenada: no existe plano validador en el sistema) | **0 (N/A al universo del sistema)** |
| ADM-16 | Jito | **AUSENTE y NO APLICA**: grep jito=0 matches; Jito es bundle engine de Solana; el sistema es EVM (chains 1/137/etc.) | ∅ | MIT | μ=0 (fuera de universo EVM) | **0 (N/A: Solana-only vs stack EVM)** |
| ADM-17 | bloXroute | **PRESENTE**: `backend/relays-client/src/relay_bloxroute.rs` (cliente completo :52-128); cred validators `backend/api-server/src/credentials/validators.ts:238` | n/a | — | μ incremental = 0 | **0 (ya presente)** |
| ADM-18 | Backrun/sandwich (detección defensiva) | **PRESENTE**: `backend/searcher-rs/src/engines/backrun_engine.rs:1` ("Non-extractive backrunning — Zero Victim"), `workers/backrun_worker.rs`, `block_scanner.rs:1-11` (FASE OMEGA backrun confirmado post-state), readiness gate G-MEV-1 `backend/api-server/src/readiness/verifiers/g-mev-1.ts:6` (REQUIRED_TERMS sandwich/frontrun/flashbots); doctrina no-sandwich-ofensivo `cartridge_boot.rs:837` ("no_sandwich","no_frontrun") | n/a | — | μ incremental = 0 (detección ya integrada; el sandwich ofensivo está PROHIBIDO por doctrina y NO se postula) | **0 (ya presente; ofensiva prohibida — no candidata)** |
| ADM-19 | Solver auctions / intents | **PRESENTE (shadow)**: `backend/searcher-rs/src/route_discovery/strategy_applicability.rs:1009` `intents_solver_engine: {enabled: true, shadow_only: true, gates: [relay_intent_feed]}` | n/a | — | μ incremental = 0 | **0 (ya presente, shadow_only)** |
| ADM-20 | Private RPC | **PRESENTE** (stack relay): wire-shapes MEV-Blocker `backend/relays-client/src/relay_no_submit_sim.rs:15,116,259` (validate_mev_blocker, orden flashbots/mev_blocker/titan submit_engine.rs:1491); Protect endpoint manejado `backend/searcher-rs/src/chain_client.rs:455` (`wss://rpc.flashbots.net`); §9 doctrine mempool privado (CLAUDE.md Risk Mgmt 5) | n/a | — | μ incremental = 0 | **0 (ya presente vía relay stack; sin cliente submit dedicado mev-blocker — sub-gap declarado, μ no computable sin broadcast)** |
| ADM-21 | Decode pending tx | **PRESENTE**: `backend/searcher-rs/src/connectors/mempool_listener.rs:33-44` (observer-only subscribe_pending_txs + NormalizedTx), `scanner.rs:1665 process_pending_tx`, `route_decoder.rs:11` ("multicall decomposition §3.3.1"), `calldata/univ3.rs`, `impact_index.rs:550-563` (selectores multicall) | n/a | — | μ incremental = 0 | **0 (ya presente)** |
| ADM-22 | State diffs / post-state | **PRESENTE** (mecanismo): `backend/searcher-rs/src/state_projector.rs` (proyección de estado batched), `dirty_signal.rs` + dirty_trace fixtures (dirty set), gates post-state `strategy_applicability.rs:429` ("confirmed new-block post-state imbalance"), block_scanner post-state backrun | n/a | — | μ incremental = 0 | **0 (ya presente como post-state projection; "state diff" literal de reth no — sub-gap declarado)** |

## 4. Síntesis de admisión

- **Admitidos: 0.** Todos los presentes ⇒ μ incremental 0 (no re-implementar). Todos los ausentes ⇒ μ NO COMPUTABLE bajo PAPER ON (¬firma/¬broadcast/¬capital), I1 (hot path congelado) o N/A de universo (Jito=Solana, mev-boost=validador). **Ningún candidato de D/E puede demostrar μ≥δ con artefacto local bajo los límites doctrinales vigentes** — veredicto mecánico honesto, ¬ re-evaluar (§𝒯).
- **Sub-gaps declarados** (para futuras ventanas no-paper, requieren autorización de flip + V1..V4): ADM-5b (Morpho/Euler adapters), ADM-7 (EIP-1153), ADM-9 (4337), ADM-10 (7702 camino de ejecución), ADM-20 (cliente submit mev-blocker), ADM-22 (state diff reth literal).
- **V1..V4: VACANTES POR DISEÑO** — el contrato exige V1..V4 para "TODO ADM-n ADMITIDO"; con |admitidos|=0 no hay incremento que gatear. Nada entró al release desde t12 (cero changedPaths de código).
- **§D fallas**: ninguna — los 7 comandos de inventario completaron (0 timeouts, 0 errores).
- **I1/I2/I5/P1/P5**: intactos — t12 no tocó hot path, no editó el VPS, no flip, cero secretos.

## 5. Cadena de custodia

| artefacto | sha256 |
| --- | --- |
| docs/integration-v4/t12-evidence/ADM_registry.md (este archivo) | (ver R_addendum.tsv) |
| docs/integration-v4/t12-evidence/ADM_inventory_greps.txt | (ver R_addendum.tsv) |
