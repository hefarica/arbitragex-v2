# Informe Final — Auditoría Estática 0-100% (Workflow #1 · 20 agentes · 2026-09-06)

> Run wf_74cd637d-e73 · 15/20 agentes vivos (5 muertos por 429 del provider LLM: simulator, performance, docs-drift, relays-exec, security-doctrine-lens)
> **103 findings**: 1 critical · 20 high · 40 medium · 42 low — 20 CONFIRMED (unanimidad 3/3 lentes + materialidad) · 4 REFUTED · 79 unverified
> Cobertura honesta del critic: **~60% de la superficie ESTÁTICA, 0% de la DINÁMICA** — ver su lista de 17 prioridades al final del informe y en la sección critic del run

---

# INFORME FINAL DE AUDITORÍA 0–100% — DApp ARBITRAGEX

**Ámbito**: working tree branch `a6-cbprom-01` (= main 086347f7 + merge #545). Modo: read-only absoluto, **estático** (sin runtime, sin build, sin on-chain, sin broadcast). Fecha: 2026-09-06. Método: síntesis ejecutiva sobre 10 auditores de superficie + proceso de 4 lentes (3 verificadores + materialidad). Anclaje con deriva declarada: snapshot git-status `086347f7` vs auditor canon anclado a `f46a0522` — los conteos requieren re-anclaje al HEAD final.

**Cuadro maestro de findings**

| Estado | Total | Critical | High | Medium | Low |
|---|---|---|---|---|---|
| CONFIRMED de pie | 20 | 0 | 17 | 3 | 0 |
| REFUTED (lente materialidad) | 4 | 1 | 3 | 0 | 0 |
| UNVERIFIED (sin ronda de lentes) | 79 | 0 | 0 | 37 | 42 |
| **Total** | **103** | **1** | **20** | **40** | **42** |

Señal de calidad del proceso: los 20 CONFIRMED de pie tienen consenso unánime 3/3 de lentes y sobrevivieron el lente de materialidad; los 4 refutados fueron exactamente los 4 con tally dividido 2-1. Cero findings con estado UNVERIFIABLE (todas las tally tienen unverifiable=0).

**Cobertura honesta**: 10 de 14 superficies con auditoría estática dedicada; 4 auditores muertos con cero hallazgos dedicados (simulator, relays-exec, docs-drift, performance). Veredicto de cobertura del critic: **0 a ~60% de la superficie ESTÁTICA y 0% de la DINÁMICA**. "0 a 100%" es inalcanzable con esta base.

---

## 1. EXECUTIVE SUMMARY

**Hechos (solo CONFIRMED, 3/3 + materialidad, evidencia file:line):**

1. **La capa económica miente antes de los gates** — el defecto de clase más grave para §34.4. Tres hallazgos convergentes: (a) **F13**: ningún constructor de producción cablea relay-bribe/p_fail/feedback — el componente de costo que el propio engine documenta como "10-50% overstatement of net profit on mainnet" existe, está testeado, y corre siempre a `estimated_relay_fee_usd=0.0`; (b) **F2**: el SizeOptimizer (motor económico) colapsa el símbolo de tokens no listados a "WETH" con decimales 18 — Topological Yield sobrestimado ~1e8x en detección para tokens tipo PEPE; (c) **F5**: fee por pierna hardcodeado a 30 bps en el kernel triangular. Con capital real, el filtro de viabilidad aprobaría sistemáticamente rutas inviables.
2. **El pipeline de evidencia paper está estructuralmente muerto** — **F93**: el único productor de `arbx:hot:simulated` jamás se invoca Y escribe un esquema más pobre que el que el único consumidor exige; los campos ausentes de gas/Decoherencia se coaccionan a 0 (None→0, anti-R8). Coherente con el síntoma histórico "0 paper runs".
3. **La supervisión muere en el terminus de notificación** — **F81**: el único receptor activo de Alertmanager postea a un webhook con gate de admin-token que no tiene token configurado → toda alerta (incluidos los 10 circuit breakers doctrinales de esta rama) muere en 401. "Nada muere en silencio" violado en la capa que existe para gritar.
4. **Perímetro evitable** — **F44**: el rate-limit 429 per-IP se ancla a headers forjables (cf-connecting-ip / primer hop XFF) con el puerto 8787 en 0.0.0.0 → un curl por request obtiene bucket fresco. **F45**: un middleware de "sensura" reescribe payloads JSON en la variante dev-local (anti-RULE 00 por construcción) y es código muerto en el worker — dos contratos de payload en el mismo repo.
5. **Cadena de despliegue divergente de su doctrina** — **F55** (IP del VPS + usuario root SSH hardcodeados y versionados, con fallback a SHA obsoleto), **F56** (cero `--env-file`, healthcheck a endpoint equivocado), **F57** (rollback sin rebuild = reutiliza las imágenes rotas nuevas), **F58** (el auto-deploy a producción no implementa el cache-busting RULE 03 ni ejecuta el verificador L4 que ya existe).
6. **Mensajería at-least-once que no lo es** — **F69**: el DLQ de relays-client depende de un ciclo XAUTOCLAIM/XCLAIM que no existe en el código (solo en comentarios); los fallos de persistencia varan entradas en el PEL para siempre.
7. **Canon con dos verdades** — **F87**: `strategy_mapping.json` (stale, 2026-07-20) diverge del canon en 264/264 estrategias y su derivado alimenta la UI de operadores, autotitulándose "single source of truth" contra la cadena canónica que SÍ tiene enforcement CI.
8. **Credenciales admin con vías de fuga latentes** — **F36** (fallback `NEXT_PUBLIC_ARBX_ADMIN_TOKEN` en código cliente activo: uncomment del .env.example = token admin en el bundle público) y **F37** (lectura de token legacy desde localStorage en /dex-registry, contradiciendo V-AT-1).
9. **Contratos**: **F27** — MakerDssAdapter.dsrExit permissionless sin contabilidad por usuario (drenaje directo **condicional a despliegue**; grep confirma cero wiring).
10. **Hot-path serial** — **F3**: pipeline completo (engines + SizeOptimizer con hasta 16 staticcalls + emit PG/Redis + REVM) awaited inline en el único consumidor del stream; la optimización Slot0Cache early-reject existe pero sin wiring (condicional a flags runtime).
11. **R8/las medium confirmadas**: **F4** (ReservesCache sin evicción ni staleness: sizing contra liquidez fantasma si pool_sync muere), **F6** (INSERT de risk_event severity=critical con `let _ =` en recon).

**Patrón sistémico dominante (dato, no opinión)**: la matemática correcta existe pero el cableado de producción no la usa; y los defectos más vistosos están **doble-muertos por ausencia de wiring** — las 4 refutaciones lo demuestran (ver Apéndice A). La seguridad presente descansa parcialmente en que código peligroso NO está cableado y en que flags NO están activos. Un flip a LIVE_MAINNET activaría precisamente esos planos.

**Contra-evidencia positiva (anti-fabricación)**: censo de sentinels `0x…dEaD` limpio (todas legítimas), oráculos de precio fail-closed, stubs 501 honestos, estados vacíos honestos en frontend, canon cuantitativo coherente (264 estrategias == 264 cartridges .rhai, 31 operadores, 2.511 edges, cross-check programático 264-vías con **0 mismatches**, CI-enforced), 228 tests ejecutados verdes (math-engine 107 + prioritization-spine 121), y `MainnetRefused` de §34.3 presente (4 líneas grep-verificadas por el critic — flujo completo NO auditado).

---

## 2. DETAILED FINDINGS POR SUPERFICIE (solo CONFIRMED de pie)

### 2.1 searcher-rs — hot-path Rust + cartridges Rhai (5: 2 high, 3 medium)

- **F2 · HIGH** · SizeOptimizer: mapa de 5 direcciones hardcodeado; token_in no listado colapsa a "WETH" con decimales 18 → gross/net_usd, caps Kelly y Net_bps calculados a precio WETH (~1e8x de error para tokens ~1e-5 USD). `size_optimizer.rs:1739-1765, 1793-1801`. Mitigado aguas abajo por evaluator+REVM gate, pero las filas emitidas (cards, ranking, financing) llevan cifras falsas — RULE 00 violado en la capa económica.
- **F3 · HIGH** (condicional a flags `ARBX_NATIVE_ENGINES`/`ARBX_CARTRIDGE_MODE`) · Head-of-line blocking: `scanner.rs:1303-1367` consume el stream serialmente con `on_route_intent(...).await` inline; bajo burst, backpressure del relay pierde eventos. `with_slot0_cache` tiene 0 calleres — el early-reject 0-RPC está implementado pero muerto. Contradice doctrina §4.1/4.3.
- **F4 · MED** · ReservesCache in-memory sin evicción ni staleness (`triangular_engine.rs:97-107`): si pool_sync muere, el sizing corre contra reservas muertas indefinidamente; no existe razón `stale_reserves` en el kernel.
- **F5 · MED** · Kernel triangular ignora fee por pierna: `fee_bps: 30` hardcodeado (`size_optimizer.rs:723`); forks V2 con fee distinto quedan malpreciados en la dirección peligrosa (net sobrestimado).
- **F6 · MED** · `recon/anomaly.rs:55`: INSERT de risk_event severity=critical con resultado descartado (`let _ =`) — la anomalía auditable desaparece si PG falla.

### 2.2 math-engine — 31 operadores + prioritization-spine (2 high)

- **F11 · HIGH** · op_24 Nash: `evaluate()` ignora MarketState y resuelve un juego 2x2 con payoffs literales `[2,0,3,1]/[2,3,0,1]` — constante servida como evidencia de operador (RULE 00). `op_24_nash.rs:74-78`.
- **F13 · HIGH** · Cableado de producción sin enriquecimientos: los 3 constructores vivos (`orchestrator.rs:1202`, `cartridge_boot.rs:1614`, `scanner.rs:2104`) usan `with_cache` sin `with_relay_fee/with_p_fail/with_feedback/with_reserves` — relay bribe (C2, documentado como 10-50% del gross en mainnet) siempre 0.0. §34.4: cada evaluación ignoraría el sobrecosto principal.

### 2.3 contracts — Solidity (1 high)

- **F27 · HIGH** (condicional a despliegue; cero wiring verificado por grep) · MakerDssAdapter.dsrExit permissionless sin contabilidad per-user (`MakerDssAdapter.sol:98-103`): cualquiera puede drenar el DAI del pot a un recipient arbitrario; incluso el uso legítimo rompe la custodia (frontrun del exit).

### 2.4 frontend — Next.js (2 high)

- **F36 · HIGH** (latente, camino de código activo) · `useRuntimeAckSocket.ts:275-279`: fallback a `NEXT_PUBLIC_ARBX_ADMIN_TOKEN` en módulo "use client" — en el momento que un operador uncomment esa var (el .env.example lo invita), el token admin queda inline en el bundle público. Contradice el contrato V-AT-1 de `lib/admin-token.ts`.
- **F37 · HIGH** · `DexRegistryClient.tsx:29-39`: lee token admin legacy de localStorage y lo envía como header crudo en mutaciones admin — T1 secreto legible por XSS que el hardening supuestamente eliminó. Vivo en cada visita a /dex-registry.

### 2.5 edge worker + api-server (2 high)

- **F44 · HIGH** (condicional al camino real de headers) · Rate-limit 429 per-IP sobre headers forjables (`edge/worker/src/index.ts:312-319`) con puerto 8787 publicado en 0.0.0.0: bypass del bucket por request + inundación del keyspace rl:*; en dev-local, Map sin poda = memoria no acotada.
- **F45 · HIGH** · Middleware "sensura" global en dev-local reescribe cuerpos JSON en vuelo (`dev-local/src/index.ts:22-85, 218-219`): muta datos de API (p.ej. elimina la evidencia 'sandwich' que G-Mev-1 exige) — anti-RULE 00; en el worker el mismo filtro es código muerto (drift de contrato entre variantes; los e2e verdes contra dev-local no prueban el payload de prod).

### 2.6 docker/compose + CI + deploy (4 high)

- **F55 · HIGH** · `deploy.yml:32,37,46-47,131`: IP del VPS (195.201.235.70) + SSH root hardcodeados y versionados en repo publicado; fallback a SHA pineado obsoleto c815ef8c = rollback silencioso si el operador omite target_sha.
- **F56 · HIGH** · `deploy-vps.yml` y `deploy-blue-green.sh`: cero `--env-file` (violación directa R3/R4), healthcheck a `/health` cuando el contrato es `/api/health`, sin el lock flock canónico.
- **F57 · HIGH** · Rollback inefectivo en 2 de 3 rutas (`deploy.yml:246-259`, `deploy-efficient.sh:123-134, 315-327`): git reset + `up -d` SIN rebuild reutiliza las imágenes rotas recién construidas — falsa reversibilidad contra HARDENING Parte 5.
- **F58 · HIGH** · El auto-deploy (único camino automático a prod) construye frontend sin `--no-cache` ni detección de drift NEXT_PUBLIC_* (RULE 03); la lógica correcta ya existe en `deploy-efficient.sh:228-248` pero NINGÚN workflow la invoca; `verify-deploy.sh` (chequeo CSP R4) tampoco está cableado a ningún flujo.

### 2.7 PostgreSQL + Redis (1 high)

- **F69 · HIGH** · DLQ de relays-client es lógica muerta: `consumer.rs:146-158` lee solo con XREADGROUP `'>'`; el retry que el comentario promete ("redelivered on the next XAUTOCLAIM/XCLAIM cycle") no existe en el código — un fallo de PG varara la entrada en el PEL para siempre y el ledger pierde la fila en silencio.

### 2.8 monitoring + risk circuit breakers (1 high)

- **F81 · HIGH** (condicional a un paso de render en VPS no verificado) · Webhook dead-end: `alertmanager.yml:44-48` postea sin header de autorización a una ruta envuelta en `requireAdminToken` (`alertmanager-webhook.ts:48-50`) → toda alerta muere en 401, sin receptor externo activo (Slack/PagerDuty/Telegram comentados). El terminus de notificación de los 10 breakers doctrinales de esta rama descarta todo silenciosamente.

### 2.9 canon 264×31 (1 high)

- **F87 · HIGH** · Dos grafos rivales: la cadena canónica (Excel→math_map.json→rhai→capability_matrix→knowledge_graph, 0 mismatches, CI-enforced) vs `strategy_mapping.json` (stale 2026-07-20, divergente 264/264) cuyo derivado generado alimenta MathOperatorsTab y se autodeclara SSOT. El operador que consulta la UI ve el grafo NO canónico.

### 2.10 transversal — zero-mocks + fail-honest (1 high)

- **F93 · HIGH** · Pipeline paper estructuralmente muerto: productor nunca invocado Y esquema incompatible con el consumidor (100% de mensajes serían skip_incomplete a nivel info); además alias net-as-gross y gas-units-as-wei, con gas/Decoherencia ausentes coaccionados a `BigInt(0)` — si el esquema rico apareciera, inflaría el net hacia ACCEPT.

### Apéndice A — Findings REFUTED (4, con razón)

| ID | Sev. original | Razón de refutación (lente MATERIALIDAD) |
|---|---|---|
| **F92** | critical | El hardcode `$3500/ETH` (`executor.ts:131-134`) es textualmente real, pero el camino está **doble-muerto**: `ARBX_PAPER_EXECUTOR_MODE` no aparece en ningún compose/.env.example/workflow, y el productor (HotPathEmitter/emit_simulated) jamás se instancia → ninguna fila con USD fabricado se escribe hoy. Queda como olor RULE 00 latente; la muerte estructural misma es el hallazgo F93 (CONFIRMED). |
| **F1** | high | Los `as_u128()` panic-overflow existen, pero el call-site que mataría la detección (`orchestrator.rs:333`) está tras `#[cfg(feature="paper-shadow")]`, compilado FUERA de la imagen prod; el sitio sobreviviente corre en `tokio::spawn` aislado (DoS parcial contenido), condicional a un modo runtime no verificable. |
| **F14** | high | La mezcla USD-vs-wei es real en la API, pero la rama solo ejecuta si `v2_reserve_snapshot` es `Some` y NINGÚN caller de producción pasa `with_reserves` (solo tests). Bug de contrato sin cablear; no puede ocurrir en código que corre hoy. |
| **F15** | high | Las reservas V2 llegan de `getReserves` cuyo tipo es uint112 (max ~5.2e33 << u128::MAX) — el escenario "reservas >2^128" es inalcanzable con datos reales; además la hidratación parsea a u128 antes de almacenar. Queda el mismatch comentario-vs-implementación como nit de calidad. |

### Listado honesto de los 79 UNVERIFIED (hipótesis con evidencia file:line, sin ronda de lentes — no son hechos)

**Medium (37)**: F7 doc-drift route_discovery NO-ACTIVE vs dispatch Active · F12 op_24 etiquetas p/q intercambiadas · F16 knob slippage 100x laxo (fracción vs pct) · F17 op_01 SVD sin guards (panic vía API) · F18 BayesianAllocator clamp vs 1/2-Kelly (2x agresivo) · F19 op_19 Simplex LP unidades incoherentes · F28 callback Balancer ejecución arbitraria no solicitada (explotabilidad INFERIDA) · F29 AaveV3 withdraw permissionless · F30 FlashLoanExecutor sin sweep de fondos · F31 DeployTestnet/Multichain sin gates mainnet · F38 ~18 fetches SSR sin timeout · F39 guard R2 con bypasses no doctrinales · F40 CSP Report-Only nunca flipado · F46 /hot/v1 glob literal devuelve 200 vacío (fabricación de vacío) · F47 default NEXT_PUBLIC_WS_URL viola RULE 02 · F48 /admin/session bucket 'anon' compartido (lockout DoS) · F49 tokens default dev evaden FORBIDDEN_TOKEN_VALUES · F59 prod sin depends_on postgres healthy · F60 healthcheck Vault `\|\| true` · F61 lock TOCTOU en deploy-efficient · F62 rollback plan referencia workflow inexistente · F63 G2/G6 sin mecanizar · F68 verify-deploy L4 sin cablear · F70 PEL sistémico sin redespacho (selector/recon/archivers) · F71 streams hot con consumidores sin productor · F72 run_migrations re-granta ALL + resetea passwords dev · F73 FK SET NULL sobre NOT NULL · F74 pool sin statement_timeout · F75 RDO sink INSERT fila-por-fila · F82 latency breaker sin input de latencia real · F83 inhibición silencia EvalStale con estado congelado · F84 sin promtool test del grupo circuit_breakers · F88 "8.184 relaciones" son celdas de grilla (reales: 1.716; 8 ops huérfanos) · F89 relaciones Strategy→Operator observe-only en runtime · F94 GoPlus outage tragado · F95 sed-core connect() Ok sobre stream eternamente vacío · F96 logos rate-limited perdidos permanentes.

**Low (42)**: F8 unwrap_or_default en emisor hot · F9 round-trips String en hot-loop · F10 comparación lexicográfica de direcciones · F20 covarianza sobre niveles no retornos · F21 gas incoherente 21k/0 entre ops · F22 op_30 GNN procedencia FUSILE falsa · F23 gates tautológicos (contracts_verified=true) · F24 5 campos de riesgo constantes · F25 whitelist 'PASS' legacy bypassea gate sim · F26 20/31 ops solo smoke test · F32 adaptadores BUSL sin SafeERC20 · F33 limit GIVEN_IN invertido · F34 maxFlashLoan sobredeclara · F35 CREATE2 permissionless · F41 suppressHydrationWarning fuera de span · F42 código realtime muerto con claims stale · F43 KPI home coacciona null a 0 · F50 pathRewrite desviación R4 · F51 drift 500-vs-502 entre variantes · F52 errores tragados sin log server-side · F53 defaults sentinel + /api/metrics campos fabricados · F54 WS handshake docstring contradice implementación + token por query · F64 Grafana password fallback + secrets muertos · F65 bypasses R2 no documentados · F66 anvil :latest sin pinear · F67 POSITIVO: G4 implementado (apéndice obsoleto) · F76 relays persiste 0.0=uncomputed · F77 tres escritores paper_trade_runs sin idempotencia · F78 drift documental RDO 14d vs 2d · F79 trigger UPDATE notifica siempre · F80 N+1 tokenValidations · F85 UNKNOWN for:0m flap pages · F86 familia arbx_risk_cb_* sin Grafana · F90 'Legend' infla 265 vs 264 · F91 31/31 UNCALIBRATED no propagado · F97 multicall decode failures sin razón · F98 upserts failure-path `let _ =` · F99 /score defaultea safety_score 50 · F100 lag=0 en fallo XPENDING · F101 /health entropy parcial fabricada · F102 g-pipe-1 lag negativo → falso GREEN · F103 V3-quote cache write descartado.

---

## 3. CONFIDENCE ASSESSMENT

| Superficie | Confianza | Por qué |
|---|---|---|
| searcher-rs | **Media** | Núcleo hot-path leído línea a línea (main, orchestrator, scanner por secciones, size_optimizer completo); PERO 11/16 engines sin leer, triangular_worker 3.805 líneas grep-only, sin compilación (AppControl). |
| math-engine + spine | **Alta (estática)** | 31 operadores + tests leídos completos (5.192 líneas); 228 tests EJECUTADOS verdes (107+121); hueco declarado: execute_arbitrage_encoder.rs (698 líneas de calldata de broadcast) sin leer. |
| contracts | **Media** | 28 .sol leídos completos, versiones OZ verificadas; sin forge build/test (read-only), estado on-chain desconocido — severidades condicionales a despliegue. |
| frontend | **Media-Alta** | Páginas flagship + barridos de patrones completos (R1/RULE 02 verificados por superficie); sin lint/tsc, bundle no inspeccionado, ~20 rutas sin page-by-page. |
| edge + api-server | **Media** | Ambas variantes leídas completas en entrypoints; ~60 rutas spot-check, audit-emit.ts no abierto, sin vitest, runtime inverificable. |
| docker/CI/deploy | **Media-Alta** | Archivos canónicos línea a línea + greps exhaustivos (0 secretos reales en working tree); solo ~8/50 workflows leídos; branch-protection (G1) inaccesible desde disco. |
| PG + Redis | **Media** | Esquema/retención/streams con evidencia file:line; migraciones leídas parcialmente (~40 de 118 a fondo); TODAS las cifras de escala son citadas de headers/docs, no medidas. |
| monitoring/breakers | **Alta (en la rama)** | Todos los archivos de la rama leídos, inventario de 65 tests verificado por lectura, bijección enum-verificada; promtool NO ejecutado. |
| canon 264×31 | **Alta (dimensión cuantitativa)** | Cross-check programático 264-vías con 0 mismatches — el claim de cobertura más fuerte de la auditoría; fórmula-por-fórmula, world/ y XLSX fuente pendientes. |
| transversal | **Media** | Lecturas dirigidas + greps + contra-evidencia positiva incluida; recon/sim internals solo grep-swept. |

**Global**: confianza ALTA en los hechos confirmados (consenso 3/3 + materialidad + file:line abierta). Confianza LIMITADA en cualquier veredicto de sistema completo: 4 superficies con cero cobertura dedicada, 79 hipótesis sin verificar y 0% de verificación dinámica. Ninguna afirmación de "auditoría completa" es sostenible sobre esta base.

---

## 4. RISK-ADJUSTED CONCLUSION (pregunta canónica §34.4)

**¿Esto funcionaría correctamente con capital real en LIVE_MAINNET? — NO. Y además, con esta base de evidencia, NO ES CERTIFICABLE.**

**Por qué NO, sobre hechos confirmados solamente (día 1 de capital real):**

1. **El filtro económico aprobaría rutas inviables de forma sistemática.** F13 (relay bribe = 0 en todos los evaluadores de producción; el propio engine cuantifica la sobreestimación en 10-50% del gross) + F2 (tokens no listados tasados como WETH) + F5 (fee hardcodeado) = Topological Yield neto sesgado al alza en la capa que decide. La simulación REVM aguas abajo mitiga la ejecución, no la decisión ni el ledger.
2. **El operador estaría ciego**: F81 (toda alerta muere en 401, incluidos los 10 breakers doctrinales) + F69 (mensajes del terminus varados en PEL para siempre) + F93 (el loop de evidencia paper que debería calibrar el sistema está muerto estructuralmente). El sistema de riesgo existe, emite correctamente a Prometheus, y nadie recibe nada.
3. **El perímetro es evitable con un curl** (F44) y el plano de despliegue que llevaría el flip a producción tiene IP+root versionados, rollbacks falsos y verificación L4 desconectada (F55-F58).
4. **Custodia**: F27 confirma drenaje permissionless en un adapter del repo (condicional a despliegue); F30 (unverified pero verificado estructuralmente: el archivo no tiene sweep) implica que el primer día live terminaría en un upgrade de emergencia para acceder al rendimiento.

**Balance explícito — lo que hoy protege al sistema:**
- El sistema corre en PAPER_SHADOW: capital expuesto = 0, sin broadcast. Varios de los defectos confirmados son latentes, no activos (F36 no fuga mientras la var esté unset; F2/F5 contaminan datos, no fondos).
- §34.3 está físicamente presente: `live_exec_policy` se rehúsa a mainnet y es default-deny (4 líneas grep-verificadas; el flujo completo NO fue auditado).
- La auditoría encontró **contra-evidencia positiva real**: sin sentinels ilegítimos, oráculos fail-closed, stubs honestos, canon cuantitativo coherente con enforcement CI, y 228 tests verdes en el núcleo matemático.

**La ironía estructural que decide la respuesta**: las 4 refutaciones demuestran que los defectos más vistosos están doble-muertos por wiring ausente. La seguridad actual es, en parte, una **ausencia de cableado**. El flip a LIVE_MAINNET es precisamente el acto que conecta esos planos: activaría la sobreestimación del 10-50% (F13), expondría el token admin (F36), haría explotable el bypass perimetral (F44) y dependería de una cadena de despliegue que no verifica lo que despliega (F58). Bajo §34.4, un sistema cuya defensa es "el código peligroso aún no está conectado" no pasa la pregunta canónica.

**Incertidumbre incorporada (no maquillada)**: (a) el terminus de ejecución — la superficie MÁS safety-critical de §34 — tiene cero auditoría dedicada: bajo R8, lo no verificado no se certifica, ni a favor ni en contra; (b) las 79 hipótesis pendientes incluyen elementos que empeorarían el cuadro (F29 withdraw permissionless, F48 lockout DoS del login admin, F16 knob de slippage decorativo) y otros que se disiparían; (c) nada fuera de math-engine/prioritization-spine fue compilado ni testeado — ni siquiera está establecido que el árbol auditado compila. El "NO" es de alta confianza en el plano de control/supervisión/economía (hechos confirmados) y de imposibilidad de certificación en el plano de ejecución (gap).

---

## 5. DATA GAPS (todo lo NO verificado, en claro)

**A. Superficies de auditores muertos (cero cobertura dedicada):**
1. **relays-client / terminus §34** (20 archivos: signer.rs, submit_engine.rs, bundle_builder.rs, nonce_manager.rs, tracker.rs, multi_relay.rs, relay_{flashbots,bloxroute,titan,no_submit_sim}.rs, relay_catalog.rs, executor/, flujo completo de live_exec_policy). Solo 4 líneas de MainnetRefused grep-verificadas por el critic + hallazgos incidentales de otros auditores (F69 confirmado; F76/F77 unverified).
2. **Familia de simulación REVM**: simulator-v2 (5) + sim-core (4) + sim-ctl (13) + mcp-sim-engine (1) = 23 archivos, cero hallazgos dedicados. Doctrina G-SIM-1, fidelidad del fork, pipeline S4 de labels: inauditados.
3. **docs-drift sistemático**: no existe matriz doctrina→código (CLAUDE.md §1-37, EXECUTION_MODES_DOCTRINE, ROUTES_CROWN_JEWEL, HARDENING G1-G6, docs/incidents/*).
4. **Performance end-to-end**: sin bench, sin latencia medida, sin perfil de allocaciones; F3/F9 son hallazgos estáticos puntuales, no un pase.

**B. Modalidades dinámicas — 0% ejecutado:**
- **Runtime/VPS**: flags ARBX_*_MODE reales, XLEN/XINFO/PEL vivos, /metrics//health, paridad git rev-parse HEAD deployado vs árbol, curl -I CSP post-build.
- **Build/test**: solo `cargo test --lib` de math-engine (107 pass) y prioritization-spine (121 pass). Sin check/clippy/fmt del workspace, sin vitest, sin forge build/test, sin promtool (ausente en el host), sin lint/tsc del frontend. No está verificado que el árbol auditado compile.
- **E2E browser+WS**: cero.
- **On-chain read-only**: cero — no se verificó si MakerDssAdapter/AaveV3CrossChainAdapter existen desplegados (condiciona F27/F29); deployments CREATE2 desconocidos.
- **Bundle/dist**: JS construido no inspeccionado (F36 es evidencia de fuente); stale dist no descartado en edge/api-server.
- **Git-history**: scan de secretos solo sobre working tree; deploy.yml probó que hay infra versionada — historia sin gitleaks/trufflehog.
- **HG workbook**: la certificación 10 gates (0/10 PASS, 2026-09-06, memoria) no fue re-verificada contra el árbol.

**C. Condicionales abiertas sobre findings CONFIRMED**: F3 (↔ flags nativos del VPS), F44 (↔ camino real de headers cloudflared/nginx), F81 (↔ posible paso de render que inyecte auth en el VPS), F36 (↔ bundle construido), F27 (↔ inexistencia de despliegue on-chain).

**D. Restos por superficie (declarados por cada auditor)**: searcher-rs — 11 engines sin leer, triangular_worker (3.805 líneas), calldata decoders, block_scanner, state_projector internals, persistence/dao, publisher, scoring/kelly/bayesian, thermodynamics/, sed_engine/, cartridge host_bindings/subscriber/loader. math-engine — execute_arbitrage_encoder.rs (698 líneas, calldata del broadcast), api.rs, matrix/, control/, strategies/, subgraph_client.rs. contracts — lib/ solo pinning de versiones, 17 archivos de test no leídos completos, integración backend de ABIs. frontend — ~20 rutas sin page-by-page, suites e2e/playwright, árboles app_backup//_recovery/ (claim "no compilados" supuesto), ~40 sitios setInterval solo por patrón. edge — ~60 rutas spot-check, audit-emit.ts, ALLOWED_ASYMMETRIES ruta a ruta. docker/CI — ~42 workflows no leídos (rust, typescript, foundry, security, no-hardcode, ethics-guard, sim-*), socket-proxy server.js, scripts/arbx-env-deploy/. PG/Redis — migraciones 011-049/061-096, diff compose prod↔dev, inventario TTL del keyspace no-stream, PEL vivos por grupo. monitoring — loki/promtail/vault/thanos, paneles Grafana solo grep, consumo frontend de /api/v1/risk/circuit-breakers no trazado. canon — world/ vs código, fórmula-por-fórmula de los 31 ops, 250 SKILL.md, XLSX fuente (requiere operador). transversal — recon/simulator/shared-rs solo grep-swept, sed-core 33/39 archivos, relays-client fuera de charter.

**E. Meta**: deriva de anclaje 086347f7 → f46a0522 → HEAD final pendiente; los conteos (264/31/2.511/103) y todos los file:line necesitan re-anclaje. Cifras de rendimiento (90.7s, 76M filas, 899MB, ~1.3M filas/h, 48.4K/24h) son evidencia documental citada, NO medición de esta sesión. Los "14 required checks" y el run #545 post-merge viven en GitHub y no fueron verificados.

---

*Informe de auditoría read-only: nada fue editado, commiteado, desplegado, firmado ni activado. Sin recomendaciones de trading. El sistema premia la honestidad R8: 20 hechos, 4 refutaciones, 79 hipótesis y los gaps declarados son el producto real de esta pasada.*