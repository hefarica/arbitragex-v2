# A7 — REGISTRO DE INCLUSIONES (track 𝒯, §13)

> Productor: t14/auditor · attempt `bb186f4f-bcf6-4f2c-ade8-a70775c03344` · 2026-10-13 · rama `feat/exp-20261013`.
> Consolidación de TODAS las ADM-n de los seis tracks 𝒯 (t4, t5, t10, t11, t12, t13). Numeración global nueva (los IDs originales colisionaban: t10 usó ADM-0..13 y t12 continuó la numeración de t4 ADM-5..22); procedencia conservada en la columna `t` como `track/id-original`.
> Fuentes (registros por track, con sus ℳ): `t4-evidence/ADM_registry.md` · `ADM-t5-frontend-bus.md` · `t10-evidence/ADM-n_A7.tsv` · `ADM-t11-quote-aggregation.md` · `t12-evidence/ADM_registry.md` · `releases/R-20261013T13HI-t13/T13_admission_table.md`.
> Falsador α7 (t17): F1–F12 **NO_REFUTADO** en los 12 ataques (`FALSATION_REPORT.md`) — ninguna línea de este registro fue refutada; el gate default-deny de μ∈CLAIM (F11) se respeta: 59/59 filas citan comando+salida/artefacto.
> Convención de veredicto: **1** = admitido e implementado (o promoción ejecutada); **0** = no admitido — incluye YA-PRESENTE-en-θ, μ NO COMPUTABLE, μ=0, N/A de universo y wire-sin-productor (todos estados terminales, ¬re-evaluar §5).

## Registro §13 — formato EXACTO `ADM-<n>|t|λ(t)|π(t)|μ(t;θ)|comando|veredicto(1/0)|fecha`

