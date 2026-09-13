# BR-00 — RE-VERIFY adversarial post-apply (READ-ONLY)

> **WO:** BR-00-REVERIFY · **kind:** verify adversarial · **agente:** ecc:security-reviewer
> (Gang Omniscience, mesa CEREBRO) · **Fecha:** 2026-09-08.
> **Presunción de partida:** diff culpable hasta evidencia contraria. TODO re-ejecutado
> por mí (aserciones del builder ≠ facts). Árbol `feat/hops-live-01` @ 27aca289 + working
> tree. **READ-ONLY:** 0 git write, 0 edits, 0 VPS (0 SSH usados — no hay era post-fix
> que medir: el fix NO está desplegado), 0/5 requests HTTP dominio público.

---

## 0. VEREDICTO: **PASS-with-advisories**

El gate `!= dex_arb` fue eliminado de verdad; la admisión es estructural y fail-closed;
los stems fluyen sin mutación con la kind EXACTA en cada reason; el clasificador no
fabrica viabilidad; el fix del decoder V2 es correcto y además cierra un borde de
PASSED mendigo pre-existente. Batería re-ejecutada: verde con los MISMOS números
declarados. 4 advisories (ninguna BLOCKED) al final.

---

## 1. Tabla PASS/FAIL por punto del protocolo

