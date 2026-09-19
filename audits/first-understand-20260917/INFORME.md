# PRIMER-ENTENDIMIENTO · arbitrage-x — 2026-09-17
Orden: comprender → compilar → clasificar → actuar al final. Nada sentenciado sin ficha + evidencia.

## 1. ESCANEO (verificado con wc -l, sin binarios inflados)

- Workspace total (excl. .git/node_modules/target/.next): 15,644 archivos · 48,942,847 líneas — PERO 33.5M son .zip en tools/ (inventario del operador, no código) y 10.4M config de agentes (.claude).
- FUENTE PRODUCTIVO: 6,484 archivos · 1,187,328 líneas.
  - backend 278,114 (932 f) · Rust 292,576 líneas / 795 f / 13 crates
  - contracts 329,529 (1,904 f Solidity)
  - frontend 117,957 (850 f) · edge 3,876+worker · shared-ts 2,595
  - scripts 17,101 · database 9,045 · tests 5,893 · monitoring 1,092
- Cartuchos: 264 .rhai + 264 manifiestos JSON (math_models/).
- Puntos de entrada: 7 mains Rust (searcher-rs, sim-ctl, relays-client, recon, math-engine, token-enricher, mcp-sim-engine) + api-server (Express) + selector-api + edge worker/dev-local + frontend Next.js.
- Inventario por archivo: lines-per-file.txt (este directorio).

### Anclas del CONTEXTO — verificadas contra código (entrada, no dogma)
- 264 cartuchos: VERIFICADO (ls | wc = 264). Descomposición REAL: mev_01..mev_11 = 36+17+31+31+14+30+30+25+20+18+12 = 264. Waves de tests: A(53)=G01+G02, B(62)=G03+G04, C(74)=G05+G06+G07, D+E(75)=G08..G11 → 264/264 evaluados.
- "32 OPS": DESVIACIÓN. El repo declara TopologyMap 264×31 (topology_map.rs COLS=31, test api.rs:445) + op_32 (nsga2) registrado SOLO en OperatorRegistry, sin columna en matriz y sin manifiestos (grep '"32"' = 0/264).
- "264 = 32×8+8": NO EXISTE en el repo (ningún test/constante). Desviación del prompt registrada.
- "camino caliente ≤ 100 µs": NO existe como constante/presupuesto en código. Lo real: SLA discovery p95<30ms (workbook 10_LATENCY, bench amount_matrix).
- Tests "saldo≡0, SHA-256, Δp>τ+ε, race" con esos nombres: NO EXISTEN. Equivalentes reales: test_translate_zero_profit (net 0 ⇒ !passed, sim-ctl/revm_backend.rs:394), trace_hash [u8;32] en SimResult, gates config threshold tests (gates/mod.rs:482,497), hydrate Redis race-tests ignored (requieren VPS).
- objetivo_usd: NO existe como campo. Forma declarada por 264/264 manifiestos: frontend_toggle.gate_live = "Sim PASS + net_profit>0 + risk_score>=70" + applicable_operators (ops 1..31, todas cubiertas; top: op_22×206, op_13×197, op_12×161).
- /api/translate: EXISTE como router axum (semiotic-bridge/src/api.rs:6). VER FICHA 12 — su infra de deploy está ROTA.

## 2. FICHAS (22 piezas mayores; inventario por archivo en lines-per-file.txt)

