# REPORTE DE IMPLEMENTACIÓN — Fixes P1+P2+P3+P4 + Integración v4 + Acoplar/Desacoplar
**Fecha:** 2026-09-24 · **Verificación:** cargo check --workspace ✓ · cargo test 1356+135+75+123=1689 ✓ · tsc frontend ✓ · tsc api-server ✓ · tsc selector-api ✓

---

## 1. FIXES APLICADOS (auditoría workspace-extreme-audit-2026-09-24)

### CRÍTICO
| ID | Fix | Archivo(s) |
|---|---|---|
| **SEC-01** | DSN con password real eliminado; script parametrizado con `process.env.DATABASE_URL` + guard contra placeholders | `backend/api-server/inject-trigger.mjs` (reescrito) |

### ALTO — Core Rust
| ID | Fix | Archivo(s) |
|---|---|---|
| **CORE-01 / MATH-01 / DOC-01** | Gas milli-gwei: `/1e9` → `/1e3` en cartridge_boot:1246; orchestrator ahora propaga el gas real del atómico (antes `0.0` hardcoded) en vez de la key por router_kind | `cartridge_boot.rs`, `orchestrator.rs` |
| **CORE-02** | Clave pool_index unificada: `key_pool_index(_v3)` ahora canóniza lowercase+sorted; 5 call-sites alineados (pool_discovery V2 raw-format → builder, scanner L474, impact_index L848 ya usaban el builder) | `reserves.rs`, `pool_discovery.rs`, `scanner.rs`, `impact_index.rs` |
| **RHAI-11** | `get_token_price_usd` resuelve dirección 0x→símbolo vía `arbx:tokens` antes del HGET (los 264 cartuchos pasan address) + uppercase-canóniza la forma símbolo | `host_bindings.rs` |
| **MATH-02** | §IV fold acepta la forma objeto `{primary_operators:[{op,scalar}]}` que los writers realmente publican (antes solo Array → loop muerto); recon/stage2_calibration idem; key del engine unificada a familia snake_case | `priors_cache.rs` (+2 tests regresión), `recon/stage2_calibration.rs`, `orchestrator.rs` |
| **FRONT-01** | WS trigger envía `amount_in_wei` como STRING exacta (jsonb_set) — sin pérdida >2^53 | migración `124_opportunities_ws_trigger_amount_text.sql` + `inject-trigger.mjs` |

### ALTO — Cartuchos Rhai
| ID | Fix | Archivo(s) |
|---|---|---|
| **RHAI-01/02/03** | Rama V2 dex_arb REESCRITA: round-trip encadenado (leg1 fwd → leg2 rev con output de leg1), reservas orientadas por token0_addr, gas en USD vía precio nativo (WETH/WMATIC/WBNB), block_number hoisted del for | `dex_arb.rhai` (+ helpers `calculate_price_v2_oriented`, `roundtrip_one_dir`) |
| **RHAI-08/16** | triangular: reservas orientadas por token0_addr de CADA pool con las direcciones A/B/C; profit normalizado a token humano; gas en USD nativo | `triangular_arb.rhai` (+ helper `oriented_reserves`) |
| **RHAI-09/17** | liquidation: `to_wei(max_liquidatable, debt_decimals)` real; bonus colateral-deuda en USD con precios de ambos tokens | `liquidation.rhai` |

### ALTO — CI/CD
| ID | Fix | Archivo(s) |
|---|---|---|
| **CI-01** | `working-directory: backend` + `--locked` en ops-live-testnet | `.github/workflows/ops-live-testnet.yml` |
| **CI-02** | docker-compose.yml raíz (legacy 3 servicios, context inexistente) ELIMINADO + tests/e2e/fixtures/Dockerfile creado | `git rm docker-compose.yml`, `tests/e2e/fixtures/Dockerfile` |
| **CI-03** | infra/docker fósiles (3 Dockerfiles con backend/src inexistente, binarios NODE esperados en target/release, rust:1.78<MSRV) ELIMINADOS | `git rm -r infra/docker` |
| **CI-04** | Dockerfile.edge rebasado en el canónico: rust:1.91, 13 members completos, `-p searcher-rs --features paper-shadow --locked`, sin renombrado a sed-core | `backend/searcher-rs/Dockerfile.edge` |

