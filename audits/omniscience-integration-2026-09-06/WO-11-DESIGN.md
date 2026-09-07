# WO-11 — DISEÑO: benchmark reproducible del kill-switch "<10ms" (claim autodeclarado)

- **WO:** WO-11 · kind: design (READ-ONLY — CERO edición de código de producción, CERO git, CERO cargo, 0 requests públicos, VPS intocado)
- **Agente:** ecc:performance-optimizer / rubric ecc:benchmark-methodology (Gang Omniscience, IA OMEGA) · 2026-09-06
- **Charter:** diseñar el benchmark reproducible del kill-switch "<10ms" que hoy es autodeclarado (informe §6.12; `CLAUDE.md` §9 promete "<10ms vía API/File/Edge"). Entregables: (1) definición operativa medible con anclas file:line del mecanismo REAL; (2) harness local reproducible (script + criterios); (3) umbrales de aceptación y política de corrección de docs si falla (RULE 00/R8); (4) separación honesta de componentes (detección vs cese de emisión vs drenaje in-flight).
- **Archivos bajo claim:** `audits/omniscience-integration-2026-09-06/WO-11-DESIGN.md` (este documento). Nada más se edita.
- **Presupuesto:** 0/5 requests HTTP públicos usados. 0 comandos SSH. 0 compilaciones (WO design-only). Solo lectura del árbol local.
- **APPLY-PASS v2 (2026-09-06, respawn — mismo charter read-only):** re-despacho kind:apply sobre el diseño ya existente. Este pase (1) re-verificó INDEPENDIENTEMENTE cada ancla file:line contra el árbol — las aserciones de agentes NO son facts (§9 ledger: 51 checks de anclas, todas exactas); (2) corrigió 2 defectos del borrador v1 MÁS ALLÁ de las anclas (C1: stream `arbx:opps:scored` citado dos veces NO EXISTE; C2: modo "sin Redis mide solo K1" era imposible — el cliente no se construye sin conexión viva); (3) endureció el preflight del runbook (C3: abort también con clave AUSENTE); (4) COMPLETÓ los harness A y B como fuentes íNTEGRAS listas para aterrizar (el borrador v1 solo tenía un sketch de comentarios y §8 prometía "diffs propuestos" — gap cerrado). Sin cargo, sin git, sin VPS, 0 requests públicos.

---

## 0. RESOLUCIÓN EJECUTIVA

**El claim "<10ms" es INDEFINIBLE como latencia end-to-end del kill con la arquitectura viva.** El mecanismo real es *poll-based*: los consumidores gated consultan `arbx:killswitch` en Redis con caché TTL de 1 s (`shared-rs/src/killswitch.rs:61`) y cadencias de loop de 2 s (`XREADGROUP BLOCK 2000`) / 5 s (`sleep(5000)` cuando ya está halted). El propio producto declara en su UI: *"Arming blocks every hot-path service from submitting new executions **within ≤ 5 s**"* (`frontend/app/killswitch/page.tsx:75`) — **la UI y `CLAUDE.md` §9 se contradicen entre sí**, y ninguna de las dos tiene medición.

La única lectura donde "<10ms" es defendible es la **latencia de una sola consulta de estado** (`is_enabled()`): con caché caliente es una lectura de `RwLock` en memoria (sub-microsegundo); con caché fría es un `GET` de Redis (sub-milisegundo en loopback). Eso es un *check*, no un *kill*.

**Decisión de diseño:** el benchmark mide **6 segmentos (K1–K6)** separados con definición operativa desde-hasta explícita, en dos harness: (a) in-process Rust contra Redis real aislado (local u operador), (b) runbook VPS operator-only (los agentes NO tocan el VPS, §32/§33). El claim de docs se reescribe con el número MEDIDO del segmento que corresponda — nunca con una cifra inventada (RULE 00/R8). Hallazgo adicional fail-honest: el vector **"File"** de `CLAUDE.md` §9 **NO EXISTE** en el código, y el searcher **NO se detiene** con el kill armado (por diseño).

---

## 1. MECANISMO REAL DEL KILL-SWITCH (anclas file:line verificadas)

### 1.1 Vector API — EXISTE (el único real)

Cadena de armado completa:

| Paso | Mecanismo | Ancla |
|---|---|---|
| 1. UI operador | `/killswitch` pide token admin + razón obligatoria (audit) | `frontend/app/killswitch/page.tsx:47-69` |
| 2. Edge proxy | `POST /admin/killswitch` reenvía a api-server con `x-arbx-admin-token` | `edge/worker/src/index.ts:918-935` |
| 2b. Edge acciones semánticas | `POST /api/killswitch/:action` (activate/deactivate) → mismo upstream | `edge/worker/src/index.ts:958-983` |
| 3. API server | `requireAdminToken` + zod + captura before-state | `backend/api-server/src/index.ts:254-262` |
| 4. Escritura | `killSwitch.set()` → `SET arbx:killswitch` + `PUBLISH arbx:killswitch:changes` | `backend/api-server/src/index.ts:263-267` → `shared-ts/src/killswitch/index.ts:54-67` (SET :62, PUBLISH :63) |
| 5. Auditoría | `writeAudit("killswitch.armed"/".disabled", …)` + log `admin.killswitch` | `backend/api-server/src/index.ts:268-276` |
| 6. Puerto loopback VPS | edge `0.0.0.0:8787:8787`; Redis `127.0.0.1:6379:6379` | `docker/compose.prod.yml:422-423`, `:65-66` |

Cliente Rust espejo (mismos semántica y clave): `backend/shared-rs/src/killswitch.rs` — `KILLSWITCH_KEY:15`, `KILLSWITCH_CHANNEL:16`, caché TTL 1 s `:61`, `is_enabled() :72-77`, `state()` fast-path de caché `:81-87` + `GET :89`, `set()` SET+PUBLISH `:121-122`. Gauge `arbx_killswitch_enabled`: `backend/shared-rs/src/metrics.rs:74-78`.

### 1.2 Vector File — NO EXISTE (declaración fail-honest)

`CLAUDE.md` §9 promete "<10ms vía **API/File/Edge**". Búsqueda exhaustiva (`kill.*(file|mtime|watch)`, `killfile`, `notify.*kill`, watchers de archivo) sobre `backend/**/*.{rs,ts}`: **cero resultados**. No hay watch de mtime, no hay kill-file. El vector "File" es documentación aspiracional sin código. El benchmark NO puede medirlo; la corrección documental (§5.2) lo declara inexistente.

### 1.3 Vector Edge — EXISTE como proxy, NO como kill independiente

El edge worker no posee semántica de kill propia: `POST /admin/killswitch` (:918) y `POST /api/killswitch/:action` (:958) son `fetch` de reenvío al api-server. La lectura `GET /api/killswitch/status` (:939) tiene `cf: { cacheTtl: 0 }` (:949) aunque su comentario dice "2s KV cache TTL" (:938) — comentario stale a corregir con el mismo WO documental. Conclusión: "vía Edge" = "vía API con un hop extra" (ese hop ES medible como K3-edge, §2).

### 1.4 Consumidores gated (dónde ocurre el efecto REAL)

| Servicio | Stream consumido | Gate | Cadencia de detección | Efecto al armar |
|---|---|---|---|---|
| selector-api | `arbx:opps:detected` (`consumer.ts:23`) | `killSwitch.isEnabled()` loop-top | caché ≤1 s + `XREADGROUP BLOCK 2000` (`consumer.ts:127`) | halt: log `consumer.halted_kill_switch` + `sleep(5000)` (`backend/selector-api/src/consumer.ts:87-105`, sleep :103) |
| sim-ctl | `arbx:opps:validated` (`consumer.rs:44`) | `killswitch.is_enabled()` loop-top (`sim-ctl/src/consumer.rs:95`) | caché ≤1 s + `BLOCK 2000` (`:158-161`) | halt idéntico A5-STALL: `sim_consumer.halted_kill_switch` + sleep 5 s (`:100-116`), resume loggeado (`:117-122`) |
| relays-client | `arbx:opps:simulated` (`consumer.rs:21`) | **por-oportunidad**, no loop-top | inmediato al consumir cada opp | `Dropped("kill_switch_on")` (`backend/relays-client/src/submit_engine.rs:346-349`); con PG presente el gate es el checklist check-1 (`backend/shared-rs/src/pre_execute_checklist.rs:214`, impl `:263-279`) |
| recon (drift_tracker) | — (tick periódico) | `is_enabled()` por tick | intervalo `cfg.interval_secs` | **gate INVERTIDO** — ver §1.5-E |

