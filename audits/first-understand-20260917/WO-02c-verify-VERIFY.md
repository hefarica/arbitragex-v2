# WO-02c-verify — Reporte de verificación (cs-validator) · 2026-09-17

> Gang Omniscience, oleada verify. Objeto: `audits/first-understand-20260917/02c-SIM-STACK-SELECTOR.md`
> (serie wo02c, tras el diseñador ecc:rust-reviewer). Adendum entregado en §10 del propio archivo.
> Método: Read/Grep SOLAMENTE. 0 cargo/npm/build/anvil, 0 git, 0 mutación VPS,
> 0/5 requests HTTP del presupuesto dominio (la pregunta runtime-VPS ya tenía evidencia
> PRIMARY_SOURCE del orquestador en 02-VPS-REMAP-20260917.md:4-5; re-verla por HTTP
> no agregaba certeza y quemaba presupuesto — fail-honest).

## Dictamen: PASS CON 2 CORRECCIONES

La estructura del reporte del diseñador, el flujo E2E, el veredicto SEL-GATE-01
(código local presente / VPS sin deploy) y los destinos ACTIVO/HUEHUFO/GAP quedan
CONFIRMADOS con evidencia re-abierta. Las correcciones son de estándar de evidencia,
no de wiring.

## 1. Muestreo (requisito ≥8 → ejecutado 18)

17 fichas EXACTAS, 1 exacta-con-matiz, 0 erróneas. 60+ marcadores file:line
re-abiertos, desviación máxima ±2 líneas (p.ej. ON CONFLICT citado :28-36, cláusula
real :35 — dentro de rango). Detalle ficha-por-ficha en §10.1 del adendum
(`02c-SIM-STACK-SELECTOR.md`).

Muestras clave re-abiertas:
- engine.ts:36-39 `producerRejected` + :56-58 wire — SEL-GATE-01 EXACTO.
- consumer.ts (selector) :23-25/:152-193/:208-210/:213 — stream path exacto.
- consumer.rs (sim-ctl) :48-50/:383-483/:496-665/:761-766/:776-784 — exacto.
- sim_encoder.rs :264/:365/:386/:421 — exacto.
- sim_multistep.rs :545 + validate :250-257 — exacto con matiz (ver C1).
- persistence.rs :15/:35/:53-61/:73-88 + 6 familias de tests :181-295 — exacto.
- bayesian.ts: grep inverso → único import = su propio test (HUEHUFO confirmado).
- bellman_ford.rs:40: grep inverso → 0 consumidores externos (HUEHUFO confirmado).

## 2. Traza E2E validar→simular — saltos de wiring Option

**Cero fabricaciones R8.** Todo ausente se declara con typed reason; los `0.0` de
`expected_amount_out`/`gross_profit` (consumer.rs:605-606) son inocentes: el encoder
no los consume (solo fixtures de test, sim_encoder.rs:545-546).

**2 hallazgos NUEVOS del verificador** (no reportados por el diseñador):

1. **Degradación silenciosa de pin (menor, determinismo)**: `block_number` NULL en la
   fila PG → `simulator_for_candidate` (consumer.rs:780-783) devuelve el simulador sin
   pin → la simulación corre contra estado `latest`, no el bloque de detección, sin
   trazabilidad runtime (log/métrica) — el caso None está documentado en el doc-comment
   de la función (consumer.rs:768-775). // WO-02c-FIX-UNPINNED (2026-09-17): acota el
   "SIN log" original (GAP-3 del cross-exam de ESTE verify,
   WO-02c-verify-CROSS-EXAM.md:54-60) — lo ausente es observabilidad runtime, no
   documentación. Consistente con la convención SimulatorV2, pero es el único
   Option-vacío que degrada calidad sin trazabilidad runtime. Candidato a observación
   `sim_consumer.unpinned_replay` (gated, no aplicado).
2. **Doc-drift route_lookup.rs:15**: el header dice "returns None on PG error"; la
   implementación devuelve `Err` en error de PG (doc :108, impl :116-127). El
   COMPORTAMIENTO es el correcto (Err→PEL en consumer.rs:542); el header miente.
   Es exactamente la confluencia Err/None que ghost_verdict (consumer.rs:746-754)
   documenta como anti-patrón. Corrección documental candidata.

## 3. Estado post-SEL-GATE-01 (pregunta 3 del charter)

- Local (fdb40401): el selector **YA NO publica** producer-rejected — prefilter
  engine.ts:56-58 antes del tail, publish accept-only consumer.ts:173-175, test de
  regresión sel-gate01.test.ts con candado de orden del call-site (:89-100) verificado.
- VPS (a06a968d): **SIGUE publicando hasta deploy** (02-VPS-REMAP:4-5). Veredicto §6
  del diseñador correcto y bien clasificado.

## 4. Correcciones emitidas

