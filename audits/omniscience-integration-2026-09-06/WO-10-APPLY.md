# WO-10 — APPLY · Instrumentación latencia detección→broadcast (MN-006, informe §6.11)

- **Fecha:** 2026-09-06 · **Agente:** ecc:performance-optimizer (rubric ecc-rust-patterns) — Gang Omniscience, respawn del apply caído por 429 (muro wf_29b60a15).
- **Diseño:** `WO-10-DESIGN.md` (re-autorado por este reemplazo — el design-agent original murió sin escribirlo).
- **Reglas:** RULE 00 / R8 / R9 · §32/§33 read-only VPS (ni un comando al VPS fue necesario) · NO-GIT (cero commit/push/PR/deploy) · diffs marcados `// WO-10 (2026-09-06)`.

## 1. Qué encontró el reemplazo (fail-honest)

El apply predecesor dejó el árbol con la instrumentación YA aterrizada en los 4 archivos fuente — verificado
con `git status` + `git diff --stat` + relectura completa de cada archivo (no con las notas del caído):

| Archivo claimado | Estado hallado | Delta del predecesor |
|---|---|---|
| `backend/searcher-rs/src/publisher.rs` | COMPLETO | +216 (familia métrica + guard R8 + span XADD + 3 tests) |
| `backend/searcher-rs/src/scanner.rs` | COMPLETO | +115 (t0 tx-on-hand + spans decode + 5 sitios publish legado) |
| `backend/searcher-rs/src/opportunity_emitter.rs` | COMPLETO | +27 (emit_boundary + construction_to_publish ambos paths) |
| `backend/api-server/src/websocket.ts` | COMPLETO (con 1 comentario FALSO) | ventana 60s + percentiles + 2 piernas E2E |
| `backend/api-server/src/websocket-wo10.test.ts` | nuevo, 7 tests | +80 |

Faltaban: el diseño (charter), el reporte, TODA verificación, y un comentario con justificación técnicamente
falsa. Este reemplazo completó exactamente eso.

## 2. Delta aplicado por ESTE agente

1. **WO-10-DESIGN.md** (nuevo): diseño completo con anclas file:line releídas, cadena exporter-vivo,
   promql p50/p95/p99, regla R9, qué NO medir (clock skew cross-container, ventana reconnect §0), diffs reales.
2. **Corrección websocket.ts:26-39**: el comentario decía "prom-client is not resolvable from
   backend/api-server" — FALSO (el propio archivo importa `runtimeAckBroadcastLatencyMs` de `@arbx/shared`
   y le hace `.observe()` en L547+). Razón verdadera documentada: api-server no tiene dependencia directa
   de prom-client (package.json: solo `@arbx/shared "*"`), el histograma debe DEFINIRSE en
   `shared-ts/src/metrics/index.ts` (fuera del file-claim de WO-10) — y Prometheus ya scrapea
   api-server:8080/metrics (prometheus.prod.yml:50-52), así que el follow-up es de 5 líneas sin wiring.
3. **Verificación completa** (§3) — el predecesor no ejecutó NINGUNA.
4. **Board** actualizado (fila WO-10 → APPLIED_VERIFIED).

## 3. Verificación (output crudo)

```
$ cargo check -p searcher-rs          (backend/, target caliente — 1ª tentativa falló: cargo fuera del PATH de git-bash; relanzado con ~/.cargo/bin/cargo.exe)
    Checking searcher-rs v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 39.75s
EXIT=0                                          # 0 warnings

$ cargo test -p searcher-rs --lib publisher
     Running unittests src\lib.rs
    running 3 tests
    test publisher::tests::construction_to_publish_observes_valid_span ... ok
    test publisher::tests::construction_to_publish_rejects_clock_step_and_stale ... ok
    test publisher::tests::pipeline_latency_family_is_registered_in_live_exporter ... ok
    test result: ok. 3 passed; 0 failed; 0 ignored; 1150 filtered out
EXIT=0

$ vitest run src/websocket-wo10.test.ts   (api-server)
  ✓ src/websocket-wo10.test.ts (7 tests) 8ms
  Test Files  1 passed (1) · Tests  7 passed (7)
EXIT=0

$ vitest run websocket                    (no-regresión familia: WO-15 vive en el mismo archivo)
  ✓ websocket-carnot.test.ts (1) · ✓ websocket-wo10.test.ts (7) · ✓ websocket-hot-streamer.test.ts (3)
  ✓ websocket-rooms.test.ts (4) · ✓ websocket.test.ts (2)
  Test Files  5 passed (5) · Tests  17 passed (17)
EXIT=0

$ npm run typecheck                       (api-server, tsc --noEmit)
EXIT=0
```

## 4. Invariantes auditadas

- **RULE 00 (cero mocks):** ninguna observación fabrica datos — serie Prometheus ausente hasta primera
  observación real (lazy deliberado, DESIGN §3.4); pierna TS cuenta `skipped` cuando el stamp falta/no
  parsea (websocket-wo10.test.ts:60-79 lo testeable: no-throw + broadcast preservado); guard R8 del publisher
  testeado por 2 tests Rust.
- **R8 (None≠Some(0)):** `arbx_pipeline_latency_invalid_total` separa "observación inválida" de "no
  computado" y del histograma; diseño explícito.
- **R9 (cero logs por-ítem):** todos los observes R9-silenciosos; pierna TS = 1 summary/60s (ring 8192,
  anclado en la primera observación post-boot para no mentir ventanas vacías tras recreate).
- **Zero-alloc hot-path:** hijos `Lazy<Histogram>` pre-resueltos; `Instant::elapsed` + `observe(f64)`;
  cero String/format en el loop (publisher.rs:88-101).
- **Wire contracts intactos:** payload `new_opportunity` byte-idéntico (testeado); `Opportunity` sin campos
  nuevos (`detected_at` ya existía); `/metrics` sin canal nuevo (misma REGISTRY, DESIGN §3).
- **§32/§33/§34.3:** cero VPS, cero executor/capital/broadcast — la instrumentación es observability pura.
- **NO-GIT:** todo vive como diff local sin commitear en el árbol compartido (riesgo conocido P0-2: se
  pierde en un checkout duro — el operador decide el aterrizaje por PR).

## 5. Límites / follow-ups (declarados)

1. **shared-ts histogram (5 líneas)** para la pierna NOTIFY→WS + `.observe()` en websocket.ts — fuera de
   claim (DESIGN §7.5 con el diff exacto). Hoy esa pierna solo tiene el summary log 60s.
2. **Panel Grafana** en `detection-pipeline.json` con el promql de DESIGN §3.1 — fuera de claim (§7.5).
3. **Producción:** nada corre en el VPS hasta PR+deploy del operador; las series nacen tras el primer
   deploy que incluya este diff (hasta ahí, "no computado" honesto en toda superficie).
4. Lectura de ventanas: aplicar la regla §6.2/6.3 del DESIGN (recreates de flota reinician el ring TS y
   particionan los rate() Prometheus — anclar lecturas lejos de deploys).
