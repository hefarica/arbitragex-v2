# WO-02 — VERIFY (adversarial, post-apply)

- **Work-order:** WO-02 · **Tipo:** VERIFY adversarial (Oleada 4, serie rust — corre TRAS el apply)
- **Agente:** ecc:rust-reviewer (Gang Omniscience, PhD verify lane)
- **Charter:** verificar el WO-02 aplicado contra `WO-02-DESIGN.md` §5.1/§5.2 + §7 gates: (1) semántica hot-stream, (2) hot-path discipline §4/R9, (3) fail-honest R8, (4) re-ejecución check+clippy+fmt+test con EXIT codes, (5) terminus §34.3/VPS intactos.
- **Reglas respetadas:** 0 requests HTTP al dominio público (todo local), cero git/commit/push, cero ssh/VPS, cero Redis/PG. Read-only + cargo local.
- **Fecha:** 2026-09-06.

---

## VEREDICTO: **APPROVE** (0 defectos CRITICAL, 0 MAJOR · 5 hallazgos INFO, ninguno bloqueante)

El wiring `emit_simulated` está implementado EXACTAMENTE como el diseño §5.1/§5.2 lo especifica: post-simulación REVM real, inmediatamente después del publish canónico, fail-soft, veredicto verbatim. `emit_detected` y `emit_gate_commit_from_state` siguen SIN call-sites (decisión §0 del diseño, respetada). Los 4 gates re-ejecutados por este verify están **TODOS en verde** — incluidos clippy `-D warnings` y fmt, que en el snapshot del apply fallaban por el huérfano WO-10 y ya fueron cerrados por su applier respawneado (verificado independiente en esta corrida).

---

## 1. Matriz de evidencia — charter (1) semántica del stream hot

| # | Aserción del diseño | Veredicto | Evidencia (file:line) |
|---|---|---|---|
| 1.1 | Emisión POST-publish canónico; si `publish` falla (`?`) NO se emite | ✅ | `scanner.rs:2642` (`publisher::publish(redis, &opportunity).await?`) → `scanner.rs:2645-2663` (bloque WO-02). El `?` propaga antes del `if let Some(sim)` |
| 1.2 | Orden interno del emisor: XADD stream primero, HSET+EXPIRE sólo en passed | ✅ | `hot_path_emitter.rs:153-180` (XADD `arbx:hot:simulated`) → `:183-199` (HSET `arbx:hot:sim:{id}` + EXPIRE 300, dentro de `if result.passed`) |
| 1.3 | Emite SOLO cuando corrió sim REVM real (pre-REVM → `None`, jamás emitido como failed) | ✅ | Pre-REVM `None`: `scanner.rs:2423-2429` (else sim-disabled), `:2977-2983` (missing_executor), `:3045-3052` (spawn_blocking_failed). Post-REVM `Some`: `:3092-3099` (wrapped_calldata_missing), `:3151-3157` (net_usd_rejected), `:3173-3179` (SIM_SUCCESS), `:3206-3212` (failed-tail). Captura única: `:3055-3058` (`let hot_sim = hot_sim_record(&outcome);` tras el match de `spawn_blocking`) |
| 1.4 | MAXLEN ~5000 de `arbx:hot:simulated` intacto | ✅ | `hot_path_emitter.rs:155-157` (`MAXLEN ~ 5000` idéntico al pre-diff) |
| 1.5 | Invariantes del publisher intactos (canal canónico NO tocado por WO-02) | ✅ | `publisher.rs:41-42` (`STREAM_KEY = "arbx:opps:detected"`, `STREAM_MAXLEN = 10_000`), `:162-165` (XADD MAXLEN ~ 10000 en `publish()` sin cambios semánticos — el diff del archivo es 100% aditivo WO-10: +214/−0, sólo histograms de latencia con observe zero-alloc en éxito). `git diff HEAD --stat -- backend/relays-client` = vacío |
| 1.6 | `emit_detected` (MAXLEN ~10000) y `emit_gate_commit_from_state` intactos y SIN cablear | ✅ | `hot_path_emitter.rs:71-115` y `:208-241` sin cambios funcionales; grep repo backend: 0 call-sites (solo definiciones + un comentario `orchestrator.rs:1220`). Único call-site de `emit_simulated`/`HotPathEmitter::new` en todo el repo: `scanner.rs:2654-2655` |
| 1.7 | Sin doble emisión: sitio único en la cola de la función, tras dedup (early-return) e insert PG | ✅ | dedup `scanner.rs:2609-2621` (return antes de publish → candidato dedupeado NO emite hot, diseño §3.4); insert PG fail-soft `:2627-2641`; emisión `:2653` una sola vez (sin loop, sin match multi-brazo) |
| 1.8 | Public sites "legacy" previos (L2043/2336/…) NO emiten hot — correcto: `hot_sim` no existe en esos brazos (retornan antes del sim gate) | ✅ | Brazos legacy con `return Ok(())`: `scanner.rs:2052`, `:2346` (y pares); el gate de sim (`scanner.rs:2407`) se alcanza solo si ningún brazo retornó → sin emisión hot para oportunidades nunca simuladas |
| 1.9 | Path V2 (`OrchestratorMode::V2`) retorna antes del path legacy → stream honestamente vacío en modo V2 (riesgo declarado §8.3) | ✅ | `scanner.rs:1583-1586` (`if orch_mode == OrchestratorMode::V2 { return Ok(()); }`) — pre-existente, no una rama de WO-02 |
| 1.10 | Defecto latente #2 cerrado: XADD lleva `opportunity_id`+`chain_id`+`strategy_kind`+`token_pair` (PaperExecutor ya no haría `skip_incomplete`) | ✅ | `hot_path_emitter.rs:167-176` (5 campos nuevos entre `gas_used` y `timestamp_ms`), firma `opp: &Opportunity` `:139-143` |

