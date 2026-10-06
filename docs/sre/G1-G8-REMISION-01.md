# G1-G8 REMISIÓN-01 — re-medición de los gates de Mainnet live (2026-10-06)

- **Tarea:** t54 · **Intento:** 1 · **attempt_id:** `a63d65bf-c14d-4312-968d-9c0fb9a5574f`
- **Rol:** SRE · **Rama:** `sre/g1-g8-remision-01` · **In-scope tocado:** `docs/sre/` (este documento).
- **`main` medido:** `3f00b359beca82685280c5d8d30f099d8bd7d921` **desplegado** (no el `a38e6779` de ayer).
- **Ventana de medición:** `2026-10-06T03:22:38Z` → `03:26:03Z`.
- **Modo:** `paper`. Sin firma, sin broadcast, sin capital, sin mutar el VPS. Todas las lecturas son GET read-only.
- **Fuente de criterios:** `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (v2.0.0) + el estado previo `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` (leído entero, 25 líneas).

> **Estándar aplicado (§34.5.3 y la nota anti-regresión del propio documento):** todo PASS de esta tabla cita su artefacto reproducible. Lo que no pude medir está declarado **NO COMPUTADO** con la razón exacta — **no se rellena con 0**. Precedente citado por el documento fuente: G2 del skill v2.0.0 citó evidencia fabricada una vez (rama `codex/567` + commit `9a10350`, inexistentes). Un PASS sin artefacto aquí es peor que un FAIL.

---

## §0 — RESPUESTA A LA PREGUNTA CENTRAL

**El cuello `v3_quote_unavailable` YA NO ESTÁ VIVO.** Medido sobre el `main` actual:

- **0 ocurrencias** de `v3_quote_unavailable` en el embudo vivo (`/api/opportunities/live`, 26 cards, `ts=2026-10-06T03:25:16Z`). Tampoco `v3_pool_not_catalogued`.
- El pipeline de descubrimiento reporta **`multi_hop_status: "route_ready"`** y **`required_data_gate.verdict: "ready"`** (tier `executable`), con **500 ciclos rentables encontrados** y **`multi_hop_v3_skipped: 0`**.
- La telemetría muestra el pricing V3 **ocurriendo**: `v3_source_priced cartridge=dex_arb chain=1 source_pool=0xbaa1… source_price=138999.46 mode=shadow`.

**Pero el objetivo NO está desbloqueado: cambió el cuello.** El embudo ahora muere en la **economía**, no en los datos:

| `rejection_reason` (26 cards, ventana viva) | n |
|---|---|
| `non_positive_profit` | 11 (13 en ventana 900 s) |
| `spread_negative_round_trip` | 8 (7) |
| `single_pool_no_spread` | 4 |
| `v3_multileg_budget_exhausted` | 2 |
| `no_tradable_size` | 1 |
| **`v3_quote_unavailable`** | **0** |

**Y G2 sigue sin poder medirse**, ahora por una razón distinta: no por el cuello, sino porque **ningún canal disponible expone `simulations WHERE passed`** (§2). G3 sí se midió y da **0**.

---

## §1 — GATE POR GATE

### G1 — Deploy veraz / infra: ✅ **PASS (medido)**

```
$ curl -s https://edge-arbx.ape-tv.net/status      (ts 2026-10-06T02:33:52.974Z)
deploy.sha = 3f00b359beca82685280c5d8d30f099d8bd7d921
deploy.id  = 37399887059
deploy.at  = 2026-10-06T02:03:30Z
env        = production-like          services ok:200 = 7

$ gh run view 37399887059 --json headSha,status,conclusion
{"conclusion":"success","headSha":"3f00b359beca82685280c5d8d30f099d8bd7d921","status":"completed"}
  jobs: Wait for all deployment gates  success  01:40:11Z → 02:03:14Z
        Deploy to VPS                 success  02:03:16Z → 02:33:40Z