### 1.5 Divergencias fail-honest declaradas (el benchmark las mide o las expone, no las tapa)

| # | Divergencia | Evidencia |
|---|---|---|
| A | **El searcher NO se detiene con el kill armado** — por diseño: "The kill-switch blocks execution downstream (relays-client), but the intelligence layer always detects opportunities". `run_subscription` descarta el handle: `let _ = killswitch; // reserved` | `backend/searcher-rs/src/scanner.rs:1140-1142` y `:1243`; el `ChainSupervisor` solo lo clona hacia el scanner sin gatear (`chain_supervisor.rs:290`, `:315-318`) |
| B | **Ausencia de suscriptor Rust del canal pub/sub**: `KILLSWITCH_CHANNEL` se publica (`killswitch.rs:122`) pero ningún binario Rust se suscribe; solo el api-server TS invalida caché por suscripción (`api-server/src/index.ts:60-62` → `shared-ts/.../killswitch/index.ts:70-84`). La propagación a Rust es 100 % poll | grep `KILLSWITCH_CHANNEL` en `backend/**`: solo publish |
| C | **Semántica de clave ausente divergente**: `KillSwitchClient` ausente → `default_when_absent` = `true` en prod (fail-closed; `configs/app.toml:7`) pero el check-1 del checklist trata clave ausente como **NO armado** (fail-open; `pre_execute_checklist.rs:277-278`) y no-parseable como bloqueado (`:272-275`). Con PG caído + Redis key wiped, el terminus tiene dos respuestas distintas según el gate que corra | `pre_execute_checklist.rs:263-279` vs `killswitch.rs:90-96` |
| D | **`KillSwitchGate` del SED es stub `todo!()`** — nunca corre en runtime | `backend/sed-core/src/types/kill_switch.rs:56-65` |
| E | **Gate invertido en drift_tracker**: `if !killswitch.is_enabled().await { continue; }` — idlea cuando el kill está OFF y **corre cuando está ARMADO** (el comentario declara la intención opuesta) | `backend/recon/src/drift_tracker.rs:161-167` (gate `:164`) |
| F | **UI vs doctrina**: UI promete "≤ 5 s" (`page.tsx:75`); `CLAUDE.md` §9 promete "<10ms vía API/File/Edge". Ambas autodeclaradas (informe §6.12) | `frontend/app/killswitch/page.tsx:75` vs `CLAUDE.md:219-220` (anchor exacta verificada apply-pass v2: "### Kill Switch" :219 / "Respuesta inmediata y determinística en <10ms vía API/File/Edge." :220) |

---

## 2. DEFINICIÓN OPERATIVA MEDIBLE (segmentos K1–K6)

Principio: cada segmento declara **desde qué evento, hasta qué efecto, con qué reloj**. Nada intermedio se agrega. El reloj es SIEMPRE monotónico (`std::time::Instant` en Rust; `time.monotonic_ns()` en el runbook; los timestamps de `docker logs --timestamps` son wall-clock del mismo host — se documenta y se asume skew intra-host ≈ 0).

| ID | Nombre | Desde (t₀) | Hasta (t₁) | Qué mide | Predictores arquitectónicos |
|---|---|---|---|---|---|
| **K1** | CHECK-WARM | justo antes de `is_enabled()` con caché válida | retorno del llamado | Latencia de la lectura de estado cacheada (RwLock + branch) | ns–µs. **Único candidato legítimo a "<10ms"** |
| **K2** | CHECK-COLD | ídem con caché expirada (TTL 1 s vencido) | retorno | Redis `GET` + parse serde + escritura de caché | sub-ms loopback; ms sobre red |
| **K3** | ARM | antes de `set()` (o antes del POST HTTP saliente para K3-api / K3-edge) | ack de `SET`+`PUBLISH` (o respuesta HTTP completa) | Costo de armar por la vía canónica | K3-redis: 1 RTT Redis; K3-api: + zod + audit PG + 1 hop; K3-edge: + hop edge |
| **K4** | VISIBILITY | t₀ del ARM | un lector de caché fría observa `enabled=true` | Propagación del estado a un observador nuevo | ≈ K3 + 1 RTT Redis |
| **K5** | DETECTION | t₀ del ARM (ack de SET) | el consumidor gated observa `enabled=true` en su loop | La latencia de detección del poller — **el corazón del claim** | ≤ caché TTL (1 s) + `XREADGROUP BLOCK` (2 s) + RTT ≈ **~3 s worst-case**; el `sleep(5000)` solo aplica al ciclo siguiente si ya estaba halted |
| **K6** | CUTOFF (cese de emisión) | t₀ del ARM | último `XADD` downstream de un consumidor gated (`arbx:opps:validated`/`simulated`/`executed`) | Fin del trabajo emitido, incluye drenaje del batch in-flight (COUNT 16 selector `consumer.ts:126` / 8 sim-ctl `consumer.rs:159` / 4 relays `consumer.rs:151`, verificados apply-pass v2) | ≤ K5 + drain del batch en curso. **C1 (corrección apply-pass):** el borrador v1 citaba `arbx:opps:scored` — ese stream NO EXISTE en el árbol (grep 0 hits); los streams reales son `detected→validated→simulated→executed` (`consumer.ts:23-24`, `sim-ctl/consumer.rs:44-45`, `relays/consumer.rs:21,24`) |

**No-goal declarado (R8):** "proceso searcher deja de evaluar" NO es el efecto del kill — el searcher sigue detectando por diseño (§1.5-A). Cualquier benchmark que espere `XLEN arbx:opps:detected` congelado mediría una propiedad que el sistema no promete en código; la detención del flujo detectado sería un bug del searcher, no un kill.

**Garantía de capital (paper-shadow terminus):** todo opp consumido DESPUÉS de t₀+K5 es `Dropped(kill_switch_on)` o bloqueado por checklist check-1. Se verifica por la razón en logs/`paper_trade_runs`, con el flujo natural del sistema — el benchmark PROHÍBE inyectar oportunidades sintéticas (RULE 00).

---

## 3. HARNESS REPRODUCIBLE

### 3.1 Harness A — in-process Rust (local u operador): `backend/shared-rs/tests/killswitch_latency.rs` *(FUENTE ÍNTEGRA PROPUESTA — no aterrizada; apply-pass v2 cerró el gap "diffs prometidos vs sketch")*

Fuente completa, escrita contra la API REAL verificada del árbol (`killswitch.rs:54-68` connect/with_cache_ttl, `:72-77` is_enabled, `:107-129` set). El crate YA tiene todas las dependencias necesarias (`backend/shared-rs/Cargo.toml`: `redis` y `serde_json` en `[dependencies]` :25/:14; `tokio` macros+rt-multi-thread en `[dev-dependencies]` :34) — el aterrizaje NO requiere tocar `Cargo.toml`. El binario del test compila como integración (`tests/` aún NO existe en shared-rs — sería el primer archivo). **Estado: REVIEWED, NO COMPILADO (WO design-only — no cargo por charter).**