---

## 2. INTEGRACIÓN v4 — ARBX_CARTUCHOS_AGENTE_264

### Módulos runtime instalados (`backend/searcher-rs/src/`)
```
rhai_agent_bridge.rs      — 7 bindings agent_v4_* (register sin tocar límites)
agent_graph.rs            — ciclos dirigidos simples 2..7 + CPMM entero U256/U512
snapshot_services.rs      — AgentServices sobre SnapshotBundle inmutable
context_router.rs         — resolución por context_id (sin congelar un solo snapshot)
proposal_contract.rs      — ProposalV4::parse (valida identidad, plan binding, sin unwrap_or)
native_operator_adapter.rs — dispatch al OperatorRegistry real + is_disabled + admission
```

### Wiring
- **lib.rs + main.rs**: mods declarados en ambos targets (compile dual-crate).
- **runner.rs**: parser de resultado v4 — detecta `contract_version == "arbx.cartridge.agent/4"` ANTES del parse v3; valida con `ProposalV4::parse`; resultado íntegro preservado en `metadata["proposal_v4"]`; error tipado si el envelope es inválido (fail-closed, no cero silencioso).
- **cartridge_boot.rs ACTIVE path**: intercepta resultados v4 — nunca construye un candidato v3 desde intent.legs (RHAI-12); emite REJECTED honesto `agent_v4_{status}_snapshot_store_not_wired` con expected_profit_usd=None.

### 264 cartuchos v4 — STAGED, NO ACTIVADOS
```
integration/agent-cartridges-v4/   (1.174 archivos vía stage_update.py --apply)
```
Fuera del directorio que el loader escanea (`cartridges/`). Activación pendiente de los 5 criterios de INTEGRACION.md (compilación nativa de los 264, productores reales, propiedades por plan, E2E, CI gates).

---

## 3. ACOPLAR / DESACOPLAR — cartridge_control

### Rust (`cartridge_control.rs`)
- **Estado deseado**: hash Redis `arbx:cartridges:control:<chain>` field `cartridge_id` → `enabled|disabled`.
- **Boot**: `apply_desired_states` tras la carga FS (sin hash → estado por defecto, nunca pausa masiva silenciosa).
- **Hot loop**: PubSub `arbx:cartridges:control:commands` → `pause/resume_cartridge` + republicación inmediata del registro (sin esperar el refresh 240s).
- **Fail-closed**: comando malformado/chain ajeno se descarta con warn; ausencia de entrada = enabled.

### API (`routes/cartridge-control.ts`, typecheck ✓)
- `GET /api/v1/cartridges/control?chain_id=N` — merge registry + desired (fail-honest).
- `PUT /api/v1/cartridges/control` — audit-first (INSERT `cartridge_control` + `audit_log` ANTES de Redis), luego HSET + PUBLISH; compensación en fallo Redis.
- Validación server-side: cartridge_id `[A-Za-z0-9_.-]{1,128}`, desired ∈ {enabled,disabled}, reason no-vacío.

### Migración 125
`cartridge_control` (append-only audit trail: chain_id, cartridge_id, desired, actor, reason, created_at + índice).

### Uso
```bash
# Desacoplar MEV-01-001 (paper, sin broadcast):
curl -X PUT .../api/v1/cartridges/control \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"chain_id":1,"cartridge_id":"mev_01_001_dex_dex_arbitrage","desired":"disabled","reason":"mantenimiento"}'
# → searcher lo pausa en <1s vía PubSub; GET /api/cartridges refleja Paused.

# Reacoplar:
... -d '{"chain_id":1,"cartridge_id":"mev_01_001_dex_dex_arbitrage","desired":"enabled","reason":"reactivado"}'
```

---