- **C1 (matiz ficha 3.2)**: "rechaza cualquier flag live" debe leerse: rechazo tipado
  en el path paper (validate :250-257); `paper_mode=false` NO retorna error, delega a
  `verified_simulation::execute` (:554-556) — ruta cheat-free del terminus. El
  invariante de aislamiento §32/§33/§34.3 queda INTACTO (sin cheats fuera de paper
  mode; sim_runner.rs:204-205 fija ambos flags). Coherente con el drift doctrina↔código
  que WO-02d documentó del terminus.
- **C2 (RULE 00, estándar de evidencia §34.5.3)**: la cifra **"87% de sims quemadas en
  rows ya rechazadas" NO es reproducible en la fuente citada**
  (audits/real-cycles-audit-20260916/00-SYNTHESIS.md no contiene 87%; contiene
  "74 sims/9h", "100% rejected", "v3_quote_unavailable 60/76"). El número existe solo
  en sel-gate01.test.ts:5-9 y en la ficha (circular). El fenómeno es canónico (código
  pre-fix + 00-SYNTHESIS), la cifra exacta no tiene artefacto. Acción: archivar el
  query del embudo o re-bajar la cifra (edición gated operador).

## 5. objetivo_usd (RULE 00)

Grep en las 4 piezas = **0 hits** → GAP §7 del diseñador CONFIRMADO. Otros números
citrados verificados exactos (TTL 300s, 120s/60s PEL, 500k/0/300 sim_runner, MAXLEN
10_000, grupos).

## 6. Omisiones (vs lines-per-file.txt)

- selector-api COMPLETO (13/13 — // WO-02c-FIX-G4: era "12/12", off-by-one; ground truth 13
  archivos src no-test, ver errata al pie), simulator-v2 COMPLETO (6/6), sim-ctl **14/16**
  (// WO-02c-FIX-TALLY: era "15/16" — off-by-one, eran DOS sin ficha: route_lookup.rs y
  build.rs; tras WO-02c-FIX-G2 la cobertura es **15/16**, NO 16/16 — build.rs sigue sin
  ficha. La anotación anterior "superseded a 16/16" heredaba la errata aritmética de
  §12.3 del entregable; ver errata TALLY al pie (§10) y §15 del entregable).
- **route_lookup.rs (232 LOC) sin ficha propia** — omisión real, la más pesada: es el
  núcleo del enriquecimiento A3 y su semántica Err/None es material de la traza E2E.
- Menores defendibles: sim-ctl/build.rs (157 LOC), sim-core/src/lib.rs (49 LOC,
  verificado trivial: solo `pub mod` re-exports).
- Nota: bellman_ford HUEHUFO sigue anunciado en `/capabilities` (lib.rs:61).

## 7. Restricciones

Read/Grep only · 0 git · 0 build · 0 VPS-mutation · 0 HTTP · lexicon OMEGA respetado.
Archivos bajo claim tocados: solo `02c-SIM-STACK-SELECTOR.md` (adendum §10) y este
reporte. Sin edición de código de producción (protocolo NO-GIT / diseño gated).

## 8. Handoff a la mesa

- Para el board: WO-02c-verify DONE; ficha 02c queda en estado VERIFICADA (PASS con
  C1/C2). El punto de acople más frágil del stack sim NO es wiring roto sino el
  deploy-debt (DEPLOY-DEBT-01 del diseñador sigue vigente: fdb40401+125b1e0b+dcfe890c
  sin desplegar, VPS=a06a968d).
- Para WO-03/WO-04: si fichan route_lookup.rs o corrigen el doc-header, citar §10.2/§10.5.
- Para cualquier pares que cite el "87%": usar C2 — citar el fenómeno, no la cifra,
  hasta que exista artefacto del embudo.

## 9. ERRATA // WO-02c-FIX-G4 (2026-09-17, fixer gang ronda 1 — GAP-4 del cross-exam)

