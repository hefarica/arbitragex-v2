# WO-02d-verify · VERIFICACIÓN de 02d-RELAYS-CLIENT-SHARED.md — PASS

> kind: verify · rol PhD ecc:security-reviewer (Gang Omniscience) · 2026-09-17.
> Objetivo: auditar `02d-RELAYS-CLIENT-SHARED.md` + `WO-02d-DESIGN.md` con lente
> de seguridad read-only. CERO edición de código (default-deny y MainnetRefused
> INTOCABLES — nada propuesto los toca), cero cargo/git/VPS-mutación.
> Lexicon OMEGA · RULE 00 · fail-honest R8.

## 0. Sincronía de mesa redonda (estado al inicio)

- Board `GOAL-WORKORDERS.md` leído completo (67 l).
- Pares disponibles en el dir: `WO-02a-DESIGN.md` (leído completo — único par
  02x publicado), `02-VPS-REMAP-20260917.md` (referencia SHA VPS=a06a968d).
- **02b y 02c: AUSENTES** al momento de verificar (FAIL-HONEST). El cross-check
  de tipos contra 02c queda abierto; contra 02a se hace en §3.
- Adendum `## VERIFICACIÓN` agregado a `02d-RELAYS-CLIENT-SHARED.md` (única
  edición de este WO, junto con este archivo).

## 1. Item 1 del charter — default-deny y MainnetRefused re-abiertos

`backend/relays-client/src/live_exec_policy.rs` (97 líneas, leído COMPLETO):

| Cita de la ficha | Verificación | Resultado |
|---|---|---|
| `:3` `DEFAULT_LIVE_CHAINS = &[11_155_111]` | línea 3 exacta | ✅ |
| `:37` `enabled: enabled == Some("true")` | línea 37 exacta — solo el string exacto habilita | ✅ |
| `:41-52` `assert_broadcast_allowed` | exacto: `NotEnabled` primero, `ChainNotAllowed` después | ✅ |
| `:6-11` enum `LiveExecDenied { NotEnabled, ChainNotAllowed }` — sin `MainnetRefused` | exacto (enum :6, variantes :7-10) | ✅ |
| test `explicit_mainnet_is_supported` `:71-76` | exacto (`Some("true"), Some("1,11155111")` → mainnet OK) | ✅ |
| test `invalid_allowlist_never_partially_activates` `:78-84` | exacto — parse `Option<Vec<_>>` → vacío ante cualquier token malformado; NUNCA expande | ✅ |
| test `exact_true_only` `:86-89` | exacto (`"TRUE"`, `"1"`, `"true "` quedan OFF) | ✅ |
| test `enabled_default_is_sepolia` `:92-96` | exacto (enabled sin chains → mainnet `is_err()`) | ✅ |

Enforcement dual re-abierto:
- **Boot**: `main.rs:172-188` — exacto; `anyhow::bail!` en :184-186 si `live_mode`
  y la policy falla. Antecedido por SECURE_BOOT A2 (`main.rs:130-162`, bail :147-156:
  paper off exige `ARBX_SIMULATOR_V2_READY == "true"`).
- **Pre-firma**: `bundle_builder.rs:57-59` — exacto, ES la primera statement de
  `build_and_sign` (post-firma de la función, pre-todo-lo-demás). Gates siguientes
  en orden verificados: estrategia :60-62, signer chain/addr :63-68, provider chain
  :69-75, resolve ejecutor TLS :76-77, binding :78-89, cap principal :90-107
  (`ARBX_LIVE_PRINCIPAL_CAP_<chain>_<token_in-hex>`, U256 exacto), head fresco
  :108-118, offset==1 :119-121.
- Tests de policy en `bundle_builder.rs:414-436` (`m1_rejects_before_fle_resolution`;
  los 3 asserts citados como :416-434): exactos — default-deny, chain fuera de
  allowlist, mainnet explícito permitido, todo ANTES de resolver `FLASHLOAN_EXECUTOR_*`.

`MainnetRefused` — grep backend (completo, sin head-limit):
```
api-server/src/routes/control-board.ts:94,173,742
api-server/src/services/control-board-drift.ts:90
```
= EXACTAMENTE las 4 líneas TS que la ficha cita, todas comentarios. Drift
doctrina↔código CONFIRMADO. **Corrección 2**: fuera de backend también aparece en
`.claude/skills/arbitragex-live-engineering/references/biblioteca/{22-golive-playbook.md:44,
20-system-integration-patterns.md:278, 14-onchain-execution-contracts.md:430}` — canon
de skills que el operador lee; la remediación documental D1 debería contemplar la
biblioteca además de api-server (P-∅: un PR = un ID — elegir UNA superficie).

