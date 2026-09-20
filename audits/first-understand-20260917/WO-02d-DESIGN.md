# WO-02d · DESIGN — terminus relays-client + shared-rs (read-only deliverable)

> kind: design · 2026-09-17 · Charter: fichas del terminus §34.3 + tipos canónicos.
> Entregable principal: `02d-RELAYS-CLIENT-SHARED.md` (fichas con file:line).
> Este documento registra los hallazgos de diseño, los diffs PROPUESTOS (no
> aplicados — producción intocada por charter), el invariante y el gate.
> Reglas duras respetadas: RULE 00, §32/§33, §34.3 (default-deny/MainnetRefused
> INTOCABLES), NO-GIT, cero cargo/npm/build.

## D1 · Drift doctrina↔código: "MainnetRefused" no existe en Rust

- **Evidencia**: `live_exec_policy.rs:6-11` define `LiveExecDenied { NotEnabled, ChainNotAllowed }`.
  Grep global backend: `MainnetRefused` aparece SOLO en comentarios TS de api-server
  (`control-board.ts:94,173,742`, `control-board-drift.ts:90`). Además el código
  SOPORTA mainnet explícito (`live_exec_policy.rs:71-76` test `explicit_mainnet_is_supported`;
  `bundle_builder.rs:433-434`), y el default Sepolia-only está en `live_exec_policy.rs:3`
  + test `enabled_default_is_sepolia` :92-96.
- **Riesgo**: un auditor que busque `MainnetRefused` en Rust concluye falsamente que
  el control no existe (falso-negativo de seguridad); o cree que mainnet es
  físicamente imposible y no audita el allowlist explícito (falso-positivo).
- **Diff PROPUESTO (documental, comentario only — NO aplicado, gated por operador)**:

```diff
--- b/backend/relays-client/src/live_exec_policy.rs
+++ b/backend/relays-client/src/live_exec_policy.rs
@@
 //! Explicit per-chain activation, including mainnet. Every signing attempt
 //! checks this policy; malformed configuration never expands the allowlist.
+// WO-02d (2026-09-17) §34.3 naming note: doctrine and api-server comments call
+// the mainnet denial "MainnetRefused". In code it materializes as the
+// default-deny (`enabled == Some("true")`, line ~37) PLUS the default
+// allowlist DEFAULT_LIVE_CHAINS = [Sepolia 11155111] (line 3), so chain_id=1
+// yields `LiveExecDenied::ChainNotAllowed` unless ARBX_LIVE_EXEC_CHAINS lists
+// it EXPLICITLY. There is no separate MainnetRefused variant by design:
+// allowlist-explicit, not chain blacklist.
 pub const DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111];
```

  Alternativa equivalente (capa TS): corregir `control-board.ts:94,173,742` para
  nombrar `ChainNotAllowed`/default-deny. UNA de las dos, no ambas (P-∅ un PR = un ID).

## D2 · GAP: canon `docs/EXECUTION_MODES_DOCTRINE.md` ausente

- CLAUDE.md §34 lo cita como "fuente de verdad detallada" y este charter como canon.
  El archivo NO existe (glob negativo). La doctrina viviente está en CLAUDE.md §34.1-34.5.
- **Acción propuesta**: NO crear el archivo desde este WO (sería invención de canon
  sin gate del operador). Se reporta como GAP al board; el operador decide si lo
  crea o corrige la referencia en CLAUDE.md.

## D3 · Propuesta (parked, gated): materializar el canary §34.5 como control técnico

- Hoy el canary ($350 / 5 WETH) es solo doctrina (grep negativo en runtime).
  Materialización SIN tocar live_exec_policy: el operador fija en VPS
  `ARBX_LIVE_PRINCIPAL_CAP_1_<weth-hex>=5` y `trading_config.capital_usd` para
  chain 1 — ambos controles YA existen y son fail-closed
  (`bundle_builder.rs:90-107`, `execution_admission.rs:193-201`).
  Cero cambios de código requeridos → este ítem es SOP de operador, no diff.
  (NOTA: ejecutarlo es cambio de VPS/env = CHANGE_AUTHORIZED, gated; este WO no lo ejecuta.)

## INVARIANTE del terminus (verificable por lectura)