## 2. Matriz de evidencia — charter (2) hot-path discipline (§4 C-S-E, R9/LOGFLOOD-01)

| # | Aserción | Veredicto | Evidencia |
|---|---|---|---|
| 2.1 | Cero allocs innecesarias en el nuevo camino | ✅ | Caso `hot_sim=None` → costo CERO (el `if let` no ejecuta nada, `scanner.rs:2653`). Caso Some: las únicas allocs son la serialización del wire contract que el diseño costea expresamente (§4: "1 XADD por sim post-publish"): `U256::to_string()` ×2 en `hot_sim_record` (`scanner.rs:3225,3227`, capturadas UNA vez en `:3058`, no por return), `opp.id.to_string()` (`hot_path_emitter.rs:150`), build del cmd XADD (inherente a cualquier op Redis), y HSET `format!`+serde SOLO en passed (`:184-185`). `redis.clone()` es un handle-clone barato de `ConnectionManager` (patrón documentado del módulo, `:44-47`). Sin `format!` de log en éxito, sin String de métricas |
| 2.2 | Logging por-ítem a debug! + summary info! (R9) | ✅ (silencio-en-éxito, la forma más estricta) | El camino nuevo NO loguea por-ítem en éxito (cero líneas de log por emisión). Único log: `warn!` de fallo con event-tag `hot_path.simulated_emit_failed` (`scanner.rs:2656-2661`) — alcanzable solo en la ventana estrecha publish-OK + XADD-hot-fail (si Redis cayera, el publish canónico `:2642` ya habría abortado el pipeline). R9 no exige summary info! cuando no hay logs por-ítem que agregar |
| 2.3 | Interacción con WO-10: el span `decode_to_publish_legacy` NO queda contaminado por la emisión hot | ✅ (positivo) | `scanner.rs:2643` (observe WO-10) ejecuta ANTES del bloque WO-02 `:2645` → el histograma mide tx→publish exactamente, sin incluir la latencia del emisor auxiliar |

## 3. Matriz de evidencia — charter (3) fail-honest R8 / RULE 00