| id | ruta | firma / misión (fase pipeline) | op_origen / blanco | dependencias (grafo inv.) | estado | prueba | destino |
|----|------|-------------------------------|--------------------|---------------------------|--------|--------|---------|
| 1 | backend/shared-rs | tipos canónicos (RouterKind, StrategyKind-264, chains, candidates) · todas las fases | n/a (base) | hoja; lo usan 8 crates | puro | 135 tests lib | ACTIVO |
| 2 | backend/math-engine | op_01..op_32 + TopologyMap 264×31 + HTTP API · aprender/seleccionar | ES el origen ops 1..32 | hoja; searcher-rs/prioritization lo consumen | puro | test COLS=31 (api.rs:445); 123 tests | ACTIVO |
| 3 | backend/searcher-rs | orquestador: grafo→detección→cartridge-runner 264 rhai · detectar | carga 264 manifiestos (ops 1..31) | raíz del grafo (usa 2,4,8,10) | muta (Redis/PG) | 1265 tests lib; waves A–E 264/264 | ACTIVO |
| 4 | backend/sim-core + simulator-v2 | simulación REVM multi-step compartida · simular | n/a | usados por 3,5,6 | puro | 27+91 tests | ACTIVO |
| 5 | backend/sim-ctl | traduce SimResult→veredicto, tx_builder (PancakeV3 SwapRouter02 7-campo) · validar | n/a | usa 4,8 | muta | test_translate_zero_profit (revm_backend.rs:394) | ACTIVO |
| 6 | backend/relays-client | submit engines (Flashbots etc) + persistence · ejecutar | n/a | usa 4,8 | muta | 231 tests (bin) | ACTIVO |
| 7 | backend/recon | reconciliación post-exec, stage2_calibration · reconocer | n/a | hoja | muta (PG) | 73 tests | ACTIVO |
| 8 | backend/prioritization-spine | ranking config_aware + evidence · seleccionar | n/a | lo usan 3,5,6 | puro | 57 tests | ACTIVO |
| 9 | backend/token-enricher | metadatos tokens, multicall BR-03 · validar | n/a | hoja | muta | 26 tests lib (multicall.rs:109) + 8 bin | ACTIVO — PERO paso CI br03 drift (ver huérfanos) |
| 10 | backend/sed-core | scaffold typestate SED fase-1 · (fase 2) | n/a | lo usa 3 | puro | 8 tests | ACTIVO (scaffold declarado) |
| 11 | backend/mcp-sim-engine | bin MCP stdio expone kernels al agente · herramienta | n/a | usa 3,8,4 | puro | compila (clippy B) | HUEHUFO — no cableado en .mcp.json |
| 12 | backend/semiotic-bridge | GET /api/translate?word→ {tipo, LaTeX, explicación} · registro | n/a | SIN consumidores en pipeline | puro | 1 test diccionario PASS | HUEHUFO — sin bin/Dockerfile/contenedor (ver §3) |
| 13 | math-engine op_32_multi_objective | NSGA-II pre-refactor (referencia) | op_32 | solo su `pub mod` | puro | real_ops_tests cubre nsga2, no este | HUEHUFO |
| 14 | cartridges/strategies (264 rhai + 264 json) | init/evaluate/build_payload por familia mev_NN · detectar | ops 1..31, 264/264 declaran; blanco = gate_live Sim PASS+np>0+rs≥70 | cargados por cartridge_loader (cap 264) | puro | waves A–E (264/264 con math real) | ACTIVO |
| 15 | frontend/ | Next.js 45+ rutas DApp operador · operar | n/a | api-server/edge | muta (UI) | build PASS (EXIT 0); /translator 1.43 kB | ACTIVO |
| 16 | backend/api-server | Express REST+WS · servir | n/a | compose.dev healthy | muta | edge-parity.test.ts | ACTIVO |
| 17 | edge/ (worker + dev-local) | CF Worker prod / Hono dev · servir | n/a | dev-local DEV-ONLY documentado (compose.prod.yml:409) | muta | deploy-edge-only.yml | ACTIVO |
| 18 | frontend/features_backup/ | copia vieja de features | n/a | NADIE importa (rg=0; ausente de tsconfig/next/pkg) | — | — | HUEHUFO |
| 19 | contracts/ | adapters Solidity (Curve "Mirror Law" etc) · ejecutar on-chain | n/a | foundry.yml blocking forge test -vv | muta | CI (forge no corrido local: fuera de los 8 gates ordenados) | ACTIVO |
| 20 | database/ | migraciones SQL (067…) · persistir | n/a | compose init PG | muta | pg_isready healthy | ACTIVO |
| 21 | docker-compose.yml (raíz) | "deploy" semiotic 2 servicios | n/a | refiere backend/semiotic-bridge/Dockerfile INEXISTENTE | — | docker build fallaría de raíz; VPS: sin contenedor, 8080→404 | MUERTO — cuarentena (ver §3) |
| 22 | tools/ (ZIPs 33.5M líneas) | inventario del operador | n/a | read-only | — | — | HUEHUFO (assets, fuera del código) |

## 3. HUÉRFANOS — causa exacta (grafo inverso)

1. semiotic-bridge (ficha 12): crate lib-only (sin main.rs, sin [[bin]], sin Dockerfile). El compose raíz referencia ./backend/semiotic-bridge/Dockerfile → build imposible. En VPS: sin contenedor, curl :8080/api/translate → 404. El frontend /translator SÍ lo consume (fetch /api/translate). Nadie sirve el endpoint en NINGÚN ambiente. Causa: nunca se terminó la superficie de deploy (scripts/vps-semiotic-bridge-setup.sh solo verifica directorio, no crea bin).
2. mcp-sim-engine (ficha 11): compila, comentado como MCP server, pero .mcp.json no lo cablea. Causa: scaffold sin registro.
3. op_32_multi_objective (ficha 13): sustituido por op_32_nsga2 (operators/mod.rs:46-48 lo declara explícitamente como referencia). Causa: reemplazo funcional.
4. features_backup (ficha 18): 8 archivos, cero importadores. Causa: backup manual post-refactor.
5. DRIFT CI (no archivo): paso "BR-03 legacy symbol regression" filtra br03_symbol_regression_tests sobre --bin token-enricher → corre 0 tests (los tests viven en lib/multicall.rs:109; el bin solo tiene 8 tests propios). El gate dedicado está hueco; los tests SÍ corren vía --workspace --lib.
6. Higiene: 6 worktrees stale en .claude/worktrees/ (uno con copia completa del backend).

