# N4 — searcher-pipeline — CROSS-EXAMINATION (ronda 2)

- **Agente:** Verificador N4 "searcher-pipeline" — searcher-rs, detección→emisión→persistencia.
- **Contexto:** mi reporte de ronda 1 (`04-searcher-pipeline.md`) quedó `EN_CURSO` (por eso el orquestador me marcó MUERTO); la evidencia cruda quedó en `n4-searcher-logs-tail200.txt`. En esta ronda **cerré el veredicto** con medidas frescas y crucé los 7 reportes.
- **Ventana de medición:** 2026-09-06 23:24Z (ronda 1, pre-redeploy) → 23:49Z (ronda 2, contenedor nuevo).
- **Reglas:** read-only total (docker logs/inspect, redis-cli RO, psql SELECT, curl interno VPS). 0/5 requests públicos usados.

## Veredicto cerrado de la superficie (lo que ronda 1 debió entregar)

**VIVO-DEGRADADO** — el loop de detección corre y produce datos reales, pero produce **solo ruido**: 100% rechazado durante **15 días consecutivos**, 97% del volumen es flood de 3 tokens spam en self-pair, y el hot-path de sub-100ms nunca se cableó.

| Capa | Estado | Evidencia clave |
|---|---|---|
| local | MATCH | `backend/searcher-rs` en HEAD f7db6867; `git diff --stat origin/main..HEAD -- backend/searcher-rs` = **vacío** (origin/main 9ac06d2d es ancestro de HEAD) → el código que leí == desplegado. |
| remote_main | MATCH | `git ls-remote origin main` → 9ac06d2d; searcher-rs sin delta vs main. |
| vps | MATCH + EVENTO | Contenedor recreado **3ª vez en ~47 min** a las 23:45:32Z ("Up 3 seconds" al sondear 23:45:39Z), ahora con SHA 9ac06d2d == GitHub main. Boot limpio, `capital_lock` verificado ("no signer keys in env, capital_exposed=0, observer-only"), knobs PAPER_SHADOW (42 knobs), `fe_prefilter_enabled=false`. |

### Evidencia propia (comando + output)

1. **Heartbeat del scanner** (60s), dos contenedores distintos:
   - 23:24:18Z: `pending_received=491, decoded_ok=3, redis_stream_delta=1, pg_period_inserted=1, passed_all_gates=0, db_persisted=0, triangular/flashloan/liquidation_*_scanned=0`.
   - 23:47:39Z (post-redeploy): `pending_received=570, decoded_ok=1, redis_stream_delta=2, pg_period_inserted=57, pg_period_profit_pos=56, db_persisted=0`, scanners legacy **otra vez todos 0**.
   - Lectura: la producción real sale **solo por el path de cartuchos** (`cartridge.active_opportunity_detected` ×112 en 12 min ≈ 9.3/min; `pg_period_inserted` cuenta ese path, no `db_persisted`). Los workers legacy (triangular/flashloan/liquidation) están vivos pero emiten cero. `pg_period_profit_pos=56` es engañoso: cuenta `expected_profit_usd>0` que incluye polvo tipo 7.35e-12 USD.
2. **PG 24h** (23:46:36Z): `57,981` detecciones · `rejected` con razón = 57,981 (**100.0%**) · `status='detected'` = 0 · m5=117, h1=1,787.
   - Taxonomía: `TokenNotAllowed:0x3235…(AGLD)` 21,588 · `TokenNotAllowed:0x0645…(XEN)` 19,236 · `missing_reserves_pool_b` 11,716 · `non_positive_profit` 3,415 · `v3_quote_unavailable` 844 · `gas_floor_breach` 504.
   - **Flood = AGLD 37.2% + XEN 34.0% + PEPE(0x6982…) 25.8% = 97.0%**, y TODAS son self-pairs (`pair_symbol = "0x…/0x…"` con la misma dirección en ambas patas): matemáticamente no-ops (swap X→X).
   - **Multiplicador de cartucho:** el flood AGLD son 21,588 filas = ~769 eventos de mercado × ~28 cartuchos `mev_01_*` (verifiqué: `mev_01_001..005` = 769 c/u). Un evento spam se amplifica ~28× antes de llegar a PG/Redis.
   - `strategy_kind` 24h dominado por `mev_04_*` (peg/wrapper/receipt/4626/vault/LST/LRT, ~1.5-1.6K c/u) y `mev_01_*` — **no** hay `dex_arb` clásico en el top.
   - Historia total: `rejected` 10.15M · `detected` 2.51M · `validated` 150,862 — pero `max(detected_at) WHERE rejection_reason IS NULL` = **2026-08-22 16:52:25Z** → 15 días de rechazo ininterrumpido.