$ curl -s https://edge-arbx.ape-tv.net/api/v1/health
system_status=healthy  math_guardian=passed  entropy=0.5837
searcher_rs running · selector_api running · sim_ctl running · recon running
relays_client running · postgres running · redis running   (7/7)

$ git rev-parse origin/main
3f00b359beca82685280c5d8d30f099d8bd7d921
```

**Los cuatro coinciden**: desplegado == `origin/main` == `deploy.sha` declarado == SHA del run de deploy. Además, la identidad **horneada** de ese mismo despliegue se midió en t46: `built_from_sha = 3f00b359…` en **10/10 imágenes** (runs `37404691956` y `37404762561`), con `recreated_by_deploy=true` en 10/10.

Con el criterio del estado de 2026-09-17 (VPS healthy + sin commits locales sin deploy), **G1 pasa de ⚠️ PARCIAL a ✅ PASS**: el drift que lo bloqueaba (3 commits locales sin deploy) está cerrado y la identidad horneada lo confirma.
*Frontera:* el criterio literal del skill para GATE-1 es otro (manifest V3 de 318 pools + 2 firmas humanas + backup testeado y restaurado). **Eso NO lo medí** → ese enunciado queda **NO COMPUTADO**.

### G2 — Simulación cíclica (≥1 sim passed): ❌ **NO COMPUTADO**

**La query del contrato, ejecutada, y su salida real:**

```
sql_query: SELECT COUNT(*) FROM simulations WHERE passed;
  -> {"ok": true, "data": "", "rows_affected": 0}
```

**Eso no es un 0: es un canal mudo.** Discriminador ya establecido en t7 y **re-verificado hoy** con una query que *debe* devolver una fila:

```
sql_query: SELECT COUNT(*) FROM executions;
  -> {"ok": true, "data": "", "rows_affected": 0}
```

Una `SELECT COUNT(*)` siempre devuelve exactamente una fila. Recibir `data: ""` con `ok: true` significa que **el canal no transporta datos** (informó t7: `SELECT * FROM __tabla_que_no_existe__` también devuelve `ok:true`). **G2 se declara NO COMPUTADO, no 0.**

**Por qué no pude usar otro canal:**

1. **Ninguna ruta del api-server lee `simulations`.** `grep -rn 'FROM simulations' backend/api-server/src` → **0 coincidencias**. La tabla existe y está poblada (`database/migrations/113_simulations_revm_idempotency.sql:17` la llama "a populated live table (~640k rows)"), pero **no hay superficie HTTP que la exponga**, ni siquiera vía el edge.
2. **`.github/workflows/` está FUERA del in-scope de t54** (`docs/sre/`), así que no puedo añadir un workflow de SQL read-only. Y **ningún workflow existente tiene input libre** de SQL/command: revisé los `inputs:` de los 24 workflows con `workflow_dispatch` — todos piden SHAs, run-ids, flags de confirmación o modos acotados. El que sí consulta PG (`pipeline-integrity.yml:53`) lo hace por `ssh arbx`, un alias local que no existe en el runner (por eso falla 12/12, diagnosticado en t31).
3. **No inyecté nada a través de workflows ajenos.** Un input que se interpola en un comando remoto permitiría ejecutar SQL arbitrario en el VPS; usar esa vía como canal no es un mecanismo sancionado y lo descarté por doctrina (mismo criterio que "no inventes uno nuevo" para SSH).

**Artefactos vivos MÁS CERCANOS (no son la query, y no la sustituyen):**

- `/api/readiness` → item **`G-SIM-1` = `red`** (`verified_at 2026-10-06T03:26:03.006Z`):
  > `premature flag — SECURE_BOOT violated: ARBX_SIMULATOR_V2_READY=true with evidencia de checklist 5/7; unmet items: [variance_benchmark, second_signoff] — stale (>30d): second_signoff — recorded failures (measured, not missing): variance_benchmark (samples_labeled=0 < min_samples=21; pred_failed=21; distinct=21)`
- **CLAIM histórico, citado como claim, NO como medición** — `backend/api-server/src/routes/readiness-extras.ts:371-376` (comentario del 2026-08-29):
  > *"0 of 639,955 sims have EVER passed — S4 probes need token_in the placeholder sim signer does not hold (TRANSFER_FROM_FAILED) and 97% of strategy kinds are not simulatable in S4"*

  Ese `0 de 639.955` es exactamente el tipo de número que este documento NO debe heredar: es un comentario de código de hace 5 semanas, no una medición de hoy. **No lo uso como resultado de G2.**

### G3 — Paper→submit engine con ciclo real: ❌ **FAIL medido** (con una frontera declarada)

Dos rutas alcanzables leen `executions` **con SQL real contra PG**:

```
$ curl -s "https://edge-arbx.ape-tv.net/api/recon/summary"      (ts 2026-10-06T03:25:20.542Z)
{"window_hours":1,"totals":{"total":0,"included":0,"reverted":0,"dropped":0,
 "avg_pnl_included_usd":null,"avg_confirm_latency_ms":null},...}