| # | Punto | Veredicto | Evidencia (re-ejecutada por mí) |
|---|---|---|---|
| 1a | Diff = EXACTAMENTE lo declarado en sim-ctl | **PASS** | `git diff --stat backend/sim-ctl/`: tx_builder.rs (+126/-…), sim_engine.rs (110), persistence.rs (24) — solo 3 archivos, 231 insertions/29 deletions, 377 líneas de diff total |
| 1b | §34.3: 0 diffs en `backend/relays-client/` | **PASS** | `git status backend/relays-client/` = vacío |
| 1c | 0 diffs de BR-00 en searcher-rs/{lib,main,runtime_knobs}.rs, frontend/, compose.prod.yml | **PASS*** | Ver §4-ADV1: el árbol SÍ está sucio ahí, pero con 0 marcadores BR-00 y 0 contenido de simulación (`git diff -- frontend/ edge/ backend/api-server/ \| grep -iE "simulat\|strategy_kind\|not_simulatable"` = 0 líneas; `git status docker/` = vacío). Atribución = WOs paralelos (control-board, CB-02), NO BR-00 |
| 2 | Marcadores `// BR-00 (2026-09-07)` en cada hunk | **PASS** | grep: **17 ocurrencias en exactamente 3 archivos** (tx_builder 9, sim_engine 5, persistence 3). Todos los hunks productivos y los tests llevan marcador |
| 3a | `cargo fmt --check -p sim-ctl` | **PASS** | exit 0, output vacío |
| 3b | `cargo check -p sim-ctl` | **PASS** | Finished dev profile, **0 errores** (7m44s en mi corrida — target frío por diffs paralelos de CB-02 en shared deps; builder reportó 43s con target caliente) |
| 3c | `cargo clippy -p sim-ctl -- -D warnings` | **PASS** | Finished, exit 0, **0 warnings** |
| 3d | `cargo test -p sim-ctl` | **PASS** | **66 passed, 0 failed, 2 ignored** — lib 5/0, main 44+1 ignored, pel_recovery 1 (40.26s), route_aware 8+1 ignored, redelivery 8/0. Idéntico a lo declarado. Los 2 tests drain-guard del charter presentes (lib.rs:67, :116) |
| 4a | ¿Stem colándose simulando lo que no es? | **PASS** | El probe es un swap REAL encodeado (selector V2 0x38ed1739 / V3 0x414bf389, verificado en tests); `passed` exige eth_call + estimate_gas + decode + umbral (sim_engine.rs:98-127). Un payload no-swap con tokens distintos muere en catálogo de routers (`UnknownRouter`) o en revert del fork. Semántica "la sim mide el PROBE" DECLARADA por el builder (§1.1/§6.1) — no es un defecto oculto |
| 4b | Multihop real fail-closed | **PASS** | Para multihop admitido (ej. `mev_01_019_multi_hop_arbitrage`), el probe mide el hop DIRECTO token_in→token_out en el router de dex_a (tx_builder.rs:96-109): si no existe pool directo, `eth_call` revierte en el fork → `reverted:*`, passed=false. Revert real, no passed mendigo |
| 4c | ¿Sufijo `:<kind>` rompe consumidor downstream? | **PASS** | grep repo-wide `strategy_not_simulatable\|strategy_cyclic_route_not_simulatable\|output_undecodable` fuera de sim-ctl: solo docs históricos (spec 2026-04-21, LEARNINGS.md). Consumidores de `fail_reason` en repo: passthrough/display (wallet-sim-runtime.ts:119, OpportunitiesClient.tsx:184, SimulateButton.tsx:88, GSimSmokeTestCard.tsx:174, smoke-test-sepolia.sh:86) — **0 matching exacto** del label. El label viejo desnudo `"strategy_not_simulatable_in_s4"` tiene **0 sitios de emisión** (grep literal = 0 matches en src/) |
| 4d | ¿Clasificador convierte gap en viable mendigo? | **PASS** | persistence.rs:79-86: gap ⇒ commit de la fila de sim y NO update de la opp (status queda detected/validated, rejection_reason NULL). `next_status='simulated'` SOLO con `r.passed` (:88). Terminus: `XADD arbx:opps:simulated` solo `if sim.passed && inserted_fresh` (consumer.rs:459). Gap nunca emite al terminus. RULE 00 preservada |
| 5 | Decoder V2 array-primero correcto | **PASS** | Retorno V2 `swapExactTokensForTokens` = `uint[]` (ABI dinámico): word-0 = offset 0x20=32 → decode `[Uint(256)]`-primero lee 32 constante (bug confirmado leyendo el código viejo en el diff: match "dex_arb" → uint256 primero). Array-primero toma `.last()` = amountOut real. Ambigüedad inversa despreciable: un uint256 V3 solo se mallee como array si su valor == 32 Y hay words extra formando un array válido — los returns de eth_call son exactamente 32 bytes para exactInputSingle. **Borde extra cerrado que el builder no reclamó:** con amount_in ≤ 32 wei, el bug viejo daba pct=0.0 → `passed=TRUE` fabricado (compute_slippage:225-230, `actual >= amt_in` → diff 0); el fix elimina también ese PASSED mendigo |
| 6 | Protocolo de métrica §1.4 / query §4 | **PASS*** | Era-separada: `simulated_at >= TS_DEPLOY` (nunca promediar) ✓; conteo absoluto por kind: query 2 agrupa por fail_reason exacto que EMBEBE la kind ✓; BR-04-no-corrido declarado como invariante (3) ✓; XLEN terminus como prueba de vida (4) ✓. NO corrí nada contra PG: no existe era post-fix (0 deploys), 0 SSH usados. Ver §4-ADV2/ADV3 |
| 7 | §34.1 mode-invariance | **PASS** | grep del diff por `ARBX_TRADE_MODE\|PAPER_SHADOW\|LIVE_MAINNET\|SIM_BACKEND\|TESTNET` = **0 matches**. El gate decide por payload, igual en todos los modos |
| 8 | Stem-stomping: 0 call-sites sobre opps de cartridge | **PASS** | Grep replicado `to_contract_strategy_kind(`: 27 hits en backend/. Los ÚNICOS de forma mutación `opp.strategy_kind = ...`: flashloan_engine.rs:326, :361 (clone_for_rejection / build_wrapped_candidate sobre StrategyCandidate NATIVOS, con `base_strategy=Some(base_label)` para observabilidad). El resto son struct-literals (construcción de opps NUEVAS) o variables locales. El path de cartridge construye `StrategyKind::cartridge(cartridge_id)` (cartridge_boot.rs:1156) y NO pasa por ningún engine re-etiquetador. El diff BR-00 solo LEE `as_str()` |

### Claims estructurales del builder verificados contra código emisor (línea a línea)