```
ADM-1|t4/G-1 SSE fallback GET /api/prices/stream (api-server+edge worker+dev-local, paridad ARBX-V-014)|P|x2 (J=641.6 ms/update); x3 descartada|mu_worst=337.543 ms >= delta=12.832 (margen 26.3x; m1=0.257 ms medido, w<=303.8 cota pesimista)|npx tsx docs/integration-v4/t4-evidence/measure_mu.ts -> t4-evidence/measure_mu_output.json sha256 C2280DB3…80B876|1|2026-10-13
ADM-2|t4/G-2 /health con release_sha (sirve a Lambda)|P|vacia (ninguna coordenada x1..x6 se mueve)|mu=0 (pi vacia)|inspeccion coordenadas releases/R-20261013T085048Z/x_ref.json (sha a6d521ea…, R.tsv:7)|0|2026-10-13
ADM-3|t4/G-3 404 honesto con cuerpo JSON|P|vacia|mu=0 (error rate baseline 0.0% n=20)|inspeccion x_ref.json + patron honest-404 pervasivo en ambos edges (t4-evidence/ADM_registry.md §2)|0|2026-10-13
ADM-4|t4/G-4 batching eth_call/Multicall3|MIT|{x3} — pi(t) EN theta YA|mu incremental=0 (ya presente)|grep aggregate3|Multicall3|multicall backend/searcher-rs/src -> 250 matches (t4-evidence/ADM_G4_multicall_grep.txt)|0|2026-10-13
ADM-5|t5/t_bus priceBus coalescing ARBX_PRICE_MIN_INTERVAL_MS=100 + snapshot + seq monotonicico|P|G-core (colapsar rafaga same-ms a 1 frame)|mu_mecanismo=0.9990 >= delta=0.01998 (999/1000 coalesced)|node --experimental-strip-types bus-bench.mts -> {"N":1000,"published":1,"coalesced":999}; vitest src/prices 4/4 (ADM-t5 §2.1)|1|2026-10-13
ADM-6|t5/t_seq createSeqGuard cliente (STREAM-SEQ-01, replay => 0 efecto)|P|G-core (replays con efecto cero)|mu_mecanismo=1.0 (200 vivos aceptados; 1000 replay + 1000 out-of-order TODOS rechazados)|vitest shared-ts prices-contracts.test.ts 7/7 + bench interno accepted=200 replayRejected=1000 outOfOrderRejected=1000 (ADM-t5 §2.2)|1|2026-10-13
ADM-7|t5/t_watchdog REEMITIR en vez de reconectar (mata 'already in CLOSING or CLOSED')|P|G-core (eliminar disconnect-storm en silencio)|mu_mecanismo=1.0 (3->0 disconnects forzados en 1h silencio; modelo, citas de codigo :204-220 legacy vs :39-45 v4) **[OBSERVADO en producción 2026-10-13 — t29: field-log del operador console-20261013-operator-field-log.md: storm ×6 ventanas con contador SIEMPRE 1/3 + 1 CLOSING/CLOSED + ~14 bfcache; sin timestamps (DECLARADO)**]|sandbox_js salida literal {"window_h":1,...,"mu_watchdog_disconnect_elimination":1}; CONNECT_OPTS intacto socket-lifecycle.ts:50 (ADM-t5 §2.3); ℳ field-log operador (docs/integration-v4/evidence/, ℛ fila t29)|1|2026-10-13
ADM-8|t5/t_lifecycle socket-lifecycle.ts callbacks opcionales onPriceTick/onPricesSnapshot (diff v4, mandato alfa11)|P|G-wiring (habilitador: sin callback socket identico)|no-regresion: 12 tests pre-existentes VERDES sin modificar + 7 nuevos; suite frontend 1418/1418|vitest features/opportunities 12+7 + suite completa 141 passed/1418 tests (ADM-t5 §2.4)|1|2026-10-13
ADM-9|t5/wire t_bus — CLAIM de wire vs x_ref|P|—|mu_wire=0 razon 'no wire producer' (0 llamadas priceBus.publish en runtime; paquete v4 sin publicador)|verificacion ADM-t5 §3 + x_ref t16 sin eventos prices:tick|0|2026-10-13
ADM-10|t5/wire t_seq — CLAIM de wire vs x_ref|P|—|mu_wire=0 'no wire producer'|idem ADM-t5 §3|0|2026-10-13
ADM-11|t5/wire t_watchdog — CLAIM de wire vs x_ref|P|—|mu_wire NO COMPUTADO (instrumentos WS no disponibles desde wire publico) => wire 0|t16 §7 via ADM-t5 §3|0|2026-10-13
ADM-12|t10/ADM-0 PROMOCION detector_policy.rs regenerado desde SSOT (+134/-0, DFAC D_:=0.80, enmienda alfa11 capitán)|P (artefacto propio t9)|promover gen/detector_policy.rs al arbol compartido tmp->fsync->rename|n/a (admitido por ENMIENDA alfa11-20261013; no candidato §12)|python promote: before=fed8cfcc3d6c promoted=5698976fd2573be7 OK=True; V1-V4 t10-evidence/V3 (cargo build exit=0; test 1431P/3F/5I con 3F==baseline exonerado => CERO FALLOS NUEVOS)|1|2026-10-13
ADM-13|t10/ADM-1 A: Bellman-Ford negcyc|P|n/a — YA PRESENTE en theta|n/a (theta ya lo contiene; sin cambio no hay mu)|Select-String bellman -> spanning_tree_engine.rs:1,66,120,145,173,290,291 (bellman_ford_cycles, peso -ln(rate) :66)|0|2026-10-13
ADM-14|t10/ADM-2 A: SPFA/Goldberg-Radzik|P|sustituir BF en spanning_tree_engine|NO COMPUTABLE vs x_ref (3 vias §D: sin deploy; bench local mide ns/iter no x1/x2; RPC mide quotes no discovery) + discovery NO es cuello (196.956 cand/h, 100% rechazo ECONOMICO, x1 viables=0)|grep SPFA|goldberg|radzik = 0 matches; XREF_BASELINE §1/§5/§6|0|2026-10-13
ADM-15|t10/ADM-3 A: Johnson SCC|P|SCC antes de negcyc|NO COMPUTABLE (idem) + no cuello (idem)|grep tarjan|scc|johnson = 0 matches; idem x_ref|0|2026-10-13
ADM-16|t10/ADM-4 A: k-shortest Yen/Eppstein|P|Yen sobre LineGraph L(G)|NO COMPUTABLE (idem). Repo YA tiene bounded DFS 2-3 + MMBF line-graph 2-7 (multi_hop_search.rs:1-16)|grep yen|eppstein|k_shortest = 0; headers unique_route_finder.rs / multi_hop_search.rs|0|2026-10-13
ADM-17|t10/ADM-5 A: enumeracion Johnson/Hawick-James|P|reemplazar enumeracion actual|NO COMPUTABLE (idem) + universo dinamico YA persistido (cycle_enumerator.rs enumera 2-3 sobre pools activos PG)|cycle_enumerator.rs:1-25; grep hawick = 0|0|2026-10-13
ADM-18|t10/ADM-6 A: DP bitmask|P|DP 2^h sobre HopMask (h<=7)|NO COMPUTABLE (idem). A06 hop_cycle_bridge YA materializa O(2^h) con HopMask_u8; presupuesto 12/block saturado 31.556 rechazos/20min (cap, no algoritmo)|hop_cycle_bridge.rs:100-123 (MIN=3/MAX=7, DEFAULT_MAX_PER_TICK=12, cap_reached :50-56)|0|2026-10-13
ADM-19|t10/ADM-7 B: xyk+fee exacta (V2)|P|n/a — YA PRESENTE en theta|n/a (theta ya lo contiene)|amm_math::v2_amount_out (amm_math.rs:96) con tests assert_eq exactos :890-940; consumido por dex_engine.rs:13-14|0|2026-10-13
ADM-20|t10/ADM-8 B: v3 tick local exact-in (sqrtPriceX96/Q64.96)|R (port uniswap-v3-math MIT; FUSILE port-with-validation)|port tick-traversal exact-in a Rust|NO COMPUTABLE (quote ±0.005 medible vs QuoterV2 RPC, PERO 'v x2' no movible sin deploy P1/P5). HALLAZGO: kernel V3MultilegUnsupported, 585/585 ciclos V3-bearing unpriceable|Select-String V3MultilegUnsupported -> hop_cycle_bridge.rs x6; :54 'measured: 585 of 585'|0|2026-10-13
ADM-21|t10/ADM-9 B: LiquidityAmounts|R|parte del port v3|NO COMPUTABLE (idem ADM-20)|grep LiquidityAmounts|liquidity_amounts = 0|0|2026-10-13
ADM-22|t10/ADM-10 B: v4 hooks quote local|no-lambda (IMPOSIBLE por diseño)|n/a|hooks arbitrarios mutan el swap: no existe quote exacta local sin ejecutarlo (solo revm). Criterio quote±0.005 local INDECIDIBLE para v4|grep v4 hooks|hook en quoters = 0|0|2026-10-13
ADM-23|t10/ADM-11 B: stableswap Newton (Curve)|R (port curve-math vyper MIT)|Newton D-iter local|NO COMPUTABLE (idem) + Curve hoy aproximado como V2V2 para el gate|dex_engine.rs:653-655|0|2026-10-13
ADM-24|t10/ADM-12 B: Balancer weighted|R (port BalancerMath MIT)|Bi/Wi exacto local|NO COMPUTABLE (idem) + misma aproximacion V2V2 activa|dex_engine.rs:299,303,653-655|0|2026-10-13
ADM-25|t10/ADM-13 B: Solidly|R (port solidly MIT)|math solidly local|NO COMPUTABLE (idem); 0 matches en repo|grep solidly = 0|0|2026-10-13
ADM-26|t11/C-1 Multicall3 aggregate3|P|batch de staticcalls en 1 eth_call|0 — YA PRESENTE en theta|grep propio 152 matches (t11-evidence/C1_multicall_grep.txt) + v3_quote_provider.rs:63 batch=100 + state_projector.rs:612 techo(N/b) + t4 ADM_G4 250|0|2026-10-13
ADM-27|t11/C-2 batch eth_call JSON-RPC / eth_callMany|P|N eth_calls en 1 request HTTP|NO COMPUTADO (x_ref sin observable RT/slot; sin productor TS — api-server no cotiza on-chain; I1 prohíbe cablear hot path) => wire 0 'sin seam productor'|t11-evidence/C2_jsonrpc_batch_grep.txt (0 matches) + sonda eth_callMany soportado en publicnode (error -32602 de tipos)|0|2026-10-13
ADM-28|t11/C-3 trace_callMany|R|N calls secuenciales-con-estado en 1 trace|0 — instrumento INACCESIBLE: HTTP 403 'Archive requests require a personal token' (stack soberano lo rehúsa; §38 prohíbe pagar)|t11-evidence/C3_trace_callMany_probe.txt|0|2026-10-13
ADM-29|t11/C-4 state override (eth_call stateDiff)|R|what-if sin mutar estado|mu=0 en eje declarado ↓RT (habilitador, no reductor; mismo 1 RT por call)|inspeccion de eje sobre x_ref.json coordenadas (patron t4 ADM-2/3, ADM-t11 §2)|0|2026-10-13
ADM-30|t11/C-5 split de routers|R|paralelizar cotizacion cross-router|mu=0 standalone (paralelismo cambia latencia, NO el conteo de RT; eje ↑x1 no computable: base viables=0)|x_ref.json x1_cobertura.viables=0|0|2026-10-13
ADM-31|t11/C-6 quoters staticcall|P|cotizar via view-only|0 — YA ES LINEA BASE (doctrina §38 expresa)|t11-evidence/C6_staticcall_grep.txt: amm_math.rs:6-7 QuoterV2 mainnet + quoteExactInputSingle:538|0|2026-10-13
ADM-32|t11/C-7 sim en fork (anvil externo)|R|sim pre-trade sobre fork|≈0 — equivalente YA en theta (revm in-process G-SIM-1) + instrumento local BLOQUEADO (anvil.exe AppControl ×2; cast/forge sí corren)|t11-evidence/C7a_revm_ya_presente.txt (Cargo.toml:39, execute_multistep_revm :75-76) + C7b_anvil_appcontrol.txt|0|2026-10-13
ADM-33|t12/ADM-5 D: Flashloan Aave v3 + Balancer + dYdX + UniV3 + callbacks|MIT (adaptadores propios)|n/a (pi EN theta)|mu incremental=0 (YA PRESENTE)|contracts/src/flashloans/{AaveV3,Balancer,DyDx,UniV3}FlashAdapter.sol + shared-rs/src/flashloan_math.rs:19-218 + flashloan_arb_worker.rs + trading_config.rs:766|0|2026-10-13
ADM-34|t12/ADM-5b D: Flashloan Morpho + Euler|P (ambos MIT/Apache, ERC-4626)|pi apunta a ↓fee-flashloan (NO x4; fee es knob de config)|NO COMPUTABLE (adaptador nuevo + contrato desplegado + fork-sim; I1 congela hot path y terminus) + AUSENTE (registry-engine.ts:546 lista solo aave/balancer/dydx/uniswap_v3)|grep + registry-engine.ts:546|0|2026-10-13
ADM-35|t12/ADM-6 D: Flash-mint|—|n/a|mu=0 (brazo YA PRESENTE estructuralmente; activación es producto de cartridge, no de 𝒯-D)|cartridge_boot.rs:489 ('flashmint_atomic' arms) + route_applicability.yaml:61-66|0|2026-10-13
ADM-36|t12/ADM-7 D: EIP-1153 transient storage|P (estándar EIP)|↓x4 (gas)|NO COMPUTABLE (delta de gas exige contrato nuevo desplegado + tx real/fork; PAPER ON ¬broadcast ∧ I1)|grep TSTORE|TLOAD|transient = 0 matches|0|2026-10-13
ADM-37|t12/ADM-8 D: Minimal proxies / deployment factory|—|n/a|mu=0 (YA PRESENTE)|contracts/src/core/DeterministicFactory.sol + sim-core/src/paper_executors.rs:347,385-386|0|2026-10-13
ADM-38|t12/ADM-9 D: ERC-4337 account abstraction|P (EntryPoint MIT)|↓tx/ruta teórico|NO COMPUTABLE (infra bundler + broadcast; fuera del terminus paper) + AUSENTE (grep real = 0; 43 matches previos eran sqrtPriceX96)|grep UserOperation|EntryPoint|4337 = 0|0|2026-10-13
ADM-39|t12/ADM-10 D: EIP-7702|P (crate Apache/MIT)|↓x4 (warm SLOADs)|NO COMPUTABLE doctrinal (delegación exige firma+broadcast ¬PAPER ON); DEPENDENCIA presente, CAMINO ausente|Cargo.lock:230 alloy-eip7702 + sequence_runner.rs:554 + readiness-extras.ts:335|0|2026-10-13
ADM-40|t12/ADM-11 D: Nonce manager|—|n/a|mu=0 (YA PRESENTE)|relays-client/src/nonce_manager.rs:1-82 + submit_engine.rs:1016-1038|0|2026-10-13
ADM-41|t12/ADM-12 D: Priority fee|—|n/a|mu=0 (YA PRESENTE)|relays-client/src/bundle_builder.rs:131-183 + submit_engine.rs:495 (WO-04)|0|2026-10-13
ADM-42|t12/ADM-13 E: Flashbots|—|n/a|mu=0 (YA PRESENTE)|relays-client/src/relay_flashbots.rs (auth signer.rs:38, callBundle :594) + sed-core flashbots_simulator.rs:8 ('NO signing')|0|2026-10-13
ADM-43|t12/ADM-14 E: mev-share|—|n/a|mu=0 (YA PRESENTE vía cartridge)|cartridge_wave_de_test.rs:2145 (mev_09_018, origen :972)|0|2026-10-13
ADM-44|t12/ADM-15 E: mev-boost|Apache-2.0|n/a (π=∅: middleware validador, no plano searcher)|mu=0 (N/A al universo del sistema: el searcher se integra a BUILDERS vía relays)|analisis de plano (t12-evidence/ADM_registry.md §3)|0|2026-10-13
ADM-45|t12/ADM-16 E: Jito|MIT|∅|mu=0 (N/A: bundle engine de Solana vs stack EVM)|grep jito = 0 matches|0|2026-10-13
ADM-46|t12/ADM-17 E: bloXroute|—|n/a|mu=0 (YA PRESENTE)|relays-client/src/relay_bloxroute.rs:52-128 + credentials/validators.ts:238|0|2026-10-13
ADM-47|t12/ADM-18 E: Backrun/sandwich (detección defensiva)|—|n/a|mu=0 (YA PRESENTE; sandwich OFENSIVO PROHIBIDO por doctrina — no candidata)|backrun_engine.rs:1 ('Zero Victim') + g-mev-1.ts:6 + cartridge_boot.rs:837|0|2026-10-13
ADM-48|t12/ADM-19 E: Solver auctions / intents|—|n/a|mu=0 (YA PRESENTE, shadow_only)|strategy_applicability.rs:1009 (intents_solver_engine enabled shadow_only)|0|2026-10-13
ADM-49|t12/ADM-20 E: Private RPC (MEV-Blocker/Protect)|—|n/a|mu=0 (YA PRESENTE vía relay stack; sub-gap declarado: sin cliente submit dedicado mev-blocker)|relay_no_submit_sim.rs:15,116,259 + chain_client.rs:455 + submit_engine.rs:1491|0|2026-10-13
ADM-50|t12/ADM-21 E: Decode pending tx|—|n/a|mu=0 (YA PRESENTE)|mempool_listener.rs:33-44 + scanner.rs:1665 + route_decoder.rs:11 + impact_index.rs:550-563|0|2026-10-13
ADM-51|t12/ADM-22 E: State diffs / post-state|—|n/a|mu=0 (YA PRESENTE como post-state projection; sub-gap: state diff reth literal no)|state_projector.rs + dirty_signal.rs + strategy_applicability.rs:429|0|2026-10-13
ADM-52|t13/ADM-1 H: sizing AMM forma cerrada (x·y=k) + cota Kelly (A14/A15)|P|control-plane puro (adm/econ_sizing.py, I1: 0 bytes hot path)|mu=17915.4383 USD >= delta=358.3088 (x6: 17/17 items, calibracion exacta net(a0)==net0 ±1e-6)|python docs/integration-v4/t13-evidence/t13_admission_eval.py -> T13_admission_eval_output.json; V1..V4 releases/R-20261013T13HI-t13 (V3 unittest 10/10 OK)|1|2026-10-13
ADM-53|t13/ADM-2 H: break-even gas↔profit|P|control-plane|mu=0.0 (0 flips/26 con contabilidad honesta)|idem script T13_admission_eval_output.json|0|2026-10-13
ADM-54|t13/ADM-3 I: staleness oraculo (gate cuarentena)|P|control-plane puro (adm/staleness_gate.py)|mu=0.0769 (2/26 con T=2405ms=p95 medido) >= delta=0.02|idem script + adm/staleness_gate.py; V1..V4 ✓|1|2026-10-13
ADM-55|t13/ADM-4 I: ventanas no atomicas (gate de clase)|P|control-plane puro (adm/nonatomic_gate.py)|mu=0.113636 (30000/264000 NONATOMIC_BRIDGE_REQUIRED en A8) >= delta=0.02|idem script + adm/nonatomic_gate.py; V1..V4 ✓|1|2026-10-13
ADM-56|t13/ADM-5 I: reorg (confirmaciones)|P|—|NO COMPUTABLE — semantica ILEGIBLE del campo (valores 1..2530 tipo block-height)|idem script (campo sin senal medible)|0|2026-10-13
ADM-57|t13/ADM-6 H: funding/basis (A18), paridad opciones (A19), spreads LST (F07)|R|—|NO COMPUTABLE — sin datos de funding/opciones/LST en sustratos A/B/C|idem script|0|2026-10-13
ADM-58|t13/ADM-7 H: optimizacion convexa multileg|P|—|NO COMPUTABLE — requiere liquidez por tick (ausente); parte V2 ya cubierta por ADM-52 (I2)|idem script|0|2026-10-13
ADM-59|t13/ADM-8 I: leakage / seleccion adversa / cascadas liquidacion|P|—|NO COMPUTABLE — sin mempool/positions lending en sustratos|idem script|0|2026-10-13
```