```

→ `backend/api-server/src/index.ts:1100-1110`: `SELECT COUNT(*)::int AS total, … FROM executions WHERE submitted_at >= NOW() - ($1::text || ' hours')::interval`. **`COUNT(*)` puro, sin JOIN, ventana 1 h → 0.**

```
$ curl -s "https://edge-arbx.ape-tv.net/api/executions/recent"   (ts 2026-10-06T03:25:18.916Z)
{"count":0,"items":[],"ts":"2026-10-06T03:25:18.916Z"}
```

→ `index.ts:1068-1081`: `SELECT … FROM executions e JOIN opportunities o ON o.id = e.opportunity_id ORDER BY e.submitted_at DESC LIMIT $1` y responde `count: q.rows.length`. **0 filas** en un read DESC sin filtro temporal ⇒ no existe ninguna ejecución con oportunidad asociada.
**Caveat declarado:** esta ruta lleva `JOIN`; en rigor prueba "0 ejecuciones *con* oportunidad", no "0 filas en la tabla". La lectura sin JOIN de `/api/recon/summary` es la que sostiene el `0` limpio — pero sólo dentro de su ventana de 1 h. **`COUNT(*) FROM executions` sin ventana no es computable por ninguna ruta alcanzable** (todas filtran por tiempo o joinean): esa parte queda **NO COMPUTADO**.

**Ledger paper (los 598K runs REJECTED del 2026-09-17):**

```
$ curl -s https://edge-arbx.ape-tv.net/api/paper/history/summary   (ts 03:25:1xZ)
{"ok":true,"source":"postgres","window_hours":24,
 "data":{"totals":{"total":"0","profitable":"0","strategies":0,"chains":0},"accepted":{"total":"0",...}}}

$ curl -s https://edge-arbx.ape-tv.net/api/metrics/paper-shadow    (ts 03:25:17.724Z)
{"metrics":{"consecutive_green_days":0,"target_days":7,"pnl_today_usd":0,"pnl_accumulated_usd":0,
 "status":"INACTIVE","started_at":null,"last_trade_at":null,
 "total_trades":0,"green_trades":0,"red_trades":0}}
```

**Lectura:** el ledger paper **está INACTIVO y no crece**: 0 filas en 24 h (`source: postgres`), 0 trades, 0 días verdes de 7. El total histórico (~598 K) NO lo medí — esa cifra es del documento del 2026-09-17 y **no se hereda**.
**G3 = FAIL**: `executions` = 0 (1 h, sin JOIN) y el ledger paper lleva 24 h sin una sola fila.

### G4 — Net-profit gate on-chain honesto: ⚠️ **NO COMPUTADO**

Sin sims passed medibles no hay input para ejercitar el gate. Lo único medible y medido:

```
$ curl -s https://edge-arbx.ape-tv.net/api/capital-gates
{"status":"ok","live_enabled":false,"capital_exposed":0,"broadcast":false,
 "submit_enabled":false,"private_relay_enabled":false,
 "gates":[{"name":"capital_exposure","status":"PASS","value":0}]}