> Registro append-only. El tally de §6 decía "selector-api COMPLETO (12/12)":
> **off-by-one**. Ground truth re-derivado por el fixer (RULE 00, no heredado):
> `cd backend/selector-api && find src -type f ! -name '*.test.ts' | wc -l` → **13**
> (consumer, index, persistence, policy/{blacklist,engine}, score,
> scoring/{bayesian,engine,factors}, token_safety/{cache,client,goplus,
> internal_heuristic}). Corregido in situ arriba a "13/13" con marcador inline.
> Coincide con `WO-02c-CROSS-EXAM.md:125` y GAP-1 de `WO-02c-verify-CROSS-EXAM.md:32-42`.
> - **La conclusión de cobertura (COMPLETO) SOBREVIVE** — las fichas §5.1–5.9 del
>   entregable cubren los 13 exactos; solo el conteo estaba mal.
> - Origen del "12": NO reproducible desde `lines-per-file.txt` (árbol principal
>   :9141-9161 → 13 no-test; worktree stale :4896-4914 → también 13 no-test) →
>   UNKNOWN, desliz de conteo manual del verificador; el hazard de doble árbol del
>   cross-exam §3 NO lo explica.
> - Ironía registrada (apunta el charter): este mismo verify aplicó el estándar de
>   evidencia reproducible §34.5.3 a la cifra "87%" (C2 §10.4) y no a su propio tally
>   — C2 y esta errata son ahora el mismo precedente en ambas direcciones.
> - Nota de alcance: la parte "sim-ctl 15/16" del mismo §6 quedó anotada arriba como
>   superseded por WO-02c-FIX-G2 (§12.3 del entregable: 16/16 tras fichar
>   route_lookup.rs); su aritmética detallada (14/16 con build.rs sin ficha) fue
>   objeto del GAP-1 del cross-exam y se cierra vía esa anotación, no re-derivada aquí.
> - Espejo de esta errata: nota append-only en §10.5 de `02c-SIM-STACK-SELECTOR.md`.
>   Cero git/cargo/npm/build/VPS/HTTP (0/5 requests).
> - [Reconciliación post-G5, añadida por el fixer G4 — texto original de arriba
>   preservado append-only]: el "16/16" de la nota de alcance precedente quedó
>   REFUTADO por `WO-02c-FIX-TALLY` (errata §10 de este archivo; §15 del entregable):
>   pre-G2 = 14/16, post-G2 = **15/16** (build.rs sigue sin ficha). La cifra vigente
>   de sim-ctl es la de la errata TALLY (bautizada "G5" antes de su renombre para no
>   colisionar con el fixer G5 del GAP-5/SIM_BACKEND), no la que esta errata G4
>   transcribía de §12.3.

## 10. ERRATA // WO-02c-FIX-TALLY (2026-09-17, fixer gang ronda 1 — mitad sim-ctl del tally)

> Registro append-only. Complementa la errata G4 (selector-api 12/12→13/13): la parte
> **sim-ctl** de este §6 era aritméticamente errónea en TRES puntos de la cadena.
> Re-derivado por este fixer (RULE 00, no heredado; find/wc sobre disco, no sobre
> lines-per-file.txt):

- **Denominador 16** = 15 .rs en `sim-ctl/src/` + `build.rs` (árbol principal).
  **Filtro documentado contra lines-per-file.txt**: solo rutas `./backend/sim-ctl/`
  — el artefacto mezcla árbol principal (:9198+) con worktree stale
  `./.claude/worktrees/wf_257a24e1-859-2/...` (:4947-4959, 4 archivos menos y LOC
  desfasadas). tests/ excluido por convención "src no-test" del propio §6.
- **Pre-FIX-G2 la cobertura era 14/16** (sin ficha: route_lookup.rs Y build.rs —
  este §6 original decía "15/16" contando una sola omisión). Corregido in situ
  arriba con marcador.
- **Post-FIX-G2 (ficha §12.1) = 15/16**, no "16/16": build.rs permanece sin ficha
  (menor defendible). La nota G4 de arriba ("superseded a 16/16") y el §12.3 del
  entregable heredaban/creaban esa errata — ambas corregidas in situ con marcador
  `// WO-02c-FIX-TALLY`; errata canónica completa: §15 de `02c-SIM-STACK-SELECTOR.md`.
- sim-core/src/lib.rs es de OTRO crate: fuera del denominador 16 de sim-ctl.
- Cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados: solo este
  reporte + el entregable 02c (ambos reportes del gang, no código de producción).

## 11. ERRATA // WO-02c-FIX-UNPINNED (2026-09-17, fixer gang ronda 1 — GAP-3 del cross-exam de ESTE verify)

> Registro append-only. `WO-02c-verify-CROSS-EXAM.md:54-60` detectó que el hallazgo 1
> de §2 ("la simulación corre contra `latest` SIN log") SOBRESTIMA la falta de
> trazabilidad: el doc-comment de `simulator_for_candidate` (consumer.rs:768-775 — la
> firma citada ":780-783" está debajo del comentario) documenta explícitamente el caso
> None → simulador compartido sin pin ("the shared simulator is reused untouched when
> no pin applies", "typically none — `latest`"). Lo ausente es observabilidad RUNTIME
> (log/métrica), no documentación.

- Corregido in situ arriba (§2, hallazgo 1) con marcador `// WO-02c-FIX-UNPINNED`:
  "SIN log" → "sin trazabilidad runtime (log/métrica) — el caso None está
  documentado en el doc-comment (consumer.rs:768-775)".
- El hallazgo de degradación silenciosa de determinismo SOBREVIVE acotado (no defecto
  R8; sigue siendo el único Option-vacío del wiring que degrada calidad sin
  trazabilidad runtime). Candidato `sim_consumer.unpinned_replay` debug-log sigue
  gated (operador — corrección de código NO aplicada, NO-GIT).
- Errata canónica completa del fix: §16 de `02c-SIM-STACK-SELECTOR.md` (también corrigió
  in situ §10.2 y §12.2 de ese entregable). Eco residual declarado, no tocado:
  `WO-02c-FIX-G2-20260917.md:64` ("sin log" — reporte histórico del par G2).
- Cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados: solo este
  reporte + `02c-SIM-STACK-SELECTOR.md` (ambos reportes del gang, no código de
  producción). Cero .rs tocados.