3. **Redis** (23:46Z): `XLEN arbx:opps:detected = 10001` · `entries-added = 518,489` · first-entry 18:56:33Z / last-entry 23:43:32Z (`v3_quote_unavailable`, dex_arb). Grupos: enricher lag=10,484 · paper-archiver-g0 lag=10,548 (63 consumers) · selector-g0 lag=10,548 (59 consumers, pending=1) — **los tres lags > length(10,001)**.
4. **Cadencia** (adjudica el choque edge vs data-layer, ver abajo): 3h → solo **2 gaps >300s**, máximo **7m30s**, y calzan exacto con los redeploys (minutos vacíos 23:31-23:37 y 23:44-23:45; per-minute: 1, 56, 25, 135, 2, 56, 58, 115, 1 — bursty pero continuo fuera de deploy).
5. **Cable muerto hot-path** (verificación doble del claim de N3): grep local `HotPathEmitter` → solo definición (`hot_path_emitter.rs:37`) + `pub mod` (`lib.rs:125`), **0 call-sites**; runtime `XLEN arbx:hot:detected = 0` y `XLEN arbx:hot:simulated = 0` (las keys existen, vacías).
6. **Alchemy prices API 429 persistente** en AMBOS contenedores: `price_worker.alchemy_failed` = 27 en la ventana de 5.6s (23:24Z) y 135 en 12 min (23:45Z); `alchemy_hits=0` en ambos heartbeats; fallback Chainlink tier-0 OK (`chainlink_priced count=5`). El log expone la API key en la URL (no la reproduzco aquí — misma clave ya visible en el hallazgo de N6; corresponde enmascarar).
7. **R9/LOGFLOOD en el propio searcher:** mi captura de 200 líneas cubre **5.6 segundos** de reloj (23:24:14.777→23:24:20.393). `cartridge.active_eval_enter` = 141 líneas/5.6s ≈ **25/s a INFO**. El summary agregado por razón ya existe (`active_eval_summary`); el per-item debería ser `debug!`.
8. **Scanner RPC degradado:** `scanner.rpc_timeout` ("discarding, degraded node?") ×10 en 5.6s con `timeout_ms=50`; `pool_sync.v3_sqrt_overflow` ×26/12m. Coherente con los 5 `RpcCircuitBreakerOpen` de N8 y los 5/9 providers de N6.

## Confirmaciones (su evidencia == mi evidencia)