```

→ El término de capital está **cerrado**: `live_enabled=false`, `capital_exposed=0`, `broadcast=false`, `submit_enabled=false`. Eso **no** acredita que el gate de net-profit on-chain sea honesto cuando reciba input: **NO COMPUTADO**.

### G5 — Contratos Sepolia verificados: ⚠️ **NO COMPUTADO**

No lo medí: requeriría Etherscan/verificación on-chain, fuera del canal que tengo en esta estación. **No se hereda el ✅ histórico del documento de 2026-09-17** (§34.5.3 exige revalidar al SHA nuevo).

### G6 — Fork replay + invariants: ⚠️ **NO COMPUTADO** (con artefacto de estado)

```
$ curl -s https://edge-arbx.ape-tv.net/api/crucible/status
{"status":"ok","ready":false,"rows":[],"count":0,
 "reason":"no_crucible_rows_available","false_green_guard":true}
```

→ El servicio responde y **declara honestamente que no tiene filas** (`false_green_guard: true`). No hay fork-replay medible en esa superficie: **NO COMPUTADO**.

### G7 — Risk-limits + checklist pre-ejecución: ⚠️ **PARCIAL (medido)**

```
$ curl -s https://edge-arbx.ape-tv.net/api/gates/status        (ts 03:22:52Z)
summary: {"total":8,"passed":7,"failed":0,"fired":1,"blocked":0,"average_score":87.5}
  gates: paper_mode PASSED · kill_switch PASSED · simulation_required PASSED · risk_limits PASSED
         pg_opportunity_flow PASSED (5132 opps/5min) · redis_opp_stream PASSED (XLEN 10001)
         redis_gate_commits FIRED (XLEN arbx:gate-commit:checksum = 0) · redis_gas_price PASSED

$ /api/readiness → G-RIS-1 green: "[risk] limits + drawdown trigger configured;
   kill-switch reachable via Redis (desarmado); auto_trip_on_high_revert_rate=true"
```

→ **Los límites están configurados y el kill-switch es alcanzable (desarmado)**, con 7/8 gates en `passed` y **0 en `failed`**. El único `fired` es `redis_gate_commits` (0 commits registrados). **El DRILL de trip/untrip sigue NO COMPUTADO**: no hay artefacto de un disparo/reapertura real del kill-switch, que es lo que el documento fuente marcaba como faltante.

### G8 — Acta + paquete de activación: ⚠️ **NO COMPUTADO**

Es un gate documental; no lo re-midó esta tarea. **No se hereda el ✅.**

---

## §2 — ¿SIGUE VIVO EL CUELLO `v3_quote_unavailable`? **NO** (artefactos)

### 2.1 El embudo vivo ya no lo produce

```
$ curl -s "https://edge-arbx.ape-tv.net/api/opportunities/live"        (ts 2026-10-06T03:25:16Z)
count=26 window_total=26   status: rejected 26/26
  non_positive_profit            11
  spread_negative_round_trip      8
  single_pool_no_spread           4
  v3_multileg_budget_exhausted    2
  no_tradable_size                1
  v3_quote_unavailable            0     ← el cuello histórico
  v3_pool_not_catalogued          0     ← el que lo reemplazó en 2026-10-04