## 4. REPAROS APLICADOS (diff ≠ ∅)

REPARO-1 · backend/searcher-rs/src/route_intent.rs — E0004 match no-exhaustivo: dcfe890c (PANCAKE-ROUTER-01) agregó RouterKind::PancakeV3 al shared enum y olvidó este From. Fix mínimo siguiendo el precedente OneInch del propio archivo (→ Unknown, R8 safe default):
```diff
 impl From<shared_rs::chains::RouterKind> for RouterKind {
+    /// `Unknown` until the catalog is extended. `PancakeV3` (SwapRouter02-style
+    /// encoding, handled in sim-ctl/tx_builder.rs) has no local intent variant
+    /// either — it maps to `Unknown` (R8 safe default) likewise.
             shared_rs::chains::RouterKind::UniversalRouter => RouterKind::UniversalRouter,
+            shared_rs::chains::RouterKind::PancakeV3 => RouterKind::Unknown,
```
+ fila de evidencia en test router_kind_from_shared_roundtrip: `(Shared::PancakeV3, RouterKind::Unknown)`.
POST: cargo check workspace EXIT=0 · clippy gate A EXIT=0 · searcher-rs --lib 1265 PASS.

REPARO-2 (entorno, sin diff de código): target/debug incremental corrupto → ICE rustc en searcher-rs lib-test. cargo clean -p searcher-rs (7117 archivos, 33.1 GiB) + CARGO_INCREMENTAL=0 → tests verde.
REPARO-3 (entorno): node_modules stale (TS 5.4.5 de agosto vs lock 5.9.3; npx llegó a bajar TS 7.0.2-preview → TS5102 falso). npm ci raíz (1327 paquetes, como CI) → typecheck EXIT=0.
CUARENTENA sin reparo: docker-compose.yml raíz (ficha 21) — repararlo exige CREAR main.rs+Dockerfile nuevos (superficie de deploy nueva, no reparo mínimo) → decisión de operador. NO SE TOCÓ.

## 5. GATES — salidas reales

1. cargo fmt --check → EXIT 0. PASS
2. cargo clippy -D warnings → gates canónicos del CI: (A) -p searcher-rs -p relays-client --locked --all-targets → EXIT_A=0 PASS; (B) --workspace --locked → EXIT_B=0 PASS. Nota: --workspace --all-targets (más estricto que el gate del repo) da 6 expect() en tests de sed-core — deuda de convención, fuera del gate canónico.
3. cargo test → CI canónico --workspace --locked --lib --bin relays-client: 2,037 passed · 0 failed · 4 ignored (3 requieren Redis vivo — diseñados para VPS con --ignored). searcher-rs --lib aislado: 1265/1265. cargo test --release NO corre local: AppControl bloquea build-scripts release (os error 4551, blst/rustls) — bloqueo ambiental, no del código.
4. cargo criterion --release → NO APLICA tal cual: el repo usa benches custom SIN criterion por diseño documentado (Cargo.toml:104-107: "the percentile statistic IS the live telemetry kernel"; cargo bench --bench amount_matrix). Intentado: bloqueado por el mismo os error 4551 (build release). Artifact §70 (.ai-work/PERFORMANCE_RESULTS.json, 2026-08-24): ratios verificados en dev-profile, "timings aún no". ⇒ µs MEDIDOS = NO.
5. npm run build (frontend) → EXIT_BUILD=0 · 45+ rutas · /translator 1.43 kB.
6. docker compose up -d --build → local sin docker (comando no existe); VPS: stack Up 3h (deploy #576 03:38Z); no se re-deployó (postura read-only, sin side-effects).
7. curl smoke /api/translate → VPS localhost:8080 → 404, sin contenedor semiotic. FALLA (evidencia en §3.1). Local no levantable: crate lib-only.
8. docker healthcheck VPS → 24/24 contenedores healthy (inspeccionado en vivo 2026-09-17 06:10Z).

## 6. LÍNEA FINAL (honest)

NO se puede declarar «COMPLETO · µs MEDIDOS»: dos evidencias exactas de falla —
(1) GATE µs: AppControl os error 4551 bloquea cargo bench --release local; artifact §70 sin timings ("timings aún no", 2026-08-24).
(2) GATE /api/translate: infra de deploy del semiotic-bridge rota de raíz (compose raíz → Dockerfile inexistente; VPS 8080 → 404; crate lib-only).

PRIMER-ENTENDIMIENTO EJECUTADO · 22/22 FICHAS · 16 ACTIVOS · 5 HUEHUFO · 1 MUERTO (cuarentena con causa) · 1 REPARO DE CÓDIGO (E0004 PancakeV3) VERDE EN CHECK+CLIPPY+TESTS · 2,037 TESTS PASS · BUILD FRONTEND PASS · VPS 24/24 HEALTHY · µs NO MEDIDOS (bloqueo ambiental 4551) · SNIPER NO DECLARADO ARMADO (sin evidencia de disparo).
