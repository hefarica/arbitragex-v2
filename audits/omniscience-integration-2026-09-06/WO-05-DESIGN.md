# WO-05 — DISEÑO: código fantasma en ruta de capital (relays-client)

- **Work-order:** WO-05 (Oleada 3 — diseño READ-ONLY; aplicación en Oleada 4 por applier Rust).
- **Anomalía fuente:** informe §2.5 — "mod executor (~350 líneas nunca compiladas) · NonceManager::refresh sin call-sites · Eden/Beaver 0 líneas".
- **Superficie auditada:** backend/relays-client/src/ completo (21 archivos .rs); leídos: main.rs, executor/ (4 archivos completos), nonce_manager.rs, submit_engine.rs (tramos del load-path), bundle_builder.rs (tramo build_and_sign), tracker.rs, relay_catalog.rs; live_exec_policy cubierto por el hermano N6.
- **Modo:** READ-ONLY verificado — CERO ediciones a código, CERO git mutante (solo log/show/grep de lectura).
- **Estado:** DESIGNED (diseño completo; diffs propuestos listos para Oleada 4).

---

## 0. Resumen ejecutivo

| Hallazgo §2.5 | Veredicto | Acción diseñada |
|---|---|---|
| (a) src/executor/ ~350 líneas nunca compiladas | CONFIRMADO: nació muerto (2026-07-14/15), la declaración "mod executor;" JAMÁS existió en main.rs, 4 bloqueantes de compilación internos, 0 referencias de código en el repo | **ELIMINAR** (git rm 4 archivos, -350 líneas; historia preservada en git) |
| (b) NonceManager::refresh sin call-sites | CONFIRMADO: solo la definición (nonce_manager.rs:56) con #[allow(dead_code)]; es un gap REAL de wiring — fuga de nonce huérfano en 5 puntos de la ruta LIVE | **INTEGRAR (wiring)**: helper resync_nonce + 5 call-sites en submit_engine.rs |
| (c) Eden/Beaver 0 líneas | CONFIRMADO: 0 integraciones; todas las menciones grep son falsos positivos (pr-eden-ce, cr-eden-tial) | **NO escribir código** (YAGNI §2 + P-∅); ítem de onboarding del OPERADOR (fila en catálogo DB relays) |

La decisión NO toca live_exec_policy.rs ni el default-deny (§34.3 intocable).

---

## 1. (a) Análisis de backend/relays-client/src/executor/ — inventario forense

### 1.1 Inventario (350 líneas exactas — wc -l)

| Archivo | Líneas | Contenido | Estado si se compilara |
|---|---|---|---|
| mod.rs | 200 | LiveTestnetExecutor + ExecutionOpportunity + PendingStatus/ExecutionState (máquina de 16 estados) + ExecutionError + ExecutorConfig + ExecutionReceipt | **NO COMPILA** (2 bloqueantes, ver 1.3) |
| nonce_manager.rs | 64 | NonceManager alternativo con lease acquire/release + TTL 300s | Compila como módulo huérfano; semánticamente REGRESIVA (ver 1.4) |
| gas_oracle.rs | 44 | GasOracle: base_fee del bloque latest + priority 2 gwei hardcodeado + max_fee = 2*base + priority | **NO COMPILA** (imports de crates no declaradas) |
| idempotency.rs | 42 | IdempotencyChecker: Redis SET NX EX 86400 por plan_hash | Compila como módulo huérfano; PERO mod.rs la llama con otra firma |

### 1.2 Historia git (verificada, read-only)

```
efa8d81e 2026-07-14 fix(ci): +122 (gas_oracle 29, idempotency 29, nonce_manager 64)
299b8ac2 2026-07-15 feat(live-testnet): +128 (mod.rs)
aa28fee4 2026-07-15 feat(live-testnet-v2): +72 (mod.rs, "executor stub")
d03e1056 2026-08-08 style: rustfmt (lo formateó muerto — fmt opera por glob, no por árbol de módulos)
```