```
broadcast(opp) ⇒
    ARBX_LIVE_EXEC_ENABLED == "true" (exacto)                       [live_exec_policy.rs:37]
  ∧ opp.chain_id ∈ ARBX_LIVE_EXEC_CHAINS (default solo 11155111)    [live_exec_policy.rs:3,41-52; bundle_builder.rs:57]
  ∧ opp.rejection_reason == None                                    [submit_engine.rs:92-112, R-0001]
  ∧ ValidatedPlan presente, parseable, binding fresco (<30 s o re-sim real) [submit_engine.rs:442-484; execution_admission.rs:41-137]
  ∧ checklist 12/12 PASS (killswitch, paper, chain, cfg, rpc, gas, floor, tokens, fábricas, slippage, mempool, breaker) [pre_execute_checklist.rs:212]
  ∧ principal ≤ min(max_value_eth, ARBX_LIVE_PRINCIPAL_CAP_*)       [bundle_builder.rs:90-107]
  ∧ economics: admitted > 0 ∧ admitted ≥ 3×gas ∧ amount_in ≤ capital_usd/50 [execution_admission.rs:185-201]
  ∧ eth_callBundle PASS (o endpoint fail-closed → drop)             [submit_engine.rs:598-748]
```
Además, invariante de modo: paper ⇒ NUNCA build→broadcast reachable sin short-circuit
(`submit_engine.rs:560-596` + `:188-217` + checklist `PaperModeActive` :334-371) y
sin signer `/execute` responde 501 salvo paper (`consumer_spawn.rs:45-47`).

## GATE de no-regresión propuesto (para WO-05/06, sin ejecutar aquí)

1. `cargo test -p relays-client live_exec_policy` — 5 tests: `defaults_are_disabled`,
   `explicit_mainnet_is_supported`, `invalid_allowlist_never_partially_activates`,
   `exact_true_only`, `enabled_default_is_sepolia` (live_exec_policy.rs:59-96).
2. `cargo test -p relays-client bundle_builder` — incluye los 3 asserts de policy
   en `bundle_builder.rs:416-434` (denegado por defecto, cadena fuera de allowlist,
   mainnet explícito permitido).
3. `cargo test -p relays-client consumer_spawn` — 4 tests de política paper/live
   (consumer_spawn.rs:53-78).
4. `cargo check --workspace` ya corre el orquestador (restricción: yo NO ejecuté
   cargo de ningún tipo — verificación pendiente en WO-05; FAIL-HONEST).

## Verificación realizada en ESTE WO (solo Read/Grep/wc)

- Lectura completa: live_exec_policy.rs (97 l), main.rs (551), consumer_spawn.rs (79),
  signer.rs (231), contracts.rs (203), lib.rs (46), execution_admission.rs (212),
  plan_validation.rs (1-200), submit_engine.rs (1-949 de 1537; resto inspeccionado
  por grep de helpers), bundle_builder.rs (1-140 + grep tests), consumer.rs (1-120),
  persistence.rs (14-143), settlement_accounting.rs (1-80), paper_mode.rs (1-120).
- Greps dirigidos: enforcement de policy (10 hits), `arbx:opps:simulated` /
  `arbx:validated_plan` (productores/consumidores), `capital_usd`, default-fns de
  config, firma pública de 30+ módulos, headers `//!` de todos los módulos fichados.
- Cero modificaciones a producción; cero git; cero VPS.

## Notas para la mesa (pares posteriores)

- 02a: confirmar punto exacto de construcción de `Opportunity` y escrituras de
  `arbx:validated_plan` (scanner.rs:2449, candidate_simulation.rs:582) — el terminus
  es fail-closed sobre esa key; si 02a encuentra OTRO productor no listado en
  §4 de 02d-RELAYS-CLIENT-SHARED.md, publicar la discrepancia en el board.
- 02b: el admisor final de estrategia es `plan_validation::supports_strategy`
  (plan_validation.rs:17-32) — 8 cartuchos MEV-01 + 3 familias base. Si vuestro
  mapa operador→cartucho dice que otros operadores llegan al terminus, el gap está
  aquí (cuello de botella real del pipeline, no upstream).
- 02c: `execution_admission::refresh` re-simula con `simulator_v2::SimulatorV2`
  (fork head, `require_positive_net_profit`) — es la segunda entrada de simulación
  además de sim-ctl; documentarla en la fase validar/simular.