## Conteo por track (veredicto 1 / veredicto 0)

| track | filas | veredicto 1 | veredicto 0 |
|---|---|---|---|
| t4 edge/SSE (G+C) | 4 | 1 (SSE prices/stream) | 3 |
| t5 frontend/bus (G) | 7 | 4 (t_bus, t_seq, t_watchdog, t_lifecycle — mecanismo) | 3 (wire sin productor) |
| t10 𝒯-A/B | 14 | 1 (promoción detector_policy.rs, enmienda α11) | 13 |
| t11 𝒯-C | 7 | 0 | 7 |
| t12 𝒯-D/E | 19 | 0 | 19 |
| t13 𝒯-H/I | 8 | 3 (sizing+Kelly, staleness, no-atómicas) | 5 |
| **TOTAL** | **59** | **9** | **50** |

## GATE |A7| > 0

```
$ grep -c '^ADM-' docs/integration-v4/REGISTRO_INCLUSIONES.md   (bloque §13)
59          ⇒ |A7| = 59 > 0   GATE: PASS
veredicto_1 = 9 · veredicto_0 = 50 (veredictos 0 INCLUIDOS, §13)
```
(re-ejecución post-swap del gate sobre el archivo final: ver §V7 más abajo — salida literal pegada)

## Coherencia ∃ADM-n ⟺ ∃ℳ(μ)