```

Re-medido con la ventana ampliada (`?window=900` y `?window=3600`): mismo conjunto, mismo orden, cero `v3_quote_*`. **Todas las razones vivas son de ECONOMÍA**, no de datos: `non_positive_profit` (no hay ganancia neta) y `spread_negative_round_trip` (el diferencial de ida y vuelta es negativo). Eso significa que **los candidatos SÍ llegan al gate económico**: ya no mueren antes, en la obtención de la quote.

### 2.2 El pipeline de datos declara `ready`

```
$ curl -s https://edge-arbx.ape-tv.net/api/route-discovery/status      (updated 2026-10-06T03:23:04Z, mode=shadow)
last_tick:
  multi_hop_status              "route_ready"
  required_data_gate.verdict    "ready"          ← tier "executable"
  required_data_gate.tier       "executable"
  multi_hop_profitable_cycles   500
  routes_found                  500
  routes_dispatched             200
  multi_hop_v3_skipped          0
  pools_total                   1139
  strategy_status_counts        {route_ready 79, needs_route_data 174, no_compatible_route 3, observe_only 8}
  graph_rejected_reasons        {missing_slot0 170, missing_token_metadata 71, low_liquidity 38}
  lat_pass_p95                  false   (lat.state p95 1672 ms vs target 3 ms)