```rust
// WO-11 (2026-09-06) — kill-switch latency harness, Harness A (in-process).
// NOT a mock benchmark (RULE 00): requires a REAL Redis via REDIS_BENCH_URL
// pointing at an ISOLATED DB — the URL MUST end in "/15" (DB 0 is the LIVE
// arbx:killswitch namespace; this harness refuses it by construction).
// With no REDIS_BENCH_URL: NOTHING is measurable — the client cannot be
// constructed without a live connection (connect() dials via
// get_connection_manager) — so ALL segments print SKIP(R8) no_computed
// (correction C2 of apply-pass v2: K1 is NOT measurable without Redis either).
// Clock: std::time::Instant (monotonic) for every t0/t1 in this file.
// Segments: K1 CHECK-WARM, K2 CHECK-COLD, K3 ARM(redis), K4 VISIBILITY.
// N: 1_000 reads for K1/K2, 30 arm/disarm cycles for K3/K4.
// Output: JSONL per-sample + JSON summary {min,p50,p90,p99,max,mean}.
// Run: REDIS_BENCH_URL=redis://127.0.0.1:6379/15 cargo test --release \
//        -p shared-rs --test killswitch_latency -- --nocapture --ignored

use std::time::{Duration, Instant};

use redis::AsyncCommands;
use shared_rs::killswitch::{KillSwitchClient, KILLSWITCH_KEY};

const N_READS: usize = 1_000;
const N_CYCLES: usize = 30;
const K4_POLL_BUDGET: usize = 2_000; // ~2 s at ~1 ms/poll -> NO_COMPUTE (R8)
const K1_REFILL_SUSPECT_NS: u128 = 50_000; // >50 us on a warm read => cache refill suspect

fn pct(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn stats(mut v: Vec<u128>) -> serde_json::Value {
    v.sort_unstable();
    let n = v.len();
    let mean = if n == 0 { 0 } else { v.iter().sum::<u128>() / n as u128 };
    serde_json::json!({
        "count": n,
        "min_ns": v.first().copied().unwrap_or(0),
        "p50_ns": pct(&v, 50.0),
        "p90_ns": pct(&v, 90.0),
        "p99_ns": pct(&v, 99.0),
        "max_ns": v.last().copied().unwrap_or(0),
        "mean_ns": mean,
    })
}

fn sample(segment: &str, i: usize, dur_ns: u128, flag: Option<&str>) {
    println!(
        "{}",
        serde_json::json!({"segment": segment, "i": i, "dur_ns": dur_ns, "flag": flag})
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "bench: needs REDIS_BENCH_URL (real Redis, isolated /15). See WO-11-DESIGN.md"]
async fn killswitch_latency_harness_a() {
    let url = match std::env::var("REDIS_BENCH_URL") {
        Ok(u) => u,
        Err(_) => {
            println!(
                "{}",
                serde_json::json!({
                    "harness": "A",
                    "verdict": "SKIP(R8)",
                    "reason": "REDIS_BENCH_URL absent",
                    "no_computed": ["K1", "K2", "K3", "K4"],
                    "note": "KillSwitchClient::connect dials via get_connection_manager — no segment is measurable without a live Redis (C2, apply-pass v2)"
                })
            );
            return;
        }
    };
    // Defensive (infra code): refuse the live control-plane DB by construction.
    let db = url.rsplit('/').next().unwrap_or("");
    if db != "15" {
        panic!(
            "REDIS_BENCH_URL must target the isolated bench DB (…/15); got tail {db:?}. \
             DB 0 holds the LIVE arbx:killswitch (refused — RULE 00 / §32)."
        );
    }

    // default_when_absent = false in the BENCH ONLY: here an absent key must
    // read as "not armed" so the harness can never mistake absence for a halt.
    // (Production uses true — fail-closed — configs/app.toml:7; that semantic
    // split is benchmark finding §1.5-C, not something this harness overrides
    // in prod.)
    let warm = KillSwitchClient::connect(&url, false)
        .await
        .expect("connect warm client");
    let cold = KillSwitchClient::connect(&url, false)
        .await
        .expect("connect cold client")
        .with_cache_ttl(Duration::ZERO); // TTL 0 -> cache check `elapsed < ZERO` is always false -> every read is a Redis GET
    let observer = KillSwitchClient::connect(&url, false)
        .await
        .expect("connect observer client")
        .with_cache_ttl(Duration::ZERO); // always-fresh proxy for the K4 "new reader"

    let bench = redis::Client::open(&url).expect("open bench redis");
    let mut rmgr = bench
        .get_connection_manager()
        .await
        .expect("bench redis connect");
    let _: () = rmgr.del(KILLSWITCH_KEY).await.expect("clean bench key");

    // Environment block — document the environment, never the wish.
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut rmgr)
        .await
        .expect("INFO server");
    let redis_version = info
        .lines()
        .find(|l| l.starts_with("redis_version:"))
        .map(|l| l.trim_start_matches("redis_version:").to_string())
        .unwrap_or_else(|| "unknown".into());
    println!(
        "{}",
        serde_json::json!({
            "env": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "profile": "release (required)",
                "redis_version": redis_version,
                "bench_db": db,
                "url_note": "never include credentials in REDIS_BENCH_URL"
            }
        })
    );

    // --- K1 CHECK-WARM: cached RwLock read (killswitch.rs:81-87) ---
    warm.state().await.expect("prime warm cache"); // cold prime fills the 1 s cache
    let mut k1 = Vec::with_capacity(N_READS);
    for i in 0..N_READS {
        let t = Instant::now();
        let _ = warm.is_enabled().await; // the exact call gated consumers make (consumer.rs:95)
        let d = t.elapsed().as_nanos();
        let refill_suspect = d > K1_REFILL_SUSPECT_NS; // 1_000 warm reads take « 1 s; >50 us smells like a TTL refill GET
        sample("K1", i, d, refill_suspect.then_some("cache_refill_suspect"));
        if !refill_suspect {
            k1.push(d);
        }
    }

    // --- K2 CHECK-COLD: Redis GET + serde parse + cache write (killswitch.rs:89-98) ---
    let mut k2 = Vec::with_capacity(N_READS);
    for i in 0..N_READS {
        let t = Instant::now();
        let _ = cold.is_enabled().await;
        let d = t.elapsed().as_nanos();
        sample("K2", i, d, None);
        k2.push(d);
    }

    // --- K3 ARM(redis): SET + PUBLISH (killswitch.rs:121-122) ---
    // --- K4 VISIBILITY: fresh observer sees enabled=true (ARM t0 -> first observation) ---
    let mut k3 = Vec::with_capacity(N_CYCLES);
    let mut k4_total = Vec::with_capacity(N_CYCLES);
    let mut k4_postarm = Vec::with_capacity(N_CYCLES);
    let mut k4_no_computed = 0usize;
    for i in 0..N_CYCLES {
        let t0 = Instant::now();
        warm.set(
            true,
            Some(format!("WO-11-bench arm {}/{}", i + 1, N_CYCLES)),
            Some("wo11-harness-a"),
        )
        .await
        .expect("arm");
        let arm_ns = t0.elapsed().as_nanos();
        sample("K3", i, arm_ns, None);
        k3.push(arm_ns);

        let mut polls = 0usize;
        let mut seen = false;
        while polls < K4_POLL_BUDGET {
            if observer.is_enabled().await {
                seen = true;
                break;
            }
            polls += 1;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        if seen {
            let total = t0.elapsed().as_nanos();
            let postarm = total.saturating_sub(arm_ns);
            sample("K4_total", i, total, None); // ARM t0 -> first observation (definition §2-K4)
            sample("K4_postarm", i, postarm, None); // propagation only (arm ack -> observation)
            k4_total.push(total);
            k4_postarm.push(postarm);
        } else {
            sample("K4", i, 0, Some("no_computed_budget_exhausted")); // R8: declared, never invented
            k4_no_computed += 1;
        }

        // Disarm (untimed) + settle: next cycle starts from a clean baseline.
        warm.set(
            false,
            Some(format!("WO-11-bench disarm {}/{}", i + 1, N_CYCLES)),
            Some("wo11-harness-a"),
        )
        .await
        .expect("disarm");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Leave the bench namespace as found (bench DB only — never the live key).
    let _: () = rmgr.del(KILLSWITCH_KEY).await.expect("cleanup del");

    println!(
        "{}",
        serde_json::json!({
            "summary": {
                "K1_check_warm": stats(k1),
                "K2_check_cold": stats(k2),
                "K3_arm_redis": stats(k3),
                "K4_visibility_total": stats(k4_total),
                "K4_visibility_postarm": stats(k4_postarm),
                "K4_no_computed_cycles": k4_no_computed,
            },
            "verdict_note": "thresholds live in WO-11-DESIGN.md §4 — p99/p100 decide; never the mean"
        })
    );
}
```