- 59/59 filas citan comando+salida o artefacto ℳ congelado (grep/Select-String con matches, scripts con output JSON, bench con salida literal, x_ref.json con sha). Cero μ rellenados con proxy; "NO COMPUTABLE/NO COMPUTADO" declarado como tal (13 filas: ADM-14,15,16,17,18,20,21,23,24,25,27,34,36,38,39,56,57,58,59 — motivo por fila en su celda μ).
- Filas cuyo ℳ es inspección de artefacto congelado (π=∅ ⇒ μ=0 por definición, patrón ratificado t4/t11): ADM-2, ADM-3, ADM-29, ADM-30, ADM-44 — el artefacto es `x_ref.json` (sha a6d521ea…, ℛ.tsv:7). Ninguna fila quedó sin ℳ: 0 eliminadas, 0 degradadas a CLAIM suelto.
- Falsador α7 (t17): 12/12 ataques NO_REFUTADO (FALSATION_REPORT.md); F11 demuestra que el gate default-deny excluye μ sin evidencia — este registro fue compilado bajo esa disciplina.

## Parada §6 — 𝒜(θ)=∅ en los seis tracks

Cada registro fuente declara su parada: t4 (4/4 candidatos), t5 (4/4 implementados + wire terminal), t10 ("|A7_admitidos_§12|=0 — ningún t* con μ≥δ demostrable"), t11 ("𝒜(θ)=∅, 7/7 veredictos"), t12 ("Admitidos: 0… veredicto mecánico honesto"), t13 (8/8 evaluados). Existe K (=cierre de cada track, 2026-10-13): ∀k>K 𝒜(θ_k)=∅ — nada admisible sin procesar. Veredictos 0 cuentan como procesados y NO se re-evalúan (§5 invariante de exclusión).