- git log --all -S "mod executor;" sobre main.rs devuelve VACÍO: la declaración **nunca existió**, ni siquiera en el commit que introdujo el módulo (299b8ac2 ya muestra 13 declaraciones mod sin executor).
- Origen: ola "live-testnet" de 2026-07-14 (spec docs/superpowers/specs/2026-07-14-live-testnet-implementation.md sección 6.2; plan docs/superpowers/plans/2026-07-14-live-testnet-implementation.md Task 3). El diseño quedó en docs; el wiring de main.rs nunca llegó.
- Referencias de código en el repo: **CERO** (LiveTestnetExecutor, executor::, GasOracle e IdempotencyChecker de este directorio aparecen solo en docs de la spec; los GasOracle / RoundTripContext que salen en grep pertenecen a sed-core y prioritization-spine, entidades distintas).

### 1.3 ¿Está completo? NO — internamente inconsistente (prueba de que jamás se compiló)

Si mañana se agregara "mod executor;" a main.rs, fallan **4 bloqueantes**:

1. mod.rs:160 llama IdempotencyChecker::new() SIN argumentos — el real (idempotency.rs:16) es pub async fn new(redis_url: &str) -> Result<Self,_>. Aridad Y asincronía no coinciden.
2. mod.rs:172 llama .check_or_insert(plan_hash) — método inexistente; el real es check_and_lock (idempotency.rs:28).
3. mod.rs:155 llama NonceManager::new() sin args y sync — el real (executor/nonce_manager.rs:21) es pub async fn new(provider, address) -> Result<Self,_>.
4. gas_oracle.rs:2-3 importa ethers_core y ethers_providers como crates directas — NO declaradas (workspace Cargo.toml:69 solo declara el umbrella ethers; las dependencias transitivas no son importables).

Además: imports muertos en mod.rs (TypedTransaction, Eip1559TransactionRequest, TransactionReceipt, Signer, interval, Duration, etc.) generarían warnings y cargo clippy -- -D warnings fallaría.

### 1.4 ¿Qué haría si viviera? — y por qué NO lo requiere LIVE_MAINNET

- LiveTestnetExecutor::execute_testnet_opportunity es un **noop**: chequea idempotencia y devuelve ExecutionReceipt con status "paper_shadow_noop", tx_hash None, con una transición de estado fabricada Approved->Finalized. La máquina de 16 estados (ExecutionState) no tiene run-loop, ni consumer del canal opportunity_rx, ni tracking de receipts — **vaporware**.
- El terminus VIVO ya implementa el ciclo completo con MÁS gates que el fantasma: SubmitEngine::execute (kill-switch -> signer -> ValidatedPlan fail-closed TTL 300s -> pre-execute checklist 11 checks -> value cap -> relay_no_submit_sim pre-egress -> paper short-circuit -> eth_callBundle re-sim fail-closed -> multi-relay broadcast -> inclusion tracker -> métricas) + barrera runtime LiveExecPolicy como PRIMERA sentencia de build_and_sign (bundle_builder.rs:106-112).
- Duplicaciones REGRESIVAS del fantasma:
  - GasOracle duplica el fee-logic ya vivo en bundle_builder.rs:155-172 (mismo esquema base_fee + 2 gwei, max = 2*base + priority), pero con **2 gwei hardcodeado** — el mismo literal que WO-04 ya marca como deuda.
  - executor/nonce_manager.rs usa Arc<Provider<Http>> crudo -> **bypasea HttpRpcPool** (circuit breaker + failover EWMA, G-RPC-1) que el NonceManager vivo sí usa. Conectarlo DEGRADARÍA la disciplina RPC.
  - LiveTestnetExecutor como segundo motor de ejecución viola §34.1 (hot-path mode-invariant: la matemática no cambia por modo; un executor separado "testnet" ES semántica por modo) y crea una **segunda ruta de capital** — superficie de ataque duplicada, inaceptable bajo §9/§34.4. Un noop que reporta Finalized sin broadcast **no funciona correctamente con capital real**: falsearía el ledger de settlement.