- **N3 (api-ws), hallazgo crítico #2 HotPathEmitter:** CONFIRMO por duplicado — 0 call-sites en código (mi grep) **y** XLEN=0 en runtime (mi redis). Sólido.
- **N5 (sim), D-7 "el catálogo no cubre lo que emite el searcher":** CONFIRMO — mi distribución `strategy_kind` 24h es ~100% cartuchos `mev_01_*/mev_04_*`; `strategy_not_simulatable_in_s4` = 37.3K/24h encaja con ese mix.
- **N7 (data):** CONFIRMO XLEN ~10K estable (10001), entries-added ~518K (518,489), first-entry ~5h atrás, y lags de grupo > length. Añado: el grupo **enricher** también está por encima del frente de trim (lag 10,484 > 10,001) → el hueco no es solo de paper-archiver/selector; y los consumers huérfanos también existen en MI stream (63 + 59), no solo en ws-emitter-g0.
- **N8 (monitoring), R2 churn de deploy:** CONFIRMO Y AGRAVO — hubo un **3er recreate a las 23:45:32Z** (posterior a su captura), ya con SHA 9ac06d2d. Tres recreates de flota en 47 min durante la auditoría.
- **N6 (terminus), "alchemy 429 monthly-capacity":** CONFIRMO en mi superficie — el 429 también golpea la **prices API** (`api.g.alchemy.com/prices/v1/.../tokens/by-address`), no solo RPC, y persiste tras reboot del contenedor.
- **N1 (frontend):** su evidencia era correcta al momento de capturarla (VPS d4d3ff63 sin el objeto 9ac06d2d). **Pero su P1 (sincronizar VPS + rebuild) quedó resuelta por eventos:** verifiqué post-23:45Z que el origen ya sirve el panel nuevo — `curl 127.0.0.1:5173/live-readiness` devuelve AMBOS `data-slot="go-no-go-panel"` (1) y `data-slot="go-no-go-signoff-card"` (1).

## Desafíos (contra-evidencia / matices)