- **Corrida:** `REDIS_BENCH_URL=redis://127.0.0.1:6379/15 cargo test --release -p shared-rs --test killswitch_latency -- --nocapture --ignored` (marcado `#[ignore]` para no correr en CI sin Redis; `-p shared-rs` = nombre de paquete verificado `backend/shared-rs/Cargo.toml:2`, lib `shared_rs` como la importan relays/sim-ctl).
- **Local Windows (RULE 01) — corrección C2:** sin Redis local NO se mide NI K1 — `KillSwitchClient::connect` marca la conexión (`get_connection_manager`, `killswitch.rs:56`) y no existe cliente sin Redis vivo. El harness imprime el ledger `SKIP(R8)` con los 4 segmentos `no_computed` y sale 0. Ese vacío es honesto; NO se rellena (el borrador v1 decía "mide solo K1 + overhead de serde" — imposible, corregido).
- **En VPS (operador):** contra `redis://127.0.0.1:6379/15` (Redis publicado en loopback, `compose.prod.yml:65-66`) — Redis real, namespace aislado (DB 15 ≠ DB 0 viva), y el propio harness rehúsa por construcción cualquier URL que no termine en `/15`.
- **Guard anti-contaminación:** el harness hace `DEL arbx:killswitch` en DB 15 al inicio y al final — la key viva en DB 0 jamás se toca desde Harness A.

### 3.2 Harness B — runbook VPS (operator-only): `scripts/bench/killswitch_bench.sh` *(FUENTE ÍNTEGRA PROPUESTA — no aterrizada; los agentes NO ejecutan VPS, §32/§33)*

Mide K3-api/K3-edge, K4, K5, K6 contra la pila VIVA con la key REAL (`arbx:killswitch`) — mutación de control-plane auditada, propiedad del operador:

1. **Preflight read-only:** estado actual (`redis-cli --raw GET arbx:killswitch`), `XLEN`/`XINFO GROUPS` de `arbx:opps:{detected,validated,simulated,executed}`, `docker ps`, ventana de logs (R9/LOGFLOOD check: `docker inspect --format '{{.HostConfig.LogConfig.Config}}'` + comparar `State.StartedAt` vs primera línea retenida). **ABORT en DOS condiciones (endurecido apply-pass v2, C3):** (a) kill ya ARMED — no se arma nada encima de un incidente; (b) clave **AUSENTE** — con `kill_switch_enabled_default = true` (`configs/app.toml:7`) la ausencia YA es un halt fail-closed en los consumidores (lección A5-STALL: 4 días de consumo cero silencioso), y además el disarm del bench dejaría `enabled:false` escrito, cambiando el baseline del sistema (R-7). Contenedores RESUELTOS dinámicamente (`docker ps --format` + patrón `arbitragex-v2-<svc>-1`): solo vault/minio/thanos tienen `container_name` explícito (`compose.prod.yml:713,753,785…`); redis/sim-ctl/selector-api usan el nombre default de compose (gotcha WO-15: `arbitragex-v2-redis-1`, NO `redis`).
2. **N=10 ciclos** (modesto a propósito: cada arm pausa el consumo ~3–5 s; off-peak; anunciado):
   - t₀ = `date +%s%N` (wall-clock documentado R-2; el reloj monotónico puro vive en Harness A) → `curl -w '%{time_total}' -X POST http://127.0.0.1:8787/admin/killswitch` (edge loopback, `:8787` publicado `compose.prod.yml:422-423`) con token admin y `reason="WO-11-bench arm i/N"` (auditable, `api-server/src/index.ts:268-276`) → registra **K3-edge**; la variante directa al api-server in-network (`http://127.0.0.1:8080`) registra **K3-api** — cada ciclo mide UN arm por API (alternando) y el disarm por la otra vía, de modo que ambas rutas quedan muestreadas sin churn extra.
   - **K4 (doble medición, honesta con el instrumento):** (a) **K4-pubsub** — suscriptor `redis-cli --raw SUBSCRIBE arbx:killswitch:changes` arrancado ANTES del arm, timestamp de llegada de la primera línea `"enabled":true` (cota inferior de propagación; overhead del lector ~1 ms declarado); (b) **K4-poll** — loop `docker exec … redis-cli --raw GET` desde el ack del arm hasta observar `true` (cota SUPERIOR; overhead de spawn docker-exec+redis-cli ≈ 5–20 ms por poll, declarado). El valor verdadero de un lector de caché fría vive entre ambos; se reportan los dos, jamás un promedio que los mezcle.
   - **K5:** `docker logs -f --timestamps --since <t0 RFC3339>` grep de `sim_consumer.halted_kill_switch` y `consumer.halted_kill_switch` (transición None→arm se loggea UNA vez por arm: `sim-ctl/src/consumer.rs:96-104`, `selector-api/src/consumer.ts:88-95`). `--since` es OBLIGATORIO: sin él `docker logs -f` re-emite historia vieja y `grep -m1` casaría un halt anterior. t₁ = timestamp del evento (wall-clock del host; cota superior que incluye el pipeline de logging — documentado).
   - **K6:** timestamps del último `XADD` downstream: `entry-id` (que embebe `ms-seq` del reloj del propio Redis — evita skew de reloj de lectura) en `arbx:opps:{validated,simulated,executed}` (**C1:** `scored` no existe — corrección apply-pass) capturado (i) justo tras el ack del arm (= cutoff) y (ii) tras la ventana de drenaje (`DRAIN_SECS=8` > K5 3 s + batch 2 s); K6_stream = `ms(last_id_post) − ms(ack)`; `new_entries_post_arm` se reporta por stream (hoy 100 % rejected ⇒ `simulated`/`executed` pueden estar idles — NO_COMPUTADO declarado si el stream está vacío, R8).
   - Disarm con la otra vía + razón `"WO-11-bench disarm i/N"`; verificar `consumer.resumed_after_kill_switch` (`consumer.ts:106-112`, `consumer.rs:117-122`) y estado final `enabled:false`. `trap EXIT` de emergencia desarma ante cualquier abort mid-cycle (jamás se deja el sistema armado por un bug del bench).
3. **Invariantes:** `XLEN arbx:opps:detected` SIGUE creciendo durante el arm (esperado — searcher no se detiene, §1.5-A; a ~48 K/24 h ≈ 0.6/s, una ventana de ~15 s debería ver ~8 entradas — se reportan ambos números, el operador juzga); consumo de gated streams congelado. Si `XLEN detected` se congela de forma sostenida → hallazgo grave, se reporta, no se oculta.
4. **Agregación:** python3 percentiles p50/p90/p99/max/min por segmento desde el `cycles.jsonl`; salida `audits/omniscience-integration-2026-09-06/WO-11-VPS-RESULTS-<date>.json` + tabla MD; bloque de entorno (CPU `/proc/cpuinfo` modelo, loadavg, `redis-cli INFO server` versión, versiones de imagen `docker inspect --format '{{.Image}}'`, `date` UTC + estado NTP `timedatectl` si existe).

Fuente completa del script (escrita contra las rutas/contenedores/puertos verificados; **REVIEWED, NO EJECUTADO** — operator-only):