| Claim | Evidencia |
|---|---|
| triangular SIEMPRE token_in==token_out | triangular_worker.rs:1520 (`StrategyKind::triangular()`), :1529-1530 (`token_in: addr_a.clone(), token_out: addr_a.clone()`) ✓ |
| flashloan_arb SIEMPRE cíclico | flashloan_arb_worker.rs:1081, :1085-1086 (addr_a en ambos) ✓ |
| liquidation dex_a="aave-v3:\<pool\>" | liquidation_worker.rs:1064-1065 (`StrategyKind::liquidation()`, `dex_a: format!("aave-v3:{}", …)`) ✓ — rechazo por kind da reason específico en vez de router-not-in-catalog engañoso |
| cartridge payload = hop (token_in primer leg / token_out último leg / dex_a dex_hint) | cartridge_boot.rs:1136-1159 ✓ (dex_a puede ser "unknown" → UnknownRouter honesto) |
| `cartridge(stem).as_str()` = stem EXACTO | shared-rs/contracts.rs:35-41 (newtype String, round-trip verbatim) ✓ — el sufijo del reason lleva el stem sin colapso |
| fail_reason TEXT soporta sufijos | database/migrations/004_simulations.sql:14 `fail_reason TEXT` ✓ |
| Bug decoder pre-existente y gated a dex_arb | Diff muestra el código viejo removido: `match opp.strategy_kind.as_str() { "dex_arb" => [Uint(256)] PRIMERO … }` ✓ |

### Greps RULE 00 sobre el diff (replicados)

- `+` líneas con `passed:true`/`passed = true`/mock/stub/fake: **0**.
- Strings de modo: **0** (ver punto 7).
- Nuevos `SimulationResult` fuera de las vías existentes (`not_implemented`/`failed`/anvil): los reason-nuevos van por `Self::not_implemented` (passed=false SIEMPRE, sim_engine.rs:154-168).

---

## 2. Análisis adversarial que NO encontró defecto (documentado para la mesa)

1. **¿La división en 2 familias maquilla la métrica?** El gate del charter (share familia
   `strategy_not_simulatable_in_s4` < 50%) es débil como criterio aislado: los cyclics
   migran a familia nueva y los admitidos a reverted/build_error reales, así que el share
   de la familia (ii) baja casi por construcción. PERO: ninguna familia está oculta
   (query 1 las muestra todas con conteo), el mix-shift está declarado (APPLY §1.3), y el
   protocolo 1.4 exige conteos absolutos + comparación contra baseline §1.1 + XLEN.
   No es maquillaje — es medición honesta con etiquetas más finas. La carga de prueba
   queda en APLICAR el protocolo completo (ADV3).
2. **`output_undecodable` como gap en vez de `slippage_too_high` reject:** pre-fix, un
   output V2 no decodificable se rechazaba con UNA MEDIDA FABRICADA (amountOut=32 →
   pct≈100). Post-fix: None honesto → gap (opp no rechazada, no viable, no XADD).
   Dirección correcta bajo R8 (ausencia de medida ≠ veredicto de mercado). El shift en
   dashboards de "rejected" es honestidad restaurada, declarada en APPLY §0/§2.2.
3. **Observación pre-existente FUERA del diff (no imputable a BR-00):**
   `revert_risk_pct: Some(0.5/50.0)` son constantes heurísticas en el camino passed/failed
   (sim_engine.rs:135) — ya existían; que la mesa las agenda para S5 si quiere rigor.

---

## 3. Números REALES re-ejecutados (los del builder quedaron confirmados)

| Comando | Resultado mío | Declarado | Match |
|---|---|---|---|
| `cargo fmt --check -p sim-ctl` | exit 0, sin output | limpio | ✓ |
| `cargo check -p sim-ctl` | Finished, 0 errores (7m44s) | 0 errores (43.42s) | ✓ (delta de tiempo = target frío, no de resultado) |
| `cargo clippy -p sim-ctl -- -D warnings` | Finished 28.23s, exit 0 | 0 warnings | ✓ |
| `cargo test -p sim-ctl` | 5+44+1+8+8 = **66 passed, 0 failed, 2 ignored** | 66/0/2 | ✓ exacto |
| Tests nuevos BR-00 | 8 presentes: non_swap_kinds_rejected, cyclic_routes_rejected_with_typed_error, cartridge_stems_and_relabels_build_probes, unknown_open_route_kind_builds, decode_v3_uint256_shape, decode_v2_array_shape_takes_last, decode_garbage_returns_none, br00_structural_gap_families_are_gaps | +8 | ✓ |
| Tests drain-guard charter | lib.rs:67 + lib.rs:116, verdes | exigidos | ✓ |