- Único fragmento con idea residual de valor: IdempotencyChecker (Redis SET NX EX 86400). El árbol vivo ya tiene protección equivalente: arbx:pending_tx:<addr> (Check 11: SET EX 180 + DEL post-resolución, TTL como backstop) + ValidatedPlan TTL 300s. El patrón NX sería un hardening futuro OPCIONAL (WO aparte), NO un prerrequisito de LIVE_MAINNET.

**Respuesta a (a):** NO está completo, NO haría nada que el árbol vivo no haga mejor, y LIVE_MAINNET NO lo requiere — el terminus canónico §34.3 es SubmitEngine + live_exec_policy, que ya existe y está verificado en runtime (hermano N6: default-deny VIVO, paper, sin signer).

---

## 2. (b) Opciones — INTEGRAR vs ELIMINAR

### Opción A — INTEGRAR (declarar el módulo y reparar)
- Costo real: arreglar los 4 bloqueantes + imports muertos + clippy deny + ESCRIBIR el run-loop del state machine que no existe (200-400 líneas nuevas) + wiring a consumer.rs + tests. No es "declarar un mod": es construir un segundo executor.
- Beneficio: ~0 (todo duplicado o inferior a lo vivo).
- Riesgo: segunda ruta de capital; noop mentiroso en el ledger; bypass del pool RPC; violación §34.1/§34.4.
- **DESCARTADA.**

### Opción B — ELIMINAR (git rm -r del directorio)
- Se pierden 350 líneas que nunca compilaron, con 0 referencias de código. La historia queda íntegra en git (4 commits arriba); recuperable con git checkout de los SHAs si algún día se revive. La spec docs de 2026-07-14 conserva el diseño original.
- Beneficio: árbol honesto — todo .rs bajo src/ compila. Hoy el fantasma es una trampa de lectura: un futuro PR podría "conectarlo" sin ver los 4 bloqueantes y con la tentación de parchearlos a lo bruto (introduciendo la segunda ruta).
- Riesgo: ninguno funcional (0 call-sites); de proceso: viaja bajo el MISMO ID de anomalía del wiring de refresh (§37 P-∅: un PR = un ID — WO-05 §2.5).

### Opción C — HÍBRIDA (RECOMENDADA): B + wiring de NonceManager::refresh
La parte (b) del WO NO es código fantasma: refresh es API viva con un gap REAL de wiring (sección 4). El mismo WO cierra ambas: elimina el fantasma Y conecta el refresh. **RECOMENDADA.**

---

## 3. (c) Recomendación con riesgo para la ruta de capital (§9 — paranoia institucional)

**RECOMENDACIÓN: Opción C.** Justificación bajo la pregunta canónica §34.4:

1. **¿Funcionaría con capital real?** El fantasma NO (noop que reporta Finalized; nonce manager sin failover; gas literal). El árbol vivo SÍ (N6 lo verificó en runtime). Eliminar el fantasma REDUCE la superficie del terminus — el terminus ES la frontera §9.
2. **¿La matemática cambia por modo?** Integrar LiveTestnetExecutor la cambiaría (motor separado por modo) -> rechazado por §34.1. El wiring de refresh es mode-invariant: corrige el cache de nonce en TODOS los modos.
3. **Riesgo residual del wiring de refresh** (declarado, no oculto):
   - refresh es fail-soft (log + continúa): si el re-fetch falla, el contador queda desincronizado como hoy (status quo ante), el warn lo hace visible, y el PRÓXIMO evento de nonce huérfano reintenta. No es capital-expuesto: un nonce adelantado produce bundles no incluidos (costo de oportunidad), no pérdida directa.
   - Concurrencia: NonceManager::state es Mutex<HashMap> — refresh y next() serializan por lock, sin locks anidados (sin deadlock). En vivo, Check 11 (arbx:pending_tx) ya serializa broadcasts por address, acotando la carrera a mas o menos 1 nonce.
   - El camino runtime de refresh no tiene test automatizable sin seam de trait (HttpRpcPool es concreta; extraer trait sería refactor invasivo, fuera de §3 Surgical). Gate local = compile+lint+tests existentes; runtime queda **APPLIED_UNVERIFIED hasta un test anvil-fork** (follow-up declarado, honesto R8).