| # | Aserción | Veredicto | Evidencia |
|---|---|---|---|
| 3.1 | Cero datos fabricados: cada campo del XADD traza a fuente real | ✅ | `id`/`opportunity_id` ← `opp.id` (Uuid); `chain_id` ← `opp.chain_id`; `strategy_kind` ← `opp.strategy_kind.as_str()`; `token_pair` ← `opp.pair_symbol` (`hot_path_emitter.rs:159-176`); `net_profit_wei` ← `outcome.simulated_profit_token_in.to_string()`; `gas_used` ← `outcome.gas_used_total`; `gas_price_wei` ← `outcome.gas_price_wei.to_string()` (`scanner.rs:3223-3227`). Tipos fuente verificados: `round_trip_executor.rs:63-72` (`passed: bool` :64, `simulated_profit_token_in: U256` :65, `gas_used_total: u64` :67, `gas_price_wei: U256` :72). Cero literales de operador |
| 3.2 | `"0"` wei = REVM zero REAL, no inventado | ✅ | `SimulationOutcome::failed()` zeroa la economía (`round_trip_executor.rs:85-93`: `simulated_profit_token_in: U256::zero()`, `gas_used_total: 0`, `gas_price_wei: U256::zero()`); el test lo clava (`scanner.rs:3338-3343`: asserts `"0"`/`"0"`/`0`) |
| 3.3 | U256 completo preservado (defecto latente #1: truncamiento u128) | ✅ | Campos String (`hot_path_emitter.rs:39,41`); test con `simulated_profit_token_in = 2·u128::MAX` (`scanner.rs:3345-3350`) compara contra el `to_string()` fuente → precisión completa probada |
| 3.4 | Defecto latente #3 cerrado: emitter consume `ConnectionManager` (el handle que el scanner hila) | ✅ | `hot_path_emitter.rs:18-21,50,56-58`; call-site `scanner.rs:2654` (`redis.clone()` sobre el mismo handle de `publish`, `:2642`) |
| 3.5 | Fail-soft asimétrico observable | ✅ | `scanner.rs:2656-2661` — warn con `event`/`opp_id`/`error`, pipeline continúa (el canónico ya éxito). Mismo patrón sancionado de `validated_plan.persist_failed` (`scanner.rs:2459-2465`) |
| 3.6 | `status` = veredicto REVM VERBATIM, emitter jamás re-clasifica | ✅ | Única fuente `outcome.passed` (`scanner.rs:3224`) → `if result.passed {"passed"} else {"failed"}` (`hot_path_emitter.rs:149`). Ver hallazgo INFO-1 |
| 3.7 | Test propio presente y PASS | ✅ | `scanner.rs:3331` `hot_sim_record_maps_outcome_verbatim` — salida de esta corrida: `test scanner::tests::hot_sim_record_maps_outcome_verbatim ... ok` (línea 1910 de la transcripción de test) |
| 3.8 | Config del cfg: helper + test bajo `#[cfg(feature = "v2-simulator")]`, feature default ON | ✅ | `scanner.rs:3219` (helper), `:3330` (test); `Cargo.toml:71` `default = ["v2-simulator"]` → compila y corre en el build por defecto |

## 4. Charter (4) — gates re-ejecutados (esta verify, independiente del apply)

Ejecutados desde `backend/` con `target/` caliente, cargo 1.91.0, PATH `$USERPROFILE/.cargo/bin`. Salida capturada a archivo para descartar truncado.

| Gate | Comando | EXIT | Notas |
|---|---|---|---|
| 1 | `cargo check -p searcher-rs --quiet` | **0** | silencioso, 0 warnings |
| 2 | `cargo clippy -p searcher-rs --quiet -- -D warnings` | **0** | archivo de salida 0 bytes — cero diagnósticos en TODO el crate. **Mejor que el snapshot del apply** (que reportaba FAIL 101 por 10 `doc_overindented_list_items` en `publisher.rs` del huérfano WO-10): el applier WO-10 respawneado ya los cerró. Mis 2 archivos claim: 0 diagnósticos |
| 3 | `cargo fmt -p searcher-rs -- --check` | **0** | archivo de salida 0 bytes — 0 diffs en el crate (el apply reportaba 1 diff en `opportunity_emitter.rs:331`, WO-10, ya cerrado) |
| 4 | `cargo test -p searcher-rs` | **0** | **2398 passed · 0 failed · 17 ignored** (15 targets + doc-tests; incluye `hot_sim_record_maps_outcome_verbatim` PASS). En esta corrida NO se reprodujo el bloqueo AppControl 4551 que el apply sorteó con exe-directo — `cartridge_syntax_validate` y `orchestrator_parallel_run` ejecutaron vía cargo y pasaron |

Tally agregado por awk sobre las 16 secciones `test result:` de la transcripción: `passed=2398 failed=0 ignored=17`. El delta de ignored vs el apply (16) es la sección doc-tests (0 passed · 1 ignored) que el apply no contaba en su tabla de 15 targets — sin impacto.

## 5. Charter (5) — terminus §34.3 y VPS intactos

| Verificación | Resultado | Evidencia |
|---|---|---|
| `backend/relays-client` SIN diff (default-deny / `MainnetRefused` intactos) | ✅ | `git diff HEAD --stat -- backend/relays-client` → vacío |
| CERO commit/push (NO-GIT 2026-08-23) | ✅ | `git log --oneline -1` = `f7db6867` (HEAD idéntico al inicio de sesión; todos los cambios del WO viven solo en working tree) |
| Cero ssh/VPS/Redis/PG writes | ✅ | Esta verify ejecutó solo lecturas de archivos + cargo local. §33.3 `XLEN arbx:opps:detected` delta=0 trivialmente cierto: nada fue deployado |
| Presupuesto dominio público | ✅ 0/5 usados | N/A pre-deploy: los invariantes INV-1..INV-6 del diseño §6 son post-deploy (ventana ≥30 min, Redis RO) = alcance OPERADOR. Nada hay en producción que probar en browser — el wiring local no deployado |

## 6. Hallazgos (ninguno bloqueante)

- **INFO-1 — `status="passed"` para candidatos REVM-aprobados pero rechazados por el gate net-USD.** Un outcome con `passed=true` que cae en `net_usd_rejected` (`scanner.rs:3151-3157`) emite al hot stream `status="passed"` con la economía GROSS real, mientras `sim_status_str` dice `SIM_DISABLED_FAIL_CLOSED`. Es la decisión explícita del diseño §3 ("veredicto REVM VERBATIM… el emitter jamás re-clasifica; el PaperExecutor aplica su propio gate net y registra REJECTED con razón") y está documentada en `hot_path_emitter.rs:130-131`. Downstream es consistente (paper-executor gate propio, doctrina "nunca re-etiquetar", memoria R-0001). Se lista para que lectores futuros del stream no confundan "REVM passed" con "SIM_SUCCESS".
- **INFO-2 — nombre `net_profit_wei` transporta GROSS.** El nombre del campo es legacy del contrato pre-WO-02; el contenido es `simulated_profit_token_in` (gross token_in delta). Documentado con justificación upstream en `hot_path_emitter.rs:33-35` + `round_trip_executor.rs:55-61` (el sim es prices-free; el net-of-gas es decisión downstream). Renombrar rompería el contrato wire aditivo — correcto dejarlo.
- **INFO-3 — `serde_json::to_string(result).unwrap_or_default()` retenido en el HSET passed-only** (`hot_path_emitter.rs:185`). Pre-existente; el diseño §5.1 mandaba dejar el HSET/EXPIRE igual. Para un struct de `bool/String/u64` la serialización no puede fallar en la práctica y el hash no tiene lectores en prod. No extendido por WO-02.
- **INFO-4 — discrepancia snapshot apply vs verify en clippy/fmt:** el apply reportaba FAIL 101/FAIL 1 (huérfano WO-10); esta verify independiente encuentra ambos EXIT=0 porque el WO-10 respawneado cerró sus residuos entre ambas corridas. El reporte del apply era fail-honest para SU instante; el estado actual del árbol es el que reporto.
- **INFO-5 — residuo declarado §5.3:** `docs/redis-schema/hot-path-v2.md` sigue sin la sección actualizada de `arbx:hot:simulated` (fuera del claim del apply, dueño = operador/WO de docs). No afecta al wiring; el contrato vivo está documentado en el doc-comment del emisor (`hot_path_emitter.rs:127-138`).

## 7. Desviaciones contra el diseño (evaluadas)

| Desviación | Evaluación |
|---|---|
| `crate::hot_path_emitter::` (diseño) → `searcher_rs::hot_path_emitter::` (código, `scanner.rs:2654,2964,3220,3222-3223`) | **CORRECTA.** `scanner.rs` es módulo del BIN (`main.rs:132 mod scanner;`) y `hot_path_emitter` vive en la LIB (`lib.rs:125`); `crate::` no resolvería desde el bin. El diseño era read-only y nunca compilado (lo admite §9). Compila EXIT=0 |
| Doc-comment de `dispatch_orchestrator_and_classify` con 2 completaciones extra (primer párrafo stale "triple" + antecedente "It carries") | **CORRECTA.** Ambas dentro del bloque que el propio hunk tocaba; sin reformateo ajeno (§37 P-∅ respetado) |

## 8. Conclusión para el board

**WO-02 = APPROVED (verify adversarial).** El productor de `arbx:hot:simulated` está cableado con la semántica exacta del diseño (post-REVM, post-publish, fail-soft, verbatim, sin flood de detección cruda), con los 3 defectos latentes cerrados y probados por test. Gates 4/4 verdes en corrida independiente. Pendientes que NO son de este WO: §5.3 docs (operador), §5.4 companion (operador), deploy + INV-1..INV-6 (operador, post-deploy Redis RO). El stream en VPS seguirá XLEN=0 hasta que el operador deploye — ese es el estado honesto esperado, no un defecto del wiring.

---

*WO-02 VERIFY — 2026-09-06 · ecc:rust-reviewer. Fail-honest: cada afirmación lleva file:line verificado por lectura directa o ejecución local; 0 requests HTTP; 0 escrituras VPS/Redis/PG; 0 git.*