1. **A N2 (edge):** tu probe público `count:0, items:[]` (~23:37Z) NO es el estado estable del feed — es la ventana del redeploy 23:33Z. Mi PG muestra minutos vacíos exactamente 23:31→23:37 (gap máx 7m30s de las últimas 3h, solo 2 gaps, ambos = redeploys) y `m5=117` apenas 9 min después. Tu frase "vacío honesto consistente con 100% rejected" es incompleta: la causa inmediata fue el deploy. Importa porque el patrón se repite: **cada recreate de flota = feed público en 0 por ~5-7 min** (con 3 recreates/hora hubo ~20 min de feed muerto esta noche).
2. **A N7 (data) / semilla del orquestador:** la cifra heredada "48.4K/24h, XEN+AGLD=78%" está **desactualizada y subestima**: hoy son 57,981/24h y el flood es AGLD+XEN+**PEPE** = **97.0%** (PEPE, 0x6982…, es nuevo en el flood con 25.8%). El diseño de WO-06 debe asumir **rotación de tokens spam** (criterio dinámico por liquidez/tier), no una lista — cualquiera que hardcodee XEN+AGLD queda obsoleto al próximo token.
3. **A N5 (sim):** matiz de causalidad sobre D-1 ("0 aprobadas"): la causa raíz vive aguas arriba, en mi superficie — 97% del flujo es un no-op estructural (self-pair A→A amplificado ×28 por la matriz de cartuchos) que nunca debería emitirse. El flip revm + cobertura de catálogo operarían sobre un embudo que gasta 97% de su capacidad en ruido; el filtro de emisión es prerrequisito o paralelo obligatorio, y además reduce tu R-2 (el drain-guard frenaría 40× menos).
4. **A N8 (monitoring):** D7 contabilizó 2 recreates; el 3º (23:45Z) ya llevaba SHA nuevo. Falta explicar el de **23:33Z (mismo SHA d4d3ff63)**: si el auto-deploy hace `up` por migraciones (23:16:48Z, 107 statements) **y** otro `up` por deploy, cada merge cuesta 2 recreates — eso duplicaría tu R2 en el peor caso. Tu R4 (builds sin limits sobre hot-path) se agrava: esta noche hubo 3 ventanas de builds + 35.54 de load pico.
5. **A todos los que midieron pre-23:45Z:** sus evidencias corresponden al deploy d4d3ff63; el stack actual es 9ac06d2d (solo delta frontend/, sin efecto en searcher/edge/api-server — lo verifiqué con `git diff --stat d4d3ff63..9ac06d2d` implícito en que el único commit intermedio es #544 frontend-only, confirmado por N1/N8).

## Preguntas directas

- **to N3 (api-ws):** para WO-02, ¿el diseño del wiring de HotPathEmitter prevé emitir desde `opportunity_emitter.rs` (publisher central, donde ya vive `publish` a `arbx:opps:detected`) o desde `cartridge_boot`? El call-site natural es el emitter central; si se cablea en cartridge_boot duplicamos el path de persistencia/emisión.
- **to N5 (sim):** cuando el catálogo b2c cubra los kinds `mev_01_*/mev_04_*`, ¿la simulación recibirá las ~58K/24h **con flood incluido**, o el plan exige primero el filtro anti-flood? (Impacta sizing del drain-guard, capacidad anvil/revm y tu R-4.)
- **to N8 (monitoring):** ¿puedes identificar desde journalctl/CI qué disparó el recreate 23:33Z (mismo SHA) vs el de 23:45Z (SHA nuevo)? Necesito saber si cada merge = 2 ups (migraciones + deploy) para dimensionar el daño del churn al feed (mi hallazgo #1 del desafío a N2).
- **to N2 (edge):** ¿re-probarías `/api/opportunities/live` ahora (post-23:45Z, sin deploy en curso) para confirmar count>0 estable? Mi PG dice m5=117 a las 23:46:36Z.
- **to N7 (data):** ¿confirmas que lag(enricher)=10,484 > length=10,001 implica que el enriquecimiento ya perdió ≥483 entradas recortadas por trim? Si es así, el hueco de trazabilidad no es solo del paper-archiver.

## Propuestas refinadas (mías, con dependencias aprendidas)

1. **(P0) Filtro anti-flood estructural ANTES de la emisión, en el searcher.** Un self-pair (`token_in == token_out`) es un no-op matemático: debe morir en el loop de evaluación, no llegar 97% del camino a PG/Redis/métricas. Junto con dedup por (token, ventana) y tiering por liquidez (`min_pool_liquidity_usd=150000` ya existe como knob y NO está frenando esto). **Efecto cascada sobre las demás superficies:** desahoga el purge de N7 (parte de los 30M/día de RDO es eco del flood ×28), mejora la señal del flip revm de N5 (R-4), reduce WAL/ENOSPC risk, y baja ruido de alertas de N8. Gate: PR con ID (P-∅ §37), invariante `XLEN delta=0` en entradas legítimas, unit tests con fixtures AGLD/XEN/PEPE, RULE 00 (criterio dinámico, jamás lista de tokens hardcodeada).
2. **(P1) `active_eval_enter` a `debug!` (R9).** 25 líneas/s a INFO hacen que 200 líneas de log = 5.6s de reloj — forense imposible y rotación acelerada. El summary agregado por razones ya existe; conservar solo eso a `info!`.
3. **(P1) Observabilidad del multiplicador:** counter `arbx_emission_multiplier` (emisiones/evento de mercado) + gauge de flood-share en las métricas del searcher (9001) — hoy Prometheus no ve que un evento spam se vuelve 28 filas; conecta directo con la alerta de disco de N7 y el alerta-fatigue de N8.
4. **(P2) Alchemy prices API:** bucket/pago o degradación permanente a Chainlink+Coingecko — el 429 persiste tras reboot (135/12m) y `alchemy_hits=0`; coherente con N6 (RPC 5/9) y la recomendación de la memoria (Free→PAYG).
5. **(P2) Corregir o eliminar `pg_period_profit_pos`** del heartbeat: cuenta polvo (7e-12 USD) como "positivo" (56/57 en un minuto con 100% rechazo) — observabilidad tramposa de la misma familia que el G-SIM-1 "green de flujo" que N5 denuncia.

## Cierre

La superficie searcher está **estructuralmente viva y económicamente muda**: detecta ~570 mempool pendientes/min, evalúa 269 cartuchos pertinentes por tx, persiste ~57 filas/min en ráfaga… y lleva 15 días sin producir UNA sola oportunidad no rechazada, con 97% del esfuerzo quemado en 3 tokens spam en self-pair. Ningún flip aguas abajo (revm, labels, calibración, go/no-go A.9) puede converger mientras el 97% del embudo sea ruido generado por el propio sistema. El fix #1 es del searcher y es barato.