**Dictamen item 1: ficha CORRECTA.** El cuadro §1 de la ficha ("verdad parcial"
de "physically refuses mainnet": allowlist explícito, no blacklist de chain 1) es
la lectura exacta del código.

## 2. Item 2 del charter — barrido §32/§33 (superficies prohibidas)

¿Documenta la ficha algún path donde PRIVATE_KEY o firma queden alcanzables fuera
de relays-client? **NO — y verifiqué que no existe en backend:**

- Grep `send_transaction|send_raw_transaction|eth_sendBundle|send_bundle` en
  backend: TODOS los hits de código real en relays-client (`multi_relay.rs:60,138`,
  `relay_flashbots.rs:91,315`, `relay_bloxroute.rs:72`, `relay_titan.rs:63`). Fuera:
  - `sed-core/src/connectors/mod.rs:20-22` — invariante negativa DOCUMENTADA:
    "No `eth_sendBundle`, `eth_sendRawTransaction`, or `send_transaction`";
    Flashbots = `eth_callBundle` (simulación) only.
  - `sed-core/src/connectors/flashbots_simulator.rs:41,90` — "Verify no
    signer/private_key exists"; sin `.sign()`/`.send_bundle()`.
  - `searcher-rs/src/workers/execution_worker.rs:35` — comentario jerga en stub
    NUNCA spawneado (02a §4.5 ya lo clasificó MUERTO; consistente).
- **OMEGA SEAL (hallazgo positivo de la verificación, no de la ficha)**:
  `searcher-rs/src/main.rs:303-343` — panic en boot si CUALQUIERA de 8 keys de
  capital está poblada en env (`FLASHBOTS_SIGNER_KEY`, `EXECUTOR_PRIVATE_KEY`,
  `ARBX_EXECUTOR_PRIVATE_KEY`, `ARBX_SIGNER_PRIVATE_KEY`, `ARBX_TESTNET_PRIVATE_KEY`,
  `SIM_SIGNER_PRIVATE_KEY`, `PRIVATE_KEY`, `MNEMONIC`). searcher-rs es físicamente
  incapaz de sostener una key — el terminus como ÚNICO firmante está reforzado
  upstream, no solo declarado.
- api-server (`readiness-steps.ts:116-119,193`): solo REPORTA presencia de keys
  (redacted "present"), jamás las carga.
- `LocalWallet` fuera de relays-client: solo en tests de relays-client
  (`multi_relay.rs:290`, `relay_flashbots.rs:625` — claves efímeras de staging).
- `signer.rs:56-63`: Debug redactado (`finish_non_exhaustive`) — exacto, la clave
  cruda es imprimible por accidente en NINGÚN path.

**Dictamen item 2: PASS.** Ninguna referencia a executor/wallet/broadcast operable
fuera del terminus; la ficha no documenta (ni existía) ninguna superficie fuga.

## 3. Item 3 del charter — consistencia de tipos canónicos (grafo inverso cruzado)

- `Opportunity` definido `shared-rs/src/contracts.rs:44-96` — verificado: campos
  citados exactos (`net_expected_profit_usd` :74-75 con doc gross≠net :56-73,
  `rejection_reason` :85-86, `cartridge_id` :92-93, doc "Do NOT use as net" :61).
- Stream `arbx:opps:simulated`: productor `sim-ctl/src/consumer.rs:49`
  (`STREAM_OUT`) — EXACTO.
- Key `arbx:validated_plan:` — grep backend COMPLETO (10 hits):
  - Productores: `sim-ctl/src/canonical_plan_consumer.rs:4,41` (TTL 300 s) +
    `searcher-rs/src/scanner.rs:2449` + `searcher-rs/src/candidate_simulation.rs:582`
    = exactamente los 3 listados en ficha §4. **Sin productores no listados.**
  - Consumidor: `relays-client/src/submit_engine.rs:161,445` (fail-closed, ficha
    :442-484). El propio comentario de `canonical_plan_consumer.rs:39-40`
    documenta el triángulo scanner→carrier→relays-client: corroboración
    independiente del grafo de la ficha.