```

→ El `required_data_gate` que antes fallaba ahora **declara `ready`** con tier `executable`, y `multi_hop_v3_skipped = 0` (no se saltea V3 por falta de datos).

### 2.3 El pricing V3 está ocurriendo (telemetría viva)

```
$ curl -s https://edge-arbx.ape-tv.net/api/cartridges/status           (ts 2026-10-06T03:22:57.790Z)
{"cartridge_id":"mev_01_026_parallel_route_arbitrage","chain_id":1,"level":"info",
 "message":"v3_source_priced cartridge=dex_arb chain=1 source_pool=0xbaa1fc…be53668
            sqrt_price_x96=29538344512313875682229448301777 liquidity=13304285785632558843092
            source_price=138999.46716004514 mode=shadow"}
```

### 2.4 Contexto histórico (citado como documentado, NO como medición de hoy)

- `docs/audits/GRAPHROUTING-CERTIFICADO-274f04fd-ADDENDUM-2026-10-04.md:55`: `v3_quote_unavailable` **29.251 → 339** ("colapsó … efecto buscado de #790"), con la masa migrando a `v3_pool_not_catalogued` (→ 21.088).
- `docs/personalops/PERSONALOPS-DELTA-v1.4.0-2026-10-05.md:96`: `v3_quote_unavailable` **6.057–6.188 (10,2–10,4 %)**.
- Hoy, en el embudo vivo: **0 de 26**. Los PRs `#797` / `#791` / `#793` / `#792` atacaron exactamente esa clase y el efecto es consistente con lo que se mide.

**Conclusión del cuello:** `v3_quote_unavailable` **ya no es el cuello único** del 2026-09-17. Ese enunciado está **obsoleto**.

---

## §3 — ¿CUÁL ES EL CUELLO HOY?

No hay un único cuello medido que explique todo, y **lo declaro como tal** en vez de fabricar uno. Lo que la evidencia sostiene:

1. **El candidato muere en la economía, no en los datos.** 26/26 rechazos con razón económica (`non_positive_profit` 11, `spread_negative_round_trip` 8, `single_pool_no_spread` 4, `no_tradable_size` 1). Los datos llegan: el gate de datos declara `ready` y el pricing V3 ocurre.
2. **El hop de simulación no se puede medir desde ningún canal disponible** (§1/G2) → **no se puede afirmar ni negar** que haya un sim passed. Es el hueco de evidencia más grande de esta tabla.
3. **G3 es 0 de verdad** (medido sin JOIN, 1 h) y **el ledger paper lleva 24 h sin una fila** con estado `INACTIVE`.

Lectura honesta: **el cuello hoy es la ausencia de un candidato con EV positivo que llegue a simular y a paper.** El bloqueo *de datos* (`v3_quote_unavailable`) está cerrado; el bloqueo *económico* está vivo y es el que mantiene G2/G3 en su estado actual. Pero **G2 no computado** impide cerrar la cadena de causalidad: no sé cuántos sims pasaron, y por eso **no declaro un cuello único**: declaro el que la evidencia sostiene (económico) y el hueco que la evidencia deja (sims-passed sin canal).

**Cuello único: NO IDENTIFICABLE con la evidencia disponible.** Se declara así en vez de elegir uno sin artefacto.

---

## §4 — Lo que esta medición NO hizo

- **NO midió** `simulations WHERE passed` (canal inexistente desde esta estación — §1/G2).
- **NO midió** `COUNT(*) FROM executions` sin ventana (ninguna ruta lo expone sin filtro temporal o JOIN).
- **NO midió** el total histórico del ledger paper (`598K` es del documento del 2026-09-17; no se hereda).
- **NO midió** G5 (contratos Sepolia on-chain) ni G8 (paquete documental).
- **NO midió** el drill trip/untrip del kill-switch (G7).
- **NO usó** la inyección de comandos vía inputs de workflows ajenos como canal.
- **NO tocó** `backend/`, `frontend/`, `.github/workflows/`, `scripts/`, `implementation-state/` (fuera de in-scope), ni el VPS.
- **NO empujó a `main`**; el cambio va en rama + PR sin merge.

---

## §5 — Procedencia y reproducción

| # | Comando (crudo) | Resultado |
|---|---|---|
| 1 | `curl -s https://edge-arbx.ape-tv.net/status` | `deploy.sha=3f00b359…`, `id=37399887059`, `at=2026-10-06T02:03:30Z`, 7×`ok:200` |
| 2 | `gh run view 37399887059 --json headSha,status,conclusion` | `success` / `completed` / `3f00b359…` |
| 3 | `gh run list --workflow=auto-deploy-vps.yml --limit 3` | `37399887059 success` · `37399866155 cancelled` · `37399845470 failure` |
| 4 | `curl -s .../api/v1/health` | `healthy`, 7/7 running |
| 5 | `curl -s .../api/opportunities/live[?window=900,3600]` | 26 cards, razones según §0/§2.1, `v3_quote_unavailable`=0 |
| 6 | `curl -s .../api/route-discovery/status` | `route_ready`, `verdict ready`, 500 ciclos, `v3_skipped 0` |
| 7 | `curl -s .../api/cartridges/status` | `v3_source_priced … source_price=138999.46` |
| 8 | `curl -s .../api/recon/summary` | `COUNT(*) executions` 1h = **0** |
| 9 | `curl -s .../api/executions/recent` | `count=0, items=[]` |
| 10 | `curl -s .../api/paper/history/summary` | `source=postgres`, 24h `total="0"` |
| 11 | `curl -s .../api/metrics/paper-shadow` | `INACTIVE`, `total_trades=0`, `green_days=0/7` |
| 12 | `curl -s .../api/readiness` | 19 items; `G-SIM-1=red`, `G-RIS-1=green`, `G-PAP-1=yellow` |
| 13 | `curl -s .../api/gates/status` | `{total:8, passed:7, failed:0, fired:1}` |
| 14 | `curl -s .../api/crucible/status` · `.../api/capital-gates` | `no_crucible_rows_available` · `live_enabled=false` |
| 15 | `sql_query: SELECT COUNT(*) FROM simulations WHERE passed;` | `{"ok":true,"data":"","rows_affected":0}` ← **canal mudo** |
| 16 | `sql_query: SELECT COUNT(*) FROM executions;` | `{"ok":true,"data":"","rows_affected":0}` ← **canal mudo** |

Fuentes leídas: `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` (25 líneas) · `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (296 líneas) · `backend/api-server/src/index.ts:1063-1132` · `backend/api-server/src/routes/readiness-extras.ts:320-390` · `database/migrations/113_simulations_revm_idempotency.sql:17` · `docs/audits/GRAPHROUTING-CERTIFICADO-274f04fd-ADDENDUM-2026-10-04.md:55` · `docs/personalops/PERSONALOPS-DELTA-v1.4.0-2026-10-05.md:96` · `implementation-state/REGLAS-OPERATIVAS.md:174,204`.

**Esto NO mueve P/N (0/115) ni acredita ningún otro criterio.** Es una medición de gates, no una activación: `ARBX_TRADE_MODE=paper`, sin firma, sin broadcast, sin capital, sin mutación del VPS.