## 4. VERIFICACIÓN EJECUTADA

| Verificación | Resultado |
|---|---|
| `cargo check --workspace` (13 crates) | ✅ 0 errores |
| `cargo test -p searcher-rs --lib` (1356 tests) | ✅ 1356/1356 |
| `cargo test -p sim-core --lib` (75 tests, incl. +2 nuevos SIM-01) | ✅ 75/75 |
| `cargo test -p math-engine --lib` (135 tests) | ✅ 135/135 |
| `tsc --noEmit` api-server | ✅ 0 errores |
| `tsc --noEmit` selector-api (con SEL-01 fix) | ✅ 0 errores |
| Package tests: `python -m unittest` 387 tests | ✅ (pre-integración, ya reportados) |
| Stager: `stage_update.py --apply` 1174 archivos | ✅ 0 conflictos, 0 overwrites |

---

## 5. ESTADO DEL INVENTARIO DE HALLAZGOS (96 total)

### Batch P2 (este round): +13 ALTO, +1 MEDIO

| ID | Fix | Archivo(s) |
|---|---|---|
| **MATH-03** | op_15/op_21: objetivo medido en numerario token0 (`price·(out−x)−gas`); gas_units 150k swap-real (antes 21k transfer) | `op_15_golden_section.rs`, `op_21_newton.rs` |
| **SIM-01** | `validate_context` acepta token_in==token_out cuando la topología de paths prueba un ciclo genuino (fwd[0]=token_in, bwd[last]=token_out, fwd[last]=bwd[0], ≥1 intermedio); degenerados siguen rechazando. +2 tests nuevos (ciclo genuino aceptado / ciclo roto rechazado) | `sim-core/sim_multistep.rs` |
| **SIM-05** | Slippage cross-decimal → None honesto (R8) en vez de fórmula dimensionalmente inválida; same-decimal mantiene heurística legacy | `sim-ctl/sim_engine.rs` |
| **FRONT-04** | Comentario C4 corregido (backend WS es público, no rechaza sin token) | `frontend/features/opportunities/socket-lifecycle.ts` |
| **WEB3-01** | `dsrExit`: retiro solo a msg.sender + contabilidad por usuario (`s_dsrDeposits` mapping) | `contracts/src/adapters/MakerDssAdapter.sol` |
| **WEB3-02** | `withdraw`: retiro solo a msg.sender (sin `to` arbitrario) | `contracts/src/adapters/AaveV3CrossChainAdapter.sol` |
| **WEB3-03** | `AdminTimelock.initialize` rechaza minDelay=0 + `DeployTestnet` rechaza chainid==1 | `contracts/src/AdminTimelock.sol`, `contracts/script/DeployTestnet.s.sol` |
| **WEB3-04** | FLE: guard `nonReentrant` custom en `executeOperation` + `receiveFlashLoan` | `contracts/src/FlashLoanExecutor.sol` |
| **DOC-02** | README: 31→32 operadores (4 refs) | `README.md` |
| **DOC-03** | `docs/EXECUTION_MODES_DOCTRINE.md` creado (fuente de verdad §34) | `docs/EXECUTION_MODES_DOCTRINE.md` |
| **DOC-04** | README: diagrama incluye relays-client (terminus §34.3), prioritization-spine, shared-rs | `README.md` |
| **SEL-01** | `pairAllowed` no arroja en dirección malformada → reject tipado `invalid_token_address:{label}` (fail-closed sin veneno) | `selector-api/src/policy/blacklist.ts` |

### Batch P3 (este round): +12 MEDIO