## V1..V4 de los 9 admitidos (verificación t14)

| ADM | V1 (hashes) | V2 (estático) | V3 (build/test) | V4 (dry-run/deploy) |
|---|---|---|---|---|
| ADM-1 (t4 SSE) | ✓ measure_mu_output.json sha C2280DB3… + cadena seam §5 (D0193916…) | △ no persistido como archivo por t4 — cubierto por ADM-t5 §5-§7 (mapa+hashes) | ✓ vía batería t5 §7 (tsc api-server 0 + 31/31) y build limpio t10 (1431P/3F baseline exonerado) | △ wire NO desplegado (F5/F6 prohibidos RG-01); staging local |
| ADM-5..8 (t5 ×4) | ✓ ADM-t5 §7 (11 sha256) | ✓ §5 mapa de archivos α11 | ✓ tsc 3× EXIT=0 + vitest 31/31 · 1418/1418 · 46/46 (literal §7) | △ V4 wire N/A: 'no wire producer' (ADM-9..11), staging local P1 |
| ADM-12 (t10 promoción) | ✓ ℛ.tsv:29 (promoted 5698976f) | ✓ fila ℛ + git status ' M detector_policy.rs' | ✓ t10-evidence/V3 cargo build exit=0, test 1431P/3F/5I (3F==baseline) | ✓ V4 staging local (P1: sin commit/push) |
| ADM-52/54/55 (t13 ×3) | ✓ V1_hashes.txt (12 sha256 + ℛ) | ✓ V2_static.txt (py_compile exit=0; node cuelga — declarado) | ✓ V3_tests.txt unittest 10/10 OK | ✓ V4_dryrun.txt DEGRADADO declarado (rsync ausente; nada desplegado — igual que t9/t11) |