```bash
#!/usr/bin/env bash
# WO-11 (2026-09-06) — kill-switch benchmark Harness B (VPS, OPERATOR-ONLY).
# PROPUESTA INTEGRAL (design doc) — NO aterrizada en el árbol por agentes.
#
# WHAT IT DOES (control-plane mutation, audit-logged): arms/disarms the REAL
# kill-switch N times via edge/api-server and measures K3-edge/K3-api/K4/K5/K6
# against the LIVE stack. Agents NEVER run this (§32/§33) — the operator does,
# off-peak, announced. Paper-shadow terminus: capital exposed = 0; relays
# default-deny stays untouched (§34.3).
#
# ABORTS: (a) kill already ARMED; (b) arbx:killswitch ABSENT (= fail-closed
# halt already in effect — A5-STALL, configs/app.toml:7).
set -euo pipefail

EDGE_URL="${EDGE_URL:-http://127.0.0.1:8787}"      # compose.prod.yml:422-423
API_URL="${API_URL:-http://127.0.0.1:8080}"        # api-server in-network variant
N="${N:-10}"
DRAIN_SECS="${DRAIN_SECS:-8}"                       # > K5(<=3s) + batch drain(<=2s)
: "${ARBX_ADMIN_TOKEN:?export ARBX_ADMIN_TOKEN (operator secret — never committed)}"
OUT="${OUT:-/tmp/wo11-bench-$(date -u +%Y%m%dT%H%M%SZ)}"
mkdir -p "$OUT"; CYCLES="$OUT/cycles.jsonl"; : > "$CYCLES"

# Containers are RESOLVED, never hardcoded (only vault/minio/thanos carry
# explicit container_name; everything else uses compose default naming).
cname() { docker ps --format '{{.Names}}' | grep -E "arbitragex-v2-$1-1$" | head -1; }
REDIS_C="$(cname redis)"; SIM_C="$(cname sim-ctl)"; SEL_C="$(cname selector-api)"
for v in REDIS_C SIM_C SEL_C; do
  [[ -n "${!v}" ]] || { echo "ABORT: container $v unresolved" >&2; exit 2; }
done
rcli() { docker exec "$REDIS_C" redis-cli --no-auth-warning "$@"; }

# Emergency disarm on ANY exit path — never leave the system armed by a bench bug.
disarm_now() {
  local body='{"enabled":false,"reason":"WO-11-bench emergency disarm (trap)","triggered_by":"wo11-harness-b"}'
  curl -sS -o /dev/null -X POST "$EDGE_URL/admin/killswitch" \
    -H "content-type: application/json" -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
    -d "$body" \
  || curl -sS -o /dev/null -X POST "$API_URL/admin/killswitch" \
    -H "content-type: application/json" -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
    -d "$body" \
  || true
}
trap disarm_now EXIT

echo "== WO-11 Harness B — env ==" | tee "$OUT/env.txt"
{ date -u +"%Y-%m-%dT%H:%M:%SZ"; uname -a; uptime
  grep -m1 'model name' /proc/cpuinfo || true
  { rcli INFO server | grep -E '^redis_version' || echo 'redis_version=lookup_failed'; }
  { docker inspect "$SIM_C" --format 'simctl_logcfg={{.HostConfig.LogConfig.Config}} started={{.State.StartedAt}}' || echo 'inspect_failed'; }
} 2>&1 | tee -a "$OUT/env.txt"
# R9/LOGFLOOD: verify the retained log window BEFORE trusting any "absence".
FIRST_LOG="$(docker logs "$SIM_C" 2>&1 | head -1 | cut -c1-32)"
echo "first_retained_log_line=$FIRST_LOG" | tee -a "$OUT/env.txt"

echo "== preflight (read-only) =="
KS="$(rcli --raw GET arbx:killswitch || true)"
if [[ -z "$KS" ]]; then
  echo "ABORT: arbx:killswitch ABSENT — consumers are in fail-closed halt RIGHT NOW (A5-STALL; configs/app.toml:7). Restore a present+disabled baseline before benching." >&2
  exit 2
fi
if grep -q '"enabled"[[:space:]]*:[[:space:]]*true' <<<"$KS"; then
  echo "ABORT: kill-switch already ARMED — refusing to bench on top of an incident." >&2
  exit 2
fi
BASE_XLEN_DETECTED="$(rcli XLEN arbx:opps:detected)"
echo "baseline ks=$KS xlen_detected=$BASE_XLEN_DETECTED" | tee -a "$OUT/env.txt"

last_id() { rcli --raw XREVRANGE "$1" + - COUNT 1 | head -1 || true; }  # "<ms>-<seq>" | empty
id_ms()   { if [[ "$1" =~ ^[0-9]+-[0-9]+$ ]]; then echo "${1%%-*}"; else echo ''; fi; }
ns_now()  { date +%s%N; }
rfc3339() { date -u +"%Y-%m-%dT%H:%M:%S.%3NZ"; }

# K6 per stream: ms(last post-arm entry-id, REDIS clock) - ms(ack, host clock).
# Empty stream => null (NO_COMPUTADO, R8). No new entries after ack => 0 (cutoff
# was at or before ack — the drain tail is empty). Intra-host skew documented (R-2).
k6ms() { # $1=ack_id $2=post_id $3=ack_ns
  local ack_ms post_ms
  [ -n "$2" ] || { echo 'null'; return; }
  post_ms="$(id_ms "$2")"; [ -n "$post_ms" ] || { echo 'null'; return; }
  ack_ms="$(id_ms "$1")"
  if [ -n "$ack_ms" ] && [ "$post_ms" -le "$ack_ms" ]; then
    echo 0; return
  fi
  echo $(( post_ms - $3 / 1000000 ))
}

arm_post() { # $1=enabled(bool) $2=reason $3=url -> echoes curl time_total (s)
  curl -sS -o "$OUT/http_body" -w '%{time_total}' -X POST "$3/admin/killswitch" \
    -H "content-type: application/json" -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
    -d "{\"enabled\":$1,\"reason\":\"$2\",\"triggered_by\":\"wo11-harness-b\"}"
}
assert_state() { # $1=expected_enabled(bool)
  grep -q "\"enabled\"[[:space:]]*:[[:space:]]*$1" "$OUT/http_body" || { echo "ABORT: unexpected state response: $(cat "$OUT/http_body")" >&2; exit 2; }
}

for i in $(seq 1 "$N"); do
  echo "-- cycle $i/$N --"
  # Alternate the arm route so both K3 variants get sampled across cycles:
  if [ $((i % 2)) -eq 1 ]; then ARM_URL="$API_URL"; DISARM_URL="$EDGE_URL"
  else ARM_URL="$EDGE_URL"; DISARM_URL="$API_URL"; fi

  ID_VAL_PRE="$(last_id arbx:opps:validated)"
  ID_SIM_PRE="$(last_id arbx:opps:simulated)"
  ID_EXE_PRE="$(last_id arbx:opps:executed)"
  XLEN_PRE="$(rcli XLEN arbx:opps:detected)"

  # K4-pubsub listener (started BEFORE the arm; reader stamps arrival, ~1ms overhead)
  ( timeout 15 docker exec -i "$REDIS_C" redis-cli --no-auth-warning --raw \
      SUBSCRIBE arbx:killswitch:changes 2>/dev/null || true ) \
    | while IFS= read -r line; do echo "$(ns_now) $line"; done > "$OUT/k4pub_$i.txt" &

  # K5 log watchers (--since is MANDATORY: without it docker logs -f replays
  # history and grep -m1 would match a PREVIOUS halt)
  SINCE="$(rfc3339)"
  ( timeout 20 docker logs -f --timestamps --since "$SINCE" "$SIM_C" 2>&1 || true ) \
    | grep -m1 'sim_consumer.halted_kill_switch' | awk '{print $1; exit}' > "$OUT/k5_sim_$i.txt" &
  ( timeout 20 docker logs -f --timestamps --since "$SINCE" "$SEL_C" 2>&1 || true ) \
    | grep -m1 'consumer.halted_kill_switch' | awk '{print $1; exit}' > "$OUT/k5_sel_$i.txt" &

  # --- ARM (timed) ---
  T0_NS="$(ns_now)"
  K3_S="$(arm_post true "WO-11-bench arm $i/$N" "$ARM_URL")"
  ACK_NS="$(ns_now)"
  assert_state true
  VARIANT=K3_edge
  if [[ "$ARM_URL" == "$API_URL" ]]; then VARIANT=K3_api; fi   # explicit if — a bare `[[ ]] && cmd` short-circuit would trip set -e

  # --- K4-poll (upper bound; docker-exec spawn overhead 5-20ms/poll, declared) ---
  K4POLL_START="$(ns_now)"; K4POLL_MS="null"; POLLS=0
  until grep -q '"enabled"[[:space:]]*:[[:space:]]*true' <<<"$(rcli --raw GET arbx:killswitch 2>/dev/null)"; do
    POLLS=$((POLLS+1))
    if [ "$POLLS" -gt 50 ]; then K4POLL_MS='"no_computed"'; break; fi   # R8: declared, never invented
  done
  if [ "$K4POLL_MS" = "null" ]; then K4POLL_MS=$(( ($(ns_now) - K4POLL_START) / 1000000 )); fi

  # --- K6 cutoff markers at ack ---
  ID_VAL_ACK="$(last_id arbx:opps:validated)"
  ID_SIM_ACK="$(last_id arbx:opps:simulated)"
  ID_EXE_ACK="$(last_id arbx:opps:executed)"

  sleep "$DRAIN_SECS"   # K5 window + in-flight batch drain

  ID_VAL_POST="$(last_id arbx:opps:validated)"
  ID_SIM_POST="$(last_id arbx:opps:simulated)"
  ID_EXE_POST="$(last_id arbx:opps:executed)"

  # --- DISARM (other route, timed as the sibling K3 sample) ---
  K3D_S="$(arm_post false "WO-11-bench disarm $i/$N" "$DISARM_URL")"
  assert_state false
  sleep 2

  # --- K5 result: docker RFC3339 ts -> epoch ns (same host clock; upper bound) ---
  k5_of() { f="$1"; [ -s "$f" ] || { echo 'null'; return; }
            ts="$(cat "$f")"; date -u -d "$ts" +%s%N 2>/dev/null || echo 'null'; }
  K5_SIM_NS="$(k5_of "$OUT/k5_sim_$i.txt")"; K5_SEL_NS="$(k5_of "$OUT/k5_sel_$i.txt")"
  to_ms() { [ "$1" = "null" ] && echo 'null' || echo $(( ($1 - $2) / 1000000 )); }
  K5_SIM_MS="$(to_ms "$K5_SIM_NS" "$ACK_NS")"   # ack -> halt log (upper bound:
  K5_SEL_MS="$(to_ms "$K5_SEL_NS" "$ACK_NS")"   #  includes log pipeline latency)

  # --- K4-pubsub result: first enabled:true arrival after arm ---
  K4PUB_MS="null"
  ARR="$(grep -m1 '"enabled"[[:space:]]*:[[:space:]]*true' "$OUT/k4pub_$i.txt" | cut -d' ' -f1 || true)"
  if [ -n "$ARR" ]; then K4PUB_MS=$(( ($ARR - $T0_NS) / 1000000 )); fi   # explicit if (set -e discipline)

  # --- invariant: detected keeps growing while armed ---
  XLEN_NOW="$(rcli XLEN arbx:opps:detected)"

  # One JSONL row per cycle (the §3.2-4 aggregation reads this file).
  echo "{\"cycle\":$i,\"variant\":\"$VARIANT\",\"k3_s\":$K3_S,\"k3_disarm_s\":$K3D_S,\"k4_pubsub_ms\":$K4PUB_MS,\"k4_poll_ms\":$K4POLL_MS,\"k5_sim_ms\":$K5_SIM_MS,\"k5_sel_ms\":$K5_SEL_MS,\"k6_val_ms\":$(k6ms "$ID_VAL_ACK" "$ID_VAL_POST" "$ACK_NS"),\"k6_sim_ms\":$(k6ms "$ID_SIM_ACK" "$ID_SIM_POST" "$ACK_NS"),\"k6_exe_ms\":$(k6ms "$ID_EXE_ACK" "$ID_EXE_POST" "$ACK_NS"),\"xlen_detected_pre\":$XLEN_PRE,\"xlen_detected_post\":$XLEN_NOW}" >> "$CYCLES"
done

echo "cycles written to $CYCLES — aggregate with the §3.2-4 python step (p50/p90/p99/max/min per segment, honest null/no_computed passthrough)"
```