- **Cross-check 02a**: sustancia CONSISTENTE (una sola ruta de emisión de
  Opportunity desde cartucho; terminus consume el tipo canónico de contracts.rs).
  **Corrección 1 (cita)**: ficha §4 atribuye a 02a "`cartridge/runner.rs:290` como
  única ruta que emite Opportunity desde cartucho". 02a §6.1/§6.3 dice que la ruta
  de emisión única es `cartridge_boot.rs:964` (`active_evaluate_and_emit`);
  `runner.rs:290` es `evaluate` (el método Rhai). Puntero equivocado, conclusión
  intacta.
- **02c: AUSENTE** — el cross-check con la mitad sim/selector queda declarado
  ABIERTO para el próximo verificador (la ficha ya lo marcaba como GAP #3).
- Spot-checks shared-rs adicionales, todos EXACTOS: `config.rs:147-149`
  (default_paper_mode→true), `:156-158` (max_value 1.0), `:165-168`
  (priority_fee 2.0, marcador WO-04 2026-09-06 presente), `paper_mode.rs:66-74`
  (Default enabled=true "Safe default"), `trading_config.rs:183`
  (`capital_usd: f64`), `chains.rs:699-714` (resolve ejecutor TLS fail-closed
  Missing/Invalid/Zero), `consumer_spawn.rs:29-39,41-47` (spawn = db∧rpc∧(signer∨
  paper); `/execute` sin signer solo en paper), `submit_engine.rs:92-112` (R-0001
  primera statement, mode-invariant), `execution_admission.rs:185-188` (three-gas)
  y `:193-201` (capital_usd/50), `relay_no_submit_sim.rs:292`
  (`validate_and_discard`).

## 4. Item 4 del charter — RULE 00 en objetivo_usd

- Grep negativo de `350|5 WETH|canary` en `relays-client/**` y `shared-rs`:
  CERO matches canary (matches WETH = constantes de dirección `chains.rs:81-224`;
  `2500.0` en `price_oracle.rs:352-470` = fixtures de TEST de precios, no canary).
- La ficha distingue correctamente: canary §34.5 = DOCTRINA (vive solo en
  CLAUDE.md); fuentes runtime REALES = `trading_config.capital_usd` (PG),
  `max_value_eth` default 1.0 + `ARBX_LIVE_PRINCIPAL_CAP_*`, `min_profit_usd`,
  three-gas — todas verificadas en §3. **PASS.** Ningún blanco USD inventado.
- Nota de coherencia con 02a §9: 02a reporta "GAP objetivo_usd, solo bps
  relativos" en searcher; 02d reporta `capital_usd` runtime en el terminus. NO se
  contradicen: el searcher (mitad detección) no consume el blanco USD; el terminus
  (mitad ejecución) sí (cap 2 %). Dos mitades, dos fuentes distintas, reporte
  honesto de ambas.

## 5. Tally

| Dimensión | Cuenta |
|---|---|
| Citas file:line re-abiertas y confirmadas | 36 (lista en §1-§3) |
| Citas corregidas | 2 (Corrección 1: runner.rs:290→cartridge_boot.rs:964; Corrección 2: drift también en biblioteca skills) |
| Citas erróneas con impacto de seguridad | 0 |
| Greps dirigidos | 6 (MainnetRefused backend + repo, firma/keys, broadcast, validated_plan, canary ×2 globs, WETH/shared-rs) |
| Archivos Rust leídos (completos o por rangos) | 14 |
| Tests verificados por lectura (no ejecutados — restricción board) | 8 (5 live_exec_policy + 1 bundle_builder m1 + consumer_spawn tests presente :49+; canary negativo) |
| Ediciones a código | 0 |
| Ediciones .md | 2 (adendum a 02d-RELAYS-CLIENT-SHARED.md + este archivo) |
| Hallazgos que sugieran tocar el default-deny | 0 |

## 6. Dictamen final

**PASS.** La ficha 02d-RELAYS-CLIENT-SHARED.md es fiel al código en todas las
afirmaciones de seguridad del terminus: el default-deny y la materialización real
del rechazo mainnet (allowlist-default-Sepolia, no variante `MainnetRefused`)
están correctamente documentados con file:line exactos; no existe superficie de
firma/broadcast fuera de relays-client; el grafo inverso de tipos canónicos está
completo; objetivo_usd distingue doctrina de runtime. Las 2 correcciones son de
cita/completitud y ya quedaron registradas en el adendum de la ficha. Pendiente
abierto para la mesa: cross-check con 02c cuando exista, y la remediación
documental D1 (gated, operador) ahora con la biblioteca de skills en alcance.