**Gap declarado y escalado**: t4 no persistió V2/V4 formales como archivos (su salida de tarea no quedó registrada en el tablero). La cobertura existe vía t5 §7 + t10 clean-build, pero t6/reviewer debe cargar esta asimetría. NO se suaviza.

## Invariantes y política

- **P1**: A7 + A8-final viajan al staging como archivos del release (docs/integration-v4/); rsync/scp AUSENTES en host (mismo hallazgo que t9/t11/t13 — declarado, no rellenado); **0 commits/push/pull como transporte** (A7 se deja como artefacto de staging NO commiteado en esta tarea — t6 decide su entrada a A1/release final).
- **P5**: PAPER ON intacto, detección/searcher P&L intactos, 0 secretos tocados, 0 ediciones de hot path por t14.
- **§D**: sin fallas de instrumento en la consolidación (todas las fuentes se leyeron completas; 0 timeouts).

## V7 — gate re-verificado post-swap (salida literal)

```
(post escritura de A7 + hoja REGISTRO_INCLUSIONES en A8-final)
grep -c '^ADM-' docs/integration-v4/REGISTRO_INCLUSIONES.md -> 59  (|A7| = 59 > 0: PASS)
hojas A8-final: 60 (59 fijas + REGISTRO_INCLUSIONES)  => invariante |hojas|>=60: CERRADA
```

> **Regla de hash §11-bis**: sha256(ESTE archivo) y sha256(A8-final) deben quedar registrados dentro de A1 al regenerarlo (t6). Ver ℛ.tsv filas t14.