**Nota de implementación honesta (fail-honest sobre ESTA propuesta):** el script de arriba es COMPLETO (todas las funciones auxiliares definidas: `cname`, `rcli`, `disarm_now`, `last_id`, `id_ms`, `ns_now`, `rfc3339`, `k6ms`, `arm_post`, `assert_state`, `k5_of`, `to_ms`), pero NO se ejecutó ni se validó con `bash -n` (charter read-only, 0 ejecuciones) — se declara estado **REVIEWED, NOT RUN**. Presentarlo como "probado" sería el mismo autodeclare que este WO existe para eliminar. El aterrador debe correr primero `bash -n scripts/bench/killswitch_bench.sh` (sintaxis) y un dry-run con `N=1` off-peak antes del run completo. Riesgos residuales conocidos del no-run: quoting de la fila JSONL (una sola línea larga), timestamps de `docker logs` con formato según driver (json-file emite RFC3339 nano — si el host usa otro driver, ajustar `k5_of`), y `date -d` GNU vs BusyBox (VPS Hetzner = GNU coreutils, verificado por arquitectura no por corrida).

### 3.3 Criterios de validez estadística

- **N:** K1/K2 = 1 000 iteraciones (baratas, estables); K3/K4 = 30 ciclos local, 10 en VPS (costo operativo real del flap).
- **Warm vs cold:** warm = iteraciones 2..N (después de la primera conexión/primera escritura); cold = primera iteración de cada proceso + K2 con TTL forzado a 0. Se reportan SEPARADOS, nunca mezclados.
- **Reloj:** monotónico en todos los t₀/t₁ medidos por el harness; excepción documentada: timestamps de `docker logs` y entry-ids de Redis (relojes del mismo host VPS).
- **Percentiles:** p50/p90/p99/max/min + media. **p99 y max son los que deciden el veredicto** (un kill-switch se juzga por su peor caso).
- **Reproducibilidad:** el script es determinista salvo carga; cada corrida registra loadavg y descarta (declarándolo) corridas con loadavg > 2× el mediano del día.
- **Entorno:** siempre `--release`; documentar rustc, OS, kernel, Redis server version, topología de red (loopback vs docker bridge vs WAN-edge para K3-edge público si se desea).

---

## 4. UMBRALES DE ACEPTACIÓN Y QUÉ PASA SI FALLA

| Segmento | Umbral propuesto (basado en límites arquitectónicos del §2, no en deseos) | Fundamento |
|---|---|---|
| K1 CHECK-WARM | p99 < 1 ms (esperado: µs) | lectura RwLock (`killswitch.rs:81-87`) |
| K2 CHECK-COLD | p99 < 10 ms loopback | 1 RTT Redis + serde (`:89-96`) |
| K3 ARM-redis / -api / -edge | p99 < 20 / 100 / 250 ms loopback | SET+PUBLISH; +zod+audit PG; +hop edge |
| K4 VISIBILITY | p99 < 2×K2 + K3 | arm + 1 lectura fría |
| **K5 DETECTION** | **p100 ≤ 5 000 ms** — el umbral que la UI ya declara ("≤ 5 s", `page.tsx:75`) | caché 1 s (`killswitch.rs:61`) + BLOCK 2 s (`consumer.rs:158-161`) + RTT |
| K6 CUTOFF | p100 ≤ K5 + 2 000 ms (drain del batch ≤16) | COUNT del XREADGROUP |

**Política si falla (RULE 00/R8 — se corrige, no se esconde):**