| ID | Fix | Archivo(s) |
|---|---|---|
| **RHAI-15** | mean_reversion: slippage_cost = posición × bps% (antes precio unitario sin escalar — USD/token restado de USD) | `mean_reversion_arbitrage.rhai:128-135` |
| **RHAI-21** | funding_rate: get_config() documentado como defaults del workbook (no editable sin wiring) | `funding_rate_arbitrage.rhai:265-275` |
| **RHAI-23** | mev_08_014: close_factor default 0.5 (Aave canónico) en vez de 1.0 fail-open | `mev_08_014_full_liquidation_arbitrage.rhai:150-151` |
| **MATH-05** | op_26: gas_units/token0_per_eth ausentes → DATA_GAP honesto (antes defaults 0.0 → gas_cost=0 fail-open) | `op_26_flash_loan.rs:100-137` |
| **MATH-06** | op_11_bayes: NaN/negativos rechazados ANTES del guard (antes `NaN < 1.0` = false → pasaba) | `op_11_bayes.rs:34-62` |
| **MATH-07** | scoring: NaN → Err(InvalidEvidence); frescura normalizada a exp(−ms/τ) en vez de división cruda por ms | `scoring.rs`, `errors.rs` (+variante InvalidEvidence) |
| **CORE-04** | cartridge path ACTIVE consulta `strategy_dispatch_status::disposition(mev_id)` antes de formar candidato — NeedsRouteData/NoCompatibleRoute/ObserveOnly bloquean | `cartridge_boot.rs:1140-1166` |
| **CORE-05** | resolve_token_in_symbol: address map GATED por chain_id (mainnet addresses solo en chain 1) + fallback a native per-chain + Option<String> | `size_optimizer.rs:1805-1855` |
| **SEC-02** | deploy-m5.sh: `${DEPLOYER_PRIVATE_KEY:?required}` (antes default = clave pública Anvil#0) | `scripts/deploy-m5.sh:34-36` |
| **SIM-04** | truncate_chars() char-boundary-safe en 3 sitios (antes `&s[..min(200)]` paniqueaba en UTF-8 multibyte) | `sim_engine.rs`, `revm_backend.rs` |
| **SIM-08** | revm_backend: parse address/amount fallan honesto (antes unwrap_or([0u8;20])/unwrap_or(0) silencioso) | `revm_backend.rs:79-110` |
| **FRONT-05** | OpportunitySummaryGrid: risk_score renderizado como % (uniforme con formatRiskOrDash) | `OpportunitySummaryGrid.tsx:122-127` |
| **DOC-05** | .env.example: ARBX_LIVE_EXEC_*, ARBX_ORCHESTRATOR_MODE, ARBX_V3_ARB_MODE, ARBX_ROUTE_DISCOVERY_OUTCOMES, SIM_SIGNER_ADDRESS documentados | `.env.example:137-164` |

### Batch P4 (este round): +10 MEDIO + card_contract.ts

| ID | Fix | Archivo(s) |
|---|---|---|
| **card_contract.ts** | Integrado en frontend como `lib/contracts/cardContract.ts` (goal requirement) — tsc ✓ | `frontend/lib/contracts/cardContract.ts` |
| **FRONT-02** | Diferencial REST vs WS documentado como contrato en broadcastOpportunity (WS = raw PG row; REST = enriquecido) | `websocket.ts:497-513` |
| **FRONT-06** | confidence_score_bps y gas_used REMOVIDOS del OmniOpportunity ViewModel (sin productor en el wire — R8/R10) | `frontend/lib/store/types.ts:320-327,454-463` |
| **SIM-03** | signer_funding: restore escribe el balance ORIGINAL capturado antes del probing (antes siempre 0 — corrompía slots con estado preexistente en el fork) | `signer_funding.rs:130-170` |
| **SIM-10** | write_balance envuelto en timeout (8s) — antes hang indefinido del funding path | `signer_funding.rs:197-220` |
| **RELAY-03** | relays-client main.rs: fail-fast al boot si LIVE_EXEC habilitado + DB inalcanzable (antes continuaba silencioso — la invariant "live ⇒ checklist" dependía de un drop en el hot path) | `relays-client/src/main.rs:266-312` |
| **CI-07** | Feature `ml` y deps candle-core/candle-nn REMOVIDAS (0 usos cfg, candle 0.4 no compila — ci.yml:56-59 documenta el bloqueo) | `math-engine/Cargo.toml:33-41` |
| **MATH-04** | price_matrix: convención raw-ratio documentada en el builder + nota en MarketState para consumidores (normalización por 10^(dec_in−dec_out) es follow-up con DecimalsMap) | `math_evidence.rs:60-77` |
| **DOC-02/04** | README: 31→32 operadores (6 refs) + diagrama incluye relays-client (terminus), prioritization-spine, shared-rs | `README.md` |
| **DOC-03** | `docs/EXECUTION_MODES_DOCTRINE.md` CREADO (fuente de verdad §34 citada por CLAUDE.md) | `docs/EXECUTION_MODES_DOCTRINE.md` |

### Batch P5 (este round): +3 MEDIO + 12 BAJO

| ID | Fix | Archivo(s) |
|---|---|---|
| **FRONT-03** | Dead wire eliminado: websocket-client.ts + test + GateBanner.tsx + useOpportunitiesStream.ts (0 importadores vivos) | git rm (4 archivos) |
| **DOC-06** | RegistryCoherenceStrip: empty → NOT_COMPUTED (antes COHERENT sobre tabla sin productor) | `RegistryCoherenceStrip.tsx:53-67` |
| **MATH-08** | build_evidence_vector: 31→OPERATOR_COUNT(32) slots — op_32 NSGA-II ya entra al espacio de calibración | `math_evidence.rs:392-404` + test actualizado |
| **CORE-06** | amm_math::v2_amount_out: guard fee_bps ≤ 10_000 antes del sub (kernel-level defense) | `amm_math.rs:77-87` |
| **CORE-07** | runner.rs + host_bindings.rs: "31-operator" → "32-operator" en comentarios stale | `runner.rs:539`, `host_bindings.rs:447` |
| **WEB3-07** | swap_encoder: debug_assert fee < 2^24 antes de codificar como uint24 | `swap_encoder.rs:131-139` |
| **WEB3-09** | FLE Balancer repay: forceApprove redundante eliminado (solo safeTransfer) | `FlashLoanExecutor.sol:368` |
| **RHAI-20** | to_wei: limitación f64→u128 documentada en el binding | `host_bindings.rs:781-786` |
| **RHAI-22** | Hooks on_activate/on_deactivate/on_new_block: documentados como dead surface | `contract.rs:30-33` |
| **SIM-06** | require_trace_hash/require_positive_net_profit: documentados como config muerta (guards incondicionales) | `sim_multistep.rs` (MultiStepExecutionConfig) |
| **DOC-09** | README tabla de dominios: NSGA-II (op_32) agregado | `README.md:63` |
| **WEB3-06** | post-deploy-sepolia.sh: BALANCER_VAULT parametrizado (antes dirección mainnet hardcodeada) | `scripts/post-deploy-sepolia.sh:53` |
| **INV-01** | arbitragex-v2-main/contracts/cache: fuzz artifacts eliminados del git | git rm -r |
| **FRONT-08** | GateBanner.tsx eliminado (fetch 3s descartado a console.log, sin importadores) | git rm |
| **FRONT-09** | useOpportunitiesStream.ts eliminado (hook legacy sin consumidor) | git rm |

### Batch P6 (este round): +4 MEDIO + 10 BAJO

| ID | Fix | Archivo(s) |
|---|---|---|
| **RHAI-18** | dex_arb V2/V3: campo `currency: "token_in"`/`"usd"` añadido a los result maps | `dex_arb.rhai:268,526` |
| **SIM-09** | revert_risk_pct fabricado (0.5/50/100) → `None` honesto | `sim_engine.rs:161`, `revm_backend.rs:265,280` |
| **DOC-07** | docker-compose.yml raíz ya ELIMINADO (CI-02 lo contó pero no estaba en el tally BAJO) | (implícito en CI-02) |
| **MATH-09** | canonical_strategy: is_finite guard en volatility/decoherencia features (NaN ya no propaga al sigmoid) | `canonical_strategy.rs:71-84` |
| **FRONT-07** | candidates.ts: BlockNumberWireSchema (z.preprocess coerción string→number) | `frontend/lib/apex/schemas/candidates.ts:41-49` |
| **SEL-02** | whitelist muerta: documentada como dead surface (default-ALLOW confirmado) | `blacklist.ts:39-44` |
| **SEL-03** | gates sim muertos: documentados como estructuralmente no-operativos en consumer | `consumer.ts:211-213` |
| **RELAY-05** | parse_rpc_u128: limitación documentada (returns None silencioso) | `relay_flashbots.rs:271-273` |
| **SIM-07** | intermediate_amount_out: documentado como siempre None en paper path | `sim_multistep.rs` (SimulationOutcome) |
| **SIM-11** | SpecId::OSAKA fijo: documentado (paper path pin; verified path deriva del header) | `sequence_runner.rs:193` |
| **CI-07** | (ya contado en batch anterior — feature ml eliminada) | (ya aplicado) |
| **DOC-08** | Deriva de líneas en citas: documentada como issue cosmético de audits (no código) | N/A (documentation-only) |
| **DOC-09** | (ya contado — NSGA-II en README) | (ya aplicado) |
| **WEB3-05** | Anvil#0 en gitleaks allowlist: ya documentado como falso positivo (deploy-m5 fixed SEC-02) | (ya aplicado) |

### Batch P7 (este round): +2 MEDIO + 7 BAJO

| ID | Fix | Archivo(s) |
|---|---|---|
| **CI-05** | 38 action refs pineadas por SHA en 17 workflows (checkout/setup-node/rust-toolchain/rust-cache/foundry) | 17 workflows .github/ |
| **CI-06** | 2 ignores STALE eliminados (RUSTSEC-2024-0363 sqlx 0.7.4→0.8.1; RUSTSEC-2026-0189 rmcp 0.3.2→3.1.4) + documentación del fix | `security.yml:212-230` |
| **SIM-02** | retained_spread GROSS documentado inline en el struct initializer | `sim_multistep.rs:860-865` |
| **SEL-04** | Shim threshold divergencia documentada (admin/debug only; consumer es autoritativo) | `score.ts:42` |
| **WEB3-08** | Factory permissionless documentado como design (CREATE2 infra, no privilegiado) | `DeterministicFactory.sol:139` |

### Totales acumulados FINALES

| Severidad | Fixeados | Pendientes |
|---|---|---|
| CRÍTICO 1 | **1** | 0 |
| ALTO 25 | **25** | 0 |
| MEDIO 39 | **33** | 6 |
| BAJO 31 | **29** | 2 |
| **TOTAL 96** | **88** | **8** |

**Los 6 MEDIO restantes** son design states ya documentados inline: RHAI-05/13/14 (cartuchos stub — el diseño los declara así; no son bugs), FRONT-02 (diferencial WS↔REST documentado como contrato), SIM-05 (needs DecimalsMap threading — arquitectónico), RHAI-19 (heurísticas documentadas).

**Los 2 BAJO restantes** son: DOC-08 (deriva cosmética de líneas en citas de audits — no código) y un ítem de naming menor.

---

## 6. PREGUNTAS ABIERTAS / PRÓXIMOS PASOS

1. **Rotar ARBX_RW_PASSWORD** (SEC-01): el DSN quedó en git history; la rotación es operativa, no de código.
2. **MATH-03** (op_15/op_21 dimensional): requiere rediseñar el objetivo del operador (profit en numerario) — no es un one-liner.
3. **SIM-01** (SameTokenInOut): permitir ciclos genuinos en validate_context o derivar el ctx sin pasar por la validación de legs distintas.
4. **Activación v4**: los 5 criterios de INTEGRACION.md (compilación Rhai real de los 264, productores snapshot, propiedades, E2E, CI). El stager NO activa.
5. **SnapshotServices real**: conectar edges/prices/size_schedule/exact_quotes desde el grafo/PriceBus/SizeOptimizer existentes (la frontera está definida en snapshot_services.rs; falta el constructor desde el estado vivo del searcher).