**Eden/Beaver (parte c del WO):** 0 líneas confirmado. NO se diseña código: la arquitectura ya tiene el punto de extensión correcto (trait RelayBackend + catálogo DB relays, migración 013, POST /admin/relays), y agregar backends sin credenciales reales del operador sería especulación (§2) y sin ID de anomalía que lo exija (P-∅). Camino correcto: (1) OPERADOR hace onboarding de credenciales Eden/Beaver -> filas en relays; (2) si se decide diversificar, PR nuevo con relay_eden.rs / relay_beaver.rs siguiendo el patrón relay_titan.rs. Hoy el pool corre con 0 backends (N6: relay_backends "none", modo NotSubmitted) — honesto y fail-closed.

---

## 4. NonceManager::refresh — el gap REAL (análisis del desync)

Semántica viva (src/nonce_manager.rs:39-52): next() hace fetch RPC del nonce pending SOLO la primera vez por (chain_id, address); después incrementa SOLO local para siempre. refresh (:56) re-fuerza el cache desde RPC — doc comment: "Call on nonce-mismatch error" — pero **nadie lo llama** (verificado: única aparición en todo backend/ es la definición; #[allow(dead_code)] silencia el lint desde que se escribió).

**Defecto latente en la ruta LIVE:** cada vez que un bundle consume un nonce del contador local y NO aterriza on-chain, el contador queda ADELANTE de la cadena -> todos los bundles posteriores firman nonces demasiado altos -> nunca incluidos -> **atasco silencioso hasta reinicio del proceso**. En LIVE_MAINNET con competidores, inclusion-timeout es lo NORMAL, no la excepción: el primer drop dejaría el terminus inerte.

Puntos de fuga (nonce consumido en bundle_builder.rs:150, nunca broadcast/incluido):

| # | Punto | Ancla exacta en submit_engine.rs | Nonce aterrizó? |
|---|---|---|---|
| 1 | Todos los relays rechazaron el bundle | rama !broadcast_result.any_success() (~L678-707) | NO |
| 2 | Abort fail-closed de eth_callBundle (BE-05) | CallBundleDecision::Abort (~L648-661) | NO |
| 3 | Inclusion timeout | InclusionOutcome::Dropped (~L804-815) | NO (o tarde — refresh cubre ambos: relee el pending count) |
| 4 | BuildError POST-nonce (fee/gas/estimate/sign) | rama Err(e) genérica de build_and_sign (~L466-468); los errores pre-nonce (LiveExecDenied, ValueExceedsCap, UnsupportedStrategy, verbatim) ocurren ANTES de nonce_mgr.next() (bundle_builder L110-148 vs L150) | NO |
| 5 | Paper short-circuit CON signer (bundle firmado, jamás broadcast) | bloque if paper (~L506-540) | NO |

NO requieren refresh: Included/Reverted (el nonce SÍ aterrizó — el incremento local coincide con el consumo on-chain) ni los not_submitted previos a build_and_sign (nonce no consumido). Nota para call-site 4: los BuildError pre-nonce también dispararían el resync — inocuo (re-fetch idempotente del valor actual del contador) y preferible a discriminar variantes dentro del match (§3 quirúrgico).

---

## 5. (d) Diffs propuestos (para Oleada 4 — applier Rust, árbol principal con target/ caliente §36.4)

### Diff 1 — ELIMINAR el directorio fantasma (-350 líneas)

    git rm backend/relays-client/src/executor/mod.rs
    git rm backend/relays-client/src/executor/gas_oracle.rs
    git rm backend/relays-client/src/executor/idempotency.rs
    git rm backend/relays-client/src/executor/nonce_manager.rs

CERO edits adicionales: main.rs no lo declara (nada que quitar), ningún otro archivo lo referencia. La limpieza de imports/variables no aplica (§3: limpiar solo el desorden propio — aquí el desorden es el directorio entero).

### Diff 2 — backend/relays-client/src/nonce_manager.rs

- Eliminar la línea #[allow(dead_code)] sobre pub async fn refresh (L55): el allow muere al existir call-sites.

### Diff 3 — backend/relays-client/src/submit_engine.rs

A) Helper privado en impl SubmitEngine (junto a los helpers dropped/not_submitted), estilo del entorno, sin unwrap/expect (el crate los tiene en deny). Firma y cuerpo propuestos:

    async fn resync_nonce(&self, chain_id: u64, addr: Address, cause: &str) {
        if let Some(nm) = self.nonce.as_ref() {
            match nm.refresh(chain_id, addr).await {
                Ok(n) => info!(event = "nonce.resynced", chain_id, nonce = n, cause,
                    "nonce cache re-synced from eth_getTransactionCount"),
                Err(e) => warn!(event = "nonce.resync_failed", chain_id, cause, error = %e,
                    "nonce refresh failed; local counter stays stale until next desync event"),
            }
        }
    }