1. **K5 > 5 s medido** → la UI (`page.tsx:75`) y cualquier doc se corrigen al percentil medido con fecha y método (ej. "p99 = X s, N=10, método WO-11"). El defecto raíz (poll TTL 1 s + BLOCK 2 s, §1.5-B) queda registrado como candidato de remediación (suscribir los Rust al `KILLSWITCH_CHANNEL` ya publicado) — NO se remedia en este WO.
2. **K1/K2 > 10 ms** → el claim "<10ms" se retira también del nivel de check y se documenta la cifra medida.
3. **Invariante violada** (§3.2-3: `XLEN detected` congelado, o un opp ejecutado post-cutoff) → hallazgo CRÍTICO independiente del benchmark; se abre anomalía con evidencia antes de tocar ninguna doc.
4. **Segmento no computado** (sin Redis local, VPS no disponible) → se declara `NO_COMPUTADO` con razón exacta (R8); jamás se infiere de otros segmentos.

---

## 5. CORRECCIÓN DOCUMENTAL PROPUESTA (aplica el operador o un apply-WO posterior, con las cifras MEDIDAS)

### 5.1 `CLAUDE.md` §9 "Kill Switch"

Texto vigente: *"Respuesta inmediata y determinística en <10ms vía API/File/Edge."* (`CLAUDE.md:219-220` — anchor exacta verificada apply-pass v2) — **TRES afirmaciones sin soporte**: (a) "<10ms" solo defendible en K1/K2; (b) "File" no existe en código (§1.2); (c) "Edge" es proxy (§1.3).

Propuesta de reemplazo (los `⟨medido⟩` se llenan tras correr el harness):

> **Kill Switch**: lectura de estado en <10 ms (caché hit ⟨medido p99⟩; Redis GET ⟨medido p99⟩). Activación vía API (edge→api-server→Redis, auditado); cese de consumo de los servicios gated en ≤ 5 s (⟨medido p100⟩ — poll TTL 1 s + XREADGROUP BLOCK 2 s). El vector File no existe. La detección del searcher NO se detiene (by design — el kill es del terminus de ejecución, no de la inteligencia).

### 5.2 Ajustes menores correlativos

- `frontend/app/killswitch/page.tsx:75`: mantener "≤ 5 s" SOLO si K5 medido lo respalda; si no, sustituir por el p100 medido.
- `edge/worker/src/index.ts:938`: comentario "2s KV cache TTL" → alinear con `cf: { cacheTtl: 0 }` (:949).
- Informe §6.12 queda RESPONDED: la autodeclaración "<10ms" se convierte en claim medido por segmento o se retira.

---

## 6. SEPARACIÓN HONESTA DE COMPONENTES (qué mata qué)

| Componente | ¿Se detiene con el kill? | Mecanismo | Latencia esperada |
|---|---|---|---|
| **Detección** (searcher-rs: scanner, workers block-based, hot-path emitter) | **NO** (by design) | ninguna — `let _ = killswitch` (`scanner.rs:1243`) | n/a |
| **Scoring/labeling** (selector-api consume `detected`) | SÍ, consumo | loop-gate + sleep 5 s (`consumer.ts:87-105`) | K5 ≈ ≤3 s |
| **Simulación** (sim-ctl consume `validated`) | SÍ, consumo | loop-gate + sleep 5 s (`consumer.rs:95-116`) | K5 ≈ ≤3 s |
| **Ejecución/broadcast** (relays-client consume `simulated`) | SÍ, por-op | checklist check-1 (raw GET, sin caché — siempre fresco) + legacy gate cacheado 1 s (`submit_engine.rs:346` / `pre_execute_checklist.rs:263`) | inmediato al consumo (0 s vs arm; la oportunidad muere en la puerta) |
| **Drenaje in-flight** | bounded | batches ya leídos (COUNT ≤16/8/4) terminan; opps dentro de `process_one` completan o Dropped | K6 − K5 ≤ 2 s |
| **Calibración** (recon drift_tracker) | gate INVERTIDO (corre solo ARMED) | bug declarado §1.5-E | n/a hasta remediar |
| **SED dispatch gate** | stub `todo!()` | `sed-core/src/types/kill_switch.rs:65` | n/a |

**Lectura de arquitectura:** el kill-switch es un **gate de consumo del terminus**, no un apagón de proceso. La garantía fuerte de capital vive en el check por-oportunidad de relays-client (checklist check-1, lectura sin caché) — un opp que llegue después del arm muere en `kill_switch_on` aunque el loop todavía no haya detectado el halt. El halt del loop (K5) es la eficiencia (dejar de trabajar), no la seguridad (la seguridad es K-por-op). Esta distinción es la que el benchmark deja establecida con números.

---

## 7. RIESGOS Y LIMITACIONES DEL DISEÑO

| # | Riesgo | Mitigación |
|---|---|---|
| R-1 | El flap arm/disarm en VPS pausa consumo ~3–5 s × N ciclos (pérdida de labels/sims durante la ventana) | N=10, off-peak, anunciado, cada ciclo < 15 s total; el operador puede abortar (los arms quedan auditados y reversibles) |
| R-2 | `docker logs` timestamps = wall-clock; skew intra-host asumido 0 | documentado; alternativa: correlacionar por entry-id de Redis (ms-seq del propio Redis) |
| R-3 | Caché del api-server TS invalidada por pub/sub hace que su estado sea más fresco que el de los Rust — confusión al interpretar K4 | K4 usa lector de caché fría dedicado; la asimetría pub-sub vs poll se declara (§1.5-B) |
| R-4 | Medir en Windows local con Redis ausente da solo K1 | declarado R8; corrida completa es responsabilidad del operador (Harness B) |
| R-5 | Divergencia fail-open/fail-closed (§1.5-C) hace que "key ausente" mida distinto según gate | el harness SIEMPRE escribe la key (nunca la borra); el caso ausente queda como hallazgo documentado, no como segmento |
| R-6 | Interpretar "<10ms" como promedio para salvar el claim | prohibido por diseño: veredicto por p99/p100 por segmento; media reportada pero no decisiva |
| R-7 | **Baseline "clave ausente":** si `arbx:killswitch` está AUSENTE al empezar, los consumidores YA están halted (fail-closed, `app.toml:7` + lección A5-STALL) y el disarm del bench escribiría `enabled:false`, CAMBIANDO el baseline del sistema (de halt-por-ausencia a running-con-clave) | preflight del Harness B ABORTA con clave ausente (endurecido apply-pass v2, C3); el operador restaura un baseline presente+disabled antes de benchear |
| R-8 | Overhead del instrumento en K4-poll (spawn docker-exec+redis-cli ≈ 5–20 ms/poll) y en K5 (pipeline de logging de docker) infla las cifras | K4 se mide por DOS vías con cotas declaradas (pubsub = piso, poll = techo; nunca se promedian); K5 se declara cota superior y R-2 documenta el reloj |

---

## 8. ESTADO Y VERIFICACIÓN DE ESTE WO

- **Estado:** DISEÑADO + APPLY-PASS v2 (verificación independiente completa — §9). No se escribió código de producción (los harness §3.1/§3.2 son PROPUESTAS íntegras embebidas en este documento, NO aterrizadas), no se ejecutó `cargo` ni `bash -n` (WO design-only), 0 requests públicos, VPS intocado (§32/§33), CERO git (protocolo operador 2026-08-23).
- **Toda afirmación del documento está anclada a file:line del árbol local (rama `a6-cbprom-01`, HEAD `f7db6867`)** — verificada DOS veces por lectura directa (autoría v1 + apply-pass v2); nada inferido ni inventado (RULE 00). El único claim de archivo que la v1 citaba sin existir (`arbx:opps:scored`) fue corregido (C1).
- **Siguiente paso (apply-WO de aterrizaje o operador):** (1) aterrizar `backend/shared-rs/tests/killswitch_latency.rs` y `scripts/bench/killswitch_bench.sh` copiando VERBATIM las fuentes §3.1/§3.2 + `cargo check -p shared-rs --tests` (target caliente §36.4) + `bash -n` del script; (2) correr Harness A (operador en VPS con Redis loopback DB 15, o local si Redis existe); (3) correr Harness B (operador ONLY — §32/§33) off-peak; (4) ejecutar la corrección documental §5 CON las cifras medidas. Ninguna cifra se escribe en docs antes de existir medida que la respalde.

