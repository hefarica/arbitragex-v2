# T21 — V6-FINAL · reporte §14 · attempt e291ab43

> 2026-10-13 18:1x–18:3x UTC · todo bajo flock /tmp/arbx-deploy.lock · scripts por archivo (patrón
> tar, cero escapes) · PAPER ON intacto (killswitch=false verificado en cada lectura).

## RESULTADO: **ROLLBACK EJECUTADO POR DISPARADOR §B (p99) — producción estable en binario previo.**
El release quedó PROBADO END-TO-END (boot, swap, motor v4 evaluando en vivo, rollback 3.11s);
la publicación NO cierra hasta que la cola p99 del binario v4 se explique/mitigue o el operador
la acepte con esta evidencia. P9 ⇒ 0 (pasada completa a re-correr).

## (1) F3 RE-DRILL IMAGEN REAL: VERDE (f3_redrill_imagen_real.txt)

- Stack efímero AISLADO (red propia arbx-drill_default, entrypoint sleep ⇒ cero contacto
  redis/pg — slice capitán cumplido). Bajo flock. Timings: **swap1 0.37s → rollback 10.60s →
  swap2 10.67s** (total <22s ≪ W=180s). Digests VERIFICADOS corriendo: a9b3ef50… (R) y
  dfd71012e1dc (previa). Condición (iv) del operador satisfecha CON EL ARTEFACTO. Limpieza 0 restantes.

## (2) V0 pre-swap (v0f_*.json)

searcher dfd71012e1dc running 0-restarts · count=33 viables=0 · histograma {spread_negative 15,
single_pool 6, non_positive 5, v3_pool_not_catalogued 2, +5 razones ×1} · p50=1434 p99=2693 ·
status 7/7 · killswitch=false · deploy.sha=5f8ea3d67 (api-server).

## (3) SWAP ÚNICO: EJECUTADO (swap_ejecucion_v2.txt)

- Mecánica (implementación del "cambiar el tag" del ruling): `docker tag a9b3ef50… arbitragex-v2-searcher-rs:latest`
  + `docker compose --env-file .env -f docker/compose.prod.yml up -d --force-recreate searcher-rs`
  bajo flock. **swap_dur_s=3.38**. Contenedor: image=sha256:a9b3ef50… running restarts=0 (18:23:26Z).
  1er intento con quoting anidado fallió ANTES de tocar nada (estado verificado intacto) —
  corregido con patrón archivo (swap.sh transportado por tar).
- **EVIDENCIA ÚNICA: el motor v4 evaluando EN PRODUCCIÓN** (logs): cartuchos v4 "workbook-derived
  generator" v4.0.0 (mev_04_004…009 con content_hash) + `cartridge.active_eval_summary`
  (pertinent=269, reasons v4: dispatch_needs_route_data 174, operators DATA_GAP, v4_edges_built=2).
  WARN no-fatal: rpc tenderly 429 (failover). Sin panic/fatal.

## (4) V7 POST: Φ=1 ∧ health 7/7 — PERO p99 DISPARÓ §B (2 ventanas)

| check | resultado | §B |
|---|---|---|
| health | 7/7, searcher ok, restarts=0 | ✓ no dispara |
| sha | contenedor image == a9b3ef50… (digest R) | ✓ cambió |
| viables | 0 (baseline 0) | ✓ |
| razón nueva W1 | `gas_floor_breach:own_capital` ×1 — **DISUELTA en W2** (transitoria; vocabulario económico v4; precedente: 3 razones v3 nuevas aparecieron entre t16→t6 SIN deploy) | ✓ no dispara |
| error rate | HTTP 200 en todas las lecturas | ✓ |
| p50 | W1 586 → W2 1414 (V0 1434) — NORMALIZADO | ✓ |
| **p99** | **W1 7885 → W2 6416 vs umbral 1.2×2693=3232 — PERSISTE >2×** en ventana pura post-warmup (18:26:24→18:29:52) | **✗ DISPARA** |
| gᵢ | phi_static 7/7 PASS · 𝒞 canónica rc=0 · Δ deriva=0 | ✓ |

## (5) V8 ROLLBACK: EJECUTADO SIN ESPERAR (v8_rollback.txt)

`docker tag dfd71012e1dc… arbitragex-v2-searcher-rs:latest` + force-recreate bajo flock:
**rollback_dur_s=3.11** — contenedor dfd71012e1dc running 0-restarts, latest restaurada,
status 7/7, wire vivo (ventana mixta p99=4893 se purga en ~5min). Propiedad del operador
cumplida EN PRODUCCIÓN: 1 comando, sin rebuild, sin red.

## (6) QUÓRUM §7 sobre el swap REAL

lock L adquirido (flock en drill+swap+rollback ✓) · α6 𝒞=1 ✓ · α7 NO_REFUTADO ✓ (sin nuevos
ataques; el propio V8 valida F9-rollback) · α8 ℛ filas añadidas ✓ · α9 deriva=0 ✓ ⇒ **6/7;
DONE (Φ ∧ Λ_vps post-swap) NO cerrado** — Λ post-swap se midió (digest R verificado corriendo)
pero el swap NO permanece: el estado final es PRE-swap por guard.

## (7) §E pasada final — notación y declaración

- Delta: **+60.778%** (= +60,78%; JAMÁS separador de miles) — medido (31744−19744)/19744.
- **A1 SUPERSEDE §10**: gobierna +0.60778 MEDIDO; '+0.604' del texto maestro SUPERSEDIDO
  (mismo tratamiento que la prohibición del '+0.636', que SIGUE vigente). Declarado en A1 (t19).
- Φ(paquete)=1 verificado sobre el release ACTIVO durante el swap (evidencia arriba).

## (8) P9: **0** — el hallazgo (p99 de cola del binario v4) activó disparador ⇒ pasada completa
a re-correr cuando la cola se explique/mitigue. El contador queda explícito en 0 por regla.

## Hallazgo técnico para el equipo (hipótesis NO verificada — motivo del rollback disciplinado)

El binario v4 (detector_policy +134 + eval v4 completa por candidato: pertinent 269, 31
operadores, edges) mantiene p50 intacto (1414≈1434) pero degrada la COLA p99 a 2.4× (6416 vs
2693). El detalle por-candidato está en los logs (`cartridge.active_eval_summary` con timing
implícito por tx). Líneas de investigación: profiling de la eval v4 en la cola, batching del
active_eval, o aceptación explícita del operador de la cola nueva (en PAPER sin capital en
riesgo la cola no pierde dinero; en mainnet sí competiría). La decisión es del operador con
esta evidencia; el guard no la tomó por él.

## Q1-Q8 / §D

Q1 escritas solo en t21-evidence + qa-baseline/t21 + ℛ (filas). Q7: VPS tocado SOLO en lo
sancionado (drill efímero, swap, rollback — todo bajo flock). Q8: cero objetivos nuevos (retag
de tags existentes; rollback = estado previo). §D: fallo de quoting anidado ×1 (swap v1) —
corregido con patrón archivo ANTES de tocar nada (estado verificado intacto entre intentos);
grep "error" 5/5 = WARN rpc-429 (no ERROR level), sin panic/fatal.