Doc-comment propuesto para el helper (resumen del §4 de este diseño): re-sincroniza el contador local tras consumir un nonce que NO aterrizó on-chain (nonce huérfano); sin esto todo bundle posterior firma un nonce demasiado alto y nunca se incluye (atasco silencioso hasta reboot); fail-soft R8 — si el re-fetch falla se registra warn y el próximo evento de desync reintenta; nunca fabrica un nonce. Address = ethers::types::Address (ya disponible via signer.address); info!/warn! ya importados.

B) Cinco call-sites, cada uno como primera sentencia de la rama indicada (ANCLAS por evento/branch — los números de línea pueden derivar; signer y opp están en scope en los cinco, verificado por lectura):

1. CallBundleDecision::Abort, antes del return dropped:
   self.resync_nonce(opp.chain_id, signer.address, "callbundle_abort").await;
2. rama if !broadcast_result.any_success(), antes del return ExecutionResult:
   self.resync_nonce(opp.chain_id, signer.address, "all_relays_failed").await;
3. InclusionOutcome::Dropped, primera sentencia de la rama:
   self.resync_nonce(opp.chain_id, signer.address, "inclusion_timeout").await;
4. rama Err(e) genérica de build_and_sign (la que hace not_submitted con build_error):
   self.resync_nonce(opp.chain_id, signer.address, "build_error_post_nonce").await;
5. paper short-circuit, primera sentencia del bloque if paper:
   self.resync_nonce(opp.chain_id, signer.address, "paper_short_circuit").await;
   (corrige la contaminación del contador cuando paper corre CON signer y luego se flipea a live dentro del mismo proceso)

El helper no toca live_exec_policy, ni caps, ni checklist — solo el cache de nonce.

### Verificación exigida al applier (comandos EXACTOS, desde backend/)

    cargo check -p relays-client
    cargo clippy -p relays-client -- -D warnings
    cargo fmt --check
    cargo test -p relays-client

Verificación adicional honesta (R8): git grep -n "executor/" -- backend/ debe devolver VACÍO post-rm; grep de refresh en backend/relays-client/src/ debe mostrar definición + 5 call-sites + 0 allow(dead_code).

### Qué NO se hace en este WO (declarado, no olvidado)

- NO se extrae trait de HttpRpcPool para testear refresh con mock (refactor invasivo, fuera de charter).
- NO se mueve nonce_mgr.next() después de estimate_gas en bundle_builder.rs (micro-mejora válida que estrecha la ventana de fuga, pero tocar el orden del hot-path de firma merece su propio PR con ID).
- NO se implementan relay_eden.rs / relay_beaver.rs (sin credenciales ni necesidad evidenciada — P-∅).
- NO se toca live_exec_policy.rs / default-deny / MainnetRefused (§34.3 intocable).
- NO se adopta la idea NX-idempotency del fantasma (hardening futuro opcional, WO aparte si el operador lo pide).