---

## 9. LEDGER DE VERIFICACIÓN INDEPENDIENTE (apply-pass v2 — 2026-09-06, respawn)

Metodología: re-lectura directa de CADA archivo citado por el borrador v1 (las aserciones de agentes NO son facts — regla de memoria del proyecto) + greps de no-existencia re-ejecutados. 51 checks individuales, agrupados abajo. Cero requests, cero SSH, cero cargo, cero git.

| Grupo | Anclas verificadas | Veredicto |
|---|---|---|
| Rust core killswitch (`backend/shared-rs/src/killswitch.rs`) | KEY:15 · CHANNEL:16 · TTL 1 s:61 · connect:54-63 · with_cache_ttl:65-68 · is_enabled:72-77 · state fast-path:81-87 · GET:89 · ausente→default:90-96 · set SET:121/PUBLISH:122 | 10/10 EXACTAS |
| shared-ts (`shared-ts/src/killswitch/index.ts`) | set:54-67 (SET:62, PUBLISH:63) · subscribeChanges:70-84 | EXACTAS |
| api-server (`backend/api-server/src/index.ts`) | cliente+subscribeChanges:56-62 · POST route+zod+before-state:254-262 · set:263-267 · writeAudit+log:268-276 | EXACTAS |
| edge (`edge/worker/src/index.ts`) | POST /admin/killswitch proxy:918-935 · GET status:939 + comentario stale "2s KV cache TTL":938 vs `cf:{cacheTtl:0}`:949 · POST :action:958-983 | EXACTAS (conflicto comentario/código confirmado) |
| UI (`frontend/app/killswitch/page.tsx`) | onToggle token+razón:47-69 · lede "≤ 5 s":75 | EXACTAS |
| selector-api (`backend/selector-api/src/consumer.ts`) | STREAM_IN detected:23 · gate isEnabled:87 · halt None→arm:88-95 · sleep 5000:103 · resume:106-112 · XREADGROUP COUNT 16:126 / BLOCK 2000:127 | EXACTAS (lección A5-STALL :49-53 confirmada) |
| sim-ctl (`backend/sim-ctl/src/consumer.rs`) | STREAM_IN validated:44 · gate:95 · halt:100-116 (transición:96-104) · resume:117-122 · COUNT 8:159 / BLOCK 2000:160-161 | EXACTAS |
| relays-client | STREAM simulated `consumer.rs:21` · EXECUTED:24 · legacy gate+Dropped(kill_switch_on) `submit_engine.rs:346-349` (contexto belt-and-suspenders :339-343) · client SIN with_cache_ttl → TTL 1 s default `main.rs:119-121` · XREADGROUP COUNT 4 `consumer.rs:150-151`/BLOCK 2000:152-153 | EXACTAS ("COUNT ≤16/8/4" confirmado) |
| checklist (`backend/shared-rs/src/pre_execute_checklist.rs`) | check-1 dispatch:214 · impl raw-GET sin caché:263-279 (GET:266, unparseable→bloqueado:272-275, ausente→NO armado:277-278) | EXACTAS (divergencia fail-open/fail-closed §1.5-C confirmada literal) |
| recon (`backend/recon/src/drift_tracker.rs`) | gate INVERTIDO:161-167 (`if !killswitch.is_enabled() { continue }` en :164; comentario :163 declara intención opuesta) | CONFIRMADO — bug real |
| sed-core (`backend/sed-core/src/types/kill_switch.rs`) | `todo!()`:65 (fn check:56-68, doctrina "never caches" :14-16) | EXACTA (stub R8-honest por diseño propio) |
| searcher (`backend/searcher-rs/src/scanner.rs`, `chain_supervisor.rs`) | "runs continuously even if ARMED":1140-1142 · `let _ = killswitch; // reserved`:1243 (dentro de `run_subscription`, fn :1228) · clone sin gate `chain_supervisor.rs:290` y :315-318 | EXACTAS |
| config/infra | `configs/app.toml:7` (fail-closed true) · `compose.prod.yml:65-66` (Redis loopback 127.0.0.1:6379) · `:422-423` (edge 0.0.0.0:8787) · `metrics.rs:74-78` gauge arbx_killswitch_enabled · `shared-rs/Cargo.toml` (paquete :2, deps redis:25/serde_json:14, dev-deps tokio:34 — harness SIN deps nuevas) · container_name explícito SOLO vault/minio/thanos (`:713,753,785…`) | EXACTAS |
| Doctrina | `CLAUDE.md:219-220` quote "<10ms vía API/File/Edge" | EXACTA (anchor precisada por apply-pass) |
| No-existencia (greps re-ejecutados) | `KILLSWITCH_CHANNEL` en `backend/**`: SOLO const :16 + publish :122 — **cero suscriptores Rust** | CONFIRMADO (§1.5-B) |
| No-existencia | vector File (`kill[_-]?file|notify.*kill|kill.*mtime|watch.*kill` sobre `*.{rs,ts,js,sh}`): **0 hits** repo-wide | CONFIRMADO (§1.2) |
| No-existencia | `arbx:opps:scored`: **0 hits** — streams reales detected→validated→simulated→executed | **C1: DEFECTO v1 CORREGIDO** (citado 2×, no existe) |

### Correcciones y completados del apply-pass v2

| # | Tipo | Descripción |
|---|---|---|
| C1 | **Defecto factual corregido** | Borrador v1 citaba `arbx:opps:scored` en §2-K6 y §3.2-K6: el stream NO EXISTE. Corregido a `{validated,simulated,executed}` con anclas de cada const. Violaba RULE 00 (citar infra inexistente). |
| C2 | **Defecto de honestidad corregido** | §3.1 v1 decía "sin Redis local → mide solo K1 + overhead de serde": IMPOSIBLE — `KillSwitchClient::connect` marca la conexión (`killswitch.rs:56`) y no hay cliente sin Redis vivo. Corregido: sin `REDIS_BENCH_URL` TODOS los segmentos son `SKIP(R8) no_computed` (ledger JSON explícito). |
| C3 | **Endurecimiento del runbook** | Preflight v1 abortaba solo con kill ARMED; ahora también con clave AUSENTE (ausencia = halt fail-closed YA activo — A5-STALL + `app.toml:7`; y el disarm del bench cambiaría el baseline, R-7). + `trap EXIT` de disarm de emergencia + `--since` OBLIGATORIO en `docker logs -f` (sin él grep -m1 casaría un halt PREVIO del historial re-emitido). |
| C4 | **Completado de promesa** | §8 v1 prometía aterrizar "con los diffs propuestos" pero §3 solo tenía sketch de comentarios. Ahora §3.1 contiene el archivo Rust ÍNTEGRO (~200 líneas, API-real verificada, sin deps nuevas) y §3.2 el bash ÍNTEGRO (~170 líneas, contenedores resueltos, k6ms/k5_of/disarm_now definidos). Estado declarado: REVIEWED, NOT COMPILED / NOT RUN (design-only). |
| C5 | **Anchor precisada** | §1.5-F y §5.1 ahora citan `CLAUDE.md:219-220` en vez del bare "§9". |

**Veredicto del apply-pass:** el diseño v1 era estructuralmente correcto y honesto (veredicto ejecutivo §0 INALTERADO — "<10ms" sigue siendo indefinible end-to-end con arquitectura poll-based viva); contenía 1 defecto factual (C1), 1 imposibilidad de honestidad (C2) y 1 promesa incumplida (C4). Los tres corregidos. El diseño queda listo para aterrizaje mecánico por un apply-WO con target caliente o por el operador.