Nota de entorno: Windows AppControl NO bloqueó los exes de test en esta corrida
(tampoco en la del builder). Batería corrida desde `backend/` (workspace root) con
toolchain pinneada 1.91.0.

---

## 4. Advisories (0 BLOCKED — correcciones/gestiones para la mesa)

- **ADV1 (redacción, no código):** APPLY §3 dice "Archivos en claim de pares: 0 diffs".
  Literalmente FALSO — el árbol tiene diffs en searcher-rs (lib, main, canonical_knobs,
  pool_discovery, reserves, route_scanner_worker), api-server, edge, frontend. Verifiqué
  que NINGUNO lleva marcadores BR-00 ni contenido de simulación (grep = 0): son WOs
  paralelos (control-board, CB-02) en árbol compartido. Sustancialmente cierto,
  literalmente impreciso. **Consecuencia operativa:** el PR de BR-00 debe incluir SOLO
  los 3 archivos de sim-ctl (+ docs) — mezclar los diffs ajenos violaría P-∅ (§37).
- **ADV2 (métrica):** las queries del §4 miden las familias gap y el mix general, pero
  para PROBAR "los stems abiertos ahora simulan" falta un join por kind del lado de
  cobertura: `SELECT o.strategy_kind, s.simulator, count(*) FROM simulations s JOIN
  opportunities o ON o.id=s.opportunity_id WHERE s.simulated_at >= TS_DEPLOY AND
  s.simulator='anvil' GROUP BY 1,2` — conteo absoluto de kinds que llegaron al probe
  real (vs baseline 1/40). Añadirla al protocolo del post-deploy.
- **ADV3 (gate de éxito):** evaluar el <50% SOLO sobre la familia (ii) es débil; exigir
  además: (a) familia cyclic con conteo absoluto estable vs su flujo de entrada,
  (b) cobertura por kind ≥ varias kinds con simulator='anvil' (query ADV2), (c)
  XLEN `arbx:opps:simulated` > 0 solo cuenta si passed>0 real, (d) BR-04 no corrido
  (invariante ya declarada). El protocolo 1.4 lo permite — que el operador lo exija.
- **ADV4 (semántica residual, ya declarada por el builder):** las sims/XADD de stems
  admitidos certifican el PROBE de hop directo, no la topología multihop real del
  cartridge. S5 (counter-trade + quoter) sigue siendo el camino a fidelidad completa.
  Downstream no debe leer probe-pass como certificación del path del cartridge.

---

## 5. Cierre

El P0 D-SIM-01 está cerrado a nivel de código: decisión estructural, stems intactos,
reasons per-kind, fail-closed en todos los bordes que pude atacar, batería verde con
números reproducidos, §34.3 intacto, mode-invariante, y de bonus un bug real de decoder
(que incluía un borde de passed fabricado) corregido con test que lo pescó. El apply
cumple el charter de re-verify (marcadores + greps §2.2 + test verde + protocolo 1.4
declarado). **PASS-with-advisories** — el BLOCK del verify anterior queda levantado;
el paso a BR-02 ya no está gated por BR-00. La métrica de éxito se mide SOLO post-deploy
veraz del operador, con el protocolo completo (ADV2/ADV3), era-separada.

**Firma de honestidad (RULE 00):** cada cifra de este reporte proviene de un comando que
YO ejecuté en esta sesión (diffs, greps, fmt/check/clippy/test), o de línea de código
citada con archivo:línea. 0 SSH, 0 requests HTTP, 0 escrituras fuera de este documento.