---

## 6. (e) Gates

### arbx-pre-edit-audit (dispara: hot-path Rust, archivos de más de 300 líneas)
- **Diseño (esta sesión):** lectura completa de los archivos relevantes ANTES de opinar; git status revisado (código de relays-client LIMPIO — CERO drift vs origin/main según hermano N6 §1.4).
- **Applier (Oleada 4, obligatorio repetir):** re-leer submit_engine.rs completo + git status ANTES del primer edit; confirmar branch intencional (§36.1) y target/ caliente del árbol principal (§36.4); re-anclar los 5 call-sites por evento/branch (los números de línea de este diseño pueden haber derivado).

### arbx-risk-limits-enforcement (dispara: tocar executor/hot-path)
- max_value_eth hard cap: INTACTO (el diff no toca build_and_sign).
- Checklist 11 checks, kill-switch, ValidatedPlan fail-closed, callBundle fail-closed: INTACTOS (el resync es posterior a la decisión, no la sustituye; fail-soft explícito).
- live_exec_policy default-deny + MainnetRefused + ARBX_LIVE_EXEC_ENABLED: INTACTOS (§34.3).
- El resync NO baja ningún límite: añade corrección de estado (re-sincroniza HACIA la verdad on-chain, nunca inventa). Un contador desincronizado era el estado de mayor riesgo; el diff lo reduce.
- Modo: diseño producido bajo §32/§33 (audit/read-only); capital expuesto = 0; sin flips.

### §37 P-∅ (carga de la prueba)
- ID de anomalía: informe §2.5 / GOAL-WORKORDERS WO-05 (uno solo: código fantasma en ruta de capital). La eliminación + el wiring son las dos caras del MISMO ID — un solo PR.
- Qué pasa si no se hace: el terminus live queda con (i) una trampa de segunda-ruta de capital en el árbol, y (ii) atasco de nonces permanente tras el primer inclusion-timeout. Revert declarado: git revert del PR restaura ambas piezas.

---

## 7. Verificación ejecutada en ESTA sesión (fail-honest, R8)

| Afirmación | Comando | Resultado |
|---|---|---|
| "mod executor;" jamás existió | git log --all -S "mod executor;" sobre main.rs | vacío (0 commits) |
| executor/ = 350 líneas | wc -l sobre los 4 archivos | 44+42+200+64 = 350 |
| refresh sin call-sites | grep de .refresh( en backend/relays-client/src | solo la definición (nonce_manager.rs:56) |
| Eden/Beaver 0 líneas | grep -i eden/beaver en backend/ | 0 reales (todos falsos positivos precedence/credential) |
| 0 referencias al fantasma | grep de LiveTestnetExecutor / executor:: / GasOracle / IdempotencyChecker / check_or_insert en el repo | solo docs/spec + entidades homónimas de otros crates |
| No edité código | — | CERO escrituras fuera de este .md; CERO git mutante |

NO ejecuté cargo check/clippy en esta sesión (diseño READ-ONLY sobre árbol sin cambios: el estado compile del fantasma es irrelevante — no compila por definición de árbol; el estado del crate vivo ya es verde en CI según N6). Los comandos de verificación del §5 son exigencia para el applier.

---

## 8. Estado final

- **Decisión:** ELIMINAR src/executor/ (-350 líneas) + INTEGRAR NonceManager::refresh (helper resync_nonce + 5 call-sites). Eden/Beaver: gap de OPERADOR (catálogo DB), sin código.
- **Status:** DESIGNED — listo para Oleada 4 (applier Rust, un PR, un ID WO-05).
- **Riesgo residual declarado:** refresh fail-soft (reintento en próximo desync, warn visible); camino runtime de refresh sin test automatizable local (APPLIED_UNVERIFIED hasta anvil-fork); micro-mejora de orden nonce/gas-estimate en bundle_builder parked para WO futuro.
