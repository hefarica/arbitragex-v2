# BOARD — FIRST-UNDERSTAND · arbitrage-x (orden del operador 2026-09-17, modo OLEADAS)

> /goal: comprender → compilar → clasificar cada pieza. Ficha por pieza con
> op_origen ∈ {1..32, multi-dex} + objetivo_usd. Clasificación ACTIVO/HUEHUFO/MUERTO
> solo con evidencia. 8 gates al final. Prohibido sentenciar sin ficha.
> Modo: OLEADAS acotadas — cada oleada entrega evidencia y para a informe.

## WO-01 · Inventario maestro — ✅ DONE (2026-09-17)
- Owner: orquestador (script bash, 0 LLM).
- Evidencia: tabla crates/packages con LOC (ver 01-INVENTARIO.md).
- Gate: conteos reproducibles con `git ls-files`.

## WO-02 · Fichas hot-path (searcher-rs, sim stack, selector, relays-client) — 🔄 IN_PROGRESS (2026-09-17, gang)
- Owner: gang (oleadas de 4, respawnBudget 8).
- Claims: solo lectura; mapeo op_origen desde registry canónico (no re-derivar 8.184 relaciones).
- Gate: cada ficha con ruta+firma+prueba (línea exacta) o marcada GAP.
- **RESTRICCIÓN EXTRA ESTE RUN**: PROHIBIDO ejecutar cargo/npm/build de CUALQUIER tipo —
  el orquestador corre `cargo check --workspace` en el target/ compartido (§36.4 serie total).
  Análisis por lectura/grep SOLAMENTE.
- Sub-alcance por especialista (reparto de mitades):
  - WO-02a: searcher-rs (193 archivos) — módulos src/*: workers, route_discovery, cartridges/runner, impacto. Entrypoints bin.
  - WO-02b: searcher-rs/src/operators + math-engine — ficha de los 31-32 operadores con op_origen ∈ {1..32} desde `backend/math-engine/src/operators/` y registro en searcher.
    - **✅ DONE (2026-09-17)** → reporte: `02b-OPERADORES-MATH-ENGINE.md` (ficha 32 ops + adendum
      de verificación) + `WO-02b-DESIGN.md`. Hallazgos clave: (1) `searcher-rs/src/operators/`
      NO existe (GAP del charter — los ops viven SOLO en math-engine, consumidos vía crate);
      (2) 32 registrados (id 32 = op_32_nsga2) vs matriz canónica 264×31 congelada por test;
      (3) op_32 NSGA-II = único con efecto decisorio (live_risk_ranker, generations=0
      selección-only); (4) objetivo_usd = GAP (RULE 00); (5) toggle math-engine sin auth =
      riesgo de superficie, NO de hot-path. Gaps corregidos post-verify por el gang:
      errata op_22 p/s (`WO-02b-FIX-20260917.md`), contraste math_map.json 264/264
      (`WO-02b-FIX-MATHMAP-20260917.md`), adendum de alcance del verify
      (`WO-02b-FIX-VERIFY-20260917.md`).
  - WO-02c: stack sim (simulator-v2, sim-core, sim-ctl) + selector-api — fases validar/simular/seleccionar.
    - ✅ DONE (2026-09-17, ecc:rust-reviewer): fichas en 02c-SIM-STACK-SELECTOR.md + cierre en WO-02c-DESIGN.md.
      Hallazgos clave: SEL-GATE-01 presente local (fdb40401) pero NO desplegado (VPS=a06a968d, ver 02-VPS-REMAP);
      HUEHUFO simulator-v2/bellman_ford.rs + selector-api/scoring/bayesian.ts; objetivo_usd = GAP en las 4 piezas;
      paper_mode invariante intacto (§34.3 OK).
    - **// WO-02c-FIX-G1 (2026-09-17, fixer gang ronda 1 — GAP-1 del cross-exam)** —
      la corrección C2 (cifra "87% de sims quemadas" no reproducible) fue declarada por
      el verify (§10.4) pero NUNCA aplicada al cuerpo del entregable. APLICADA ahora:
      02c-SIM-STACK-SELECTOR.md §5.3 (:171) y §9.4 (:231) re-bajan la cifra a
      "fracción mayoritaria NO cuantificada (74 sims/9h S4, detección 100% rejected)"
      con nota inline de circularidad vs sel-gate01.test.ts:5-9 y referencia a C2 §10.4;
      errata append-only §11 registra ambos cambios. Re-verificación RULE 00 del fixer:
      grep "87%|funnel 2026-09-17T04:00Z" en real-cycles-audit-20260916/ = 0 hits
      (00-SYNTHESIS.md:24 = PRIMARY_SOURCE del reemplazo). Header del test =
      operator-gated, NO tocado. Precisión: el cross-exam citaba "§5.3 y §6" — §6 no
      contenía "87%" (imprecisión menor documentada). Reporte:
      `WO-02c-FIX-G1-20260917.md`. Cero git/cargo/npm/VPS/HTTP (0/5).
  - WO-02d: relays-client (terminus §34.3: live_exec_policy default-deny) + shared-rs tipos canónicos.
    - **✅ DONE (2026-09-17)** — `02d-RELAYS-CLIENT-SHARED.md` (fichas: 19 relays-client
      + shared-rs a nivel módulo-tipo — 24 archivos .rs en src/, 25 crate-wide con tests/;
      // WO-02d-FIX-G1 (2026-09-17): la cifra original "52 fichas: 19 + 33 shared-rs"
      era NO reproducible, corregida a los conteos `wc -l` reales, ver
      `WO-02d-FIX-G1-20260917.md` y ERRATA-01 de la ficha) + `WO-02d-DESIGN.md`. Hallazgos clave: (1) `MainnetRefused` NO existe
      como variante Rust — es default-deny + allowlist-default-Sepolia
      (live_exec_policy.rs:3,37,41-52); mainnet explícito SÍ soportado por código
      (test :71-76) → drift doctrina↔código documentado, remediación documental propuesta
      (gated, no aplicada). (2) docs/EXECUTION_MODES_DOCTRINE.md AUSENTE (GAP). (3)
      op_origen del terminus = multi-dex restringido a 8 cartuchos MEV-01 + 3 familias
      base (plan_validation.rs:17-32). Cero cargo/git/VPS-mutación (restricción cumplida).
- **WO-02a ✅ DONE (2026-09-17)** → reporte: `WO-02a-DESIGN.md`. Hallazgos clave:
  (1) src/impacto/ NO existe → real es impact_index.rs; (2) 01-INVENTARIO.md ausente
  (DISCREPANCIA confirmada vs este board WO-01); (3) workers legacy (triangular/flashloan/
  liquidation) default-OFF post-Phase-15 = HUEHUFO; execution_worker+hft_mempool_listener
  = MUERTOS (nunca spawneados); (4) RouteDiscoveryMode es typestate SIN variante Active
  (gate más fuerte del repo); (5) objetivo_usd = GAP, solo min_net_bps=5.0 (QUOTEBASE-264)
  y gas-safety floor reales; (6) 3 diseños D-1..D-3 con diffs exactos, NO aplicados.
  Punto de acople para WO-02b: cartridge_boot.rs:972 (Arc<OperatorRegistry>) +
  math_map.json (op_origen canónico, no re-derivar).
- **ERRATA-WO-02a (2026-09-17, fixer gang)** — procedencia falsa del diff PancakeV3 en
  `route_intent.rs`: la entrada anterior endosa `WO-02a-DESIGN.md`, cuyos §0/§3 y el
  adendum del verify §7 atribuyen el diff working-tree (PancakeV3→Unknown) al commit
  `dcfe890c`. REFUTADO con git: `git show dcfe890c --name-only` toca SOLO
  `backend/shared-rs/src/chains.rs` + `backend/sim-ctl/src/tx_builder.rs`; `git log --
  route_intent.rs` último commit = `a37bcbfc`. Reclasificación canónica de la mesa:
  **diff huérfano de working-tree, NO commiteado, autoría SIN confirmar (hipótesis:
  operador follow-up de PANCAKE-ROUTER-01 — el comentario interno del diff dice
  "PANCAKE-ROUTER-01 follow-up"; mtime verificado 2026-09-17 01:36:02 -0500)** — consistente
  con WO-05 (línea "Incluye el diff huérfano route_intent.rs"). Evidencia recomputable:
  `git status --porcelain -- backend/searcher-rs/src/route_intent.rs` (M), `git diff`
  (+7/−1: mapeo :275 + comentario + test). Erratas de detalle PENDIENTES en los archivos
  del claim (WO-02a-DESIGN.md §0/§3/§10-D1, WO-02a-verify-VERIFY.md §7 — asignadas al fixer
  de esa mitad, append-only; al momento de esta errata del board aún NO aplicadas). Contaminación residual
  documentada (NO tocada, archivos de otros owners): INFORME.md:63 (REPARO-1 atribuye el
  diff a "dcfe890c olvidó este From") y WO-02a-verify-CROSS-EXAM.md:35 (mismo error del
  cross-examinador que originó esta errata).
- **ERRATA-2 WO-02a APLICADA (2026-09-17, fixer gang, ronda 1 — `WO-02a-FIX-R2-20260917.md`)** —
  fuente fantasma `configs/router_kinds.json`: NO existe en el repo (find negativo) y
  `router_to_protocol` es campo muerto (`#[allow(dead_code)]` impact_index.rs:221-222, init
  vacío :244, 0 lectores — `resolve` saca los protocolos del registro de pools :503). El §7
  del DESIGN marcó la fuente 4 como MUERTA/NO-IMPLEMENTADA y D-1 quedó REESCRITO (D-1-rev2,
  append-only): la propagación PancakeV3→ProtocolType::V3 vive en el decoder
  (`calldata/pancake.rs` nuevo + brazo de dispatch calldata/mod.rs:101-108; sin decoder NO
  hay RouteIntent — el swap muere con `UnknownRouter`, no llega degradado al orchestrator),
  con gate de verificación del decoder en calldata/. Bonus: fuente 5 `selector_to_decoder`
  también dead_code (:226-227). dcfe890c confirmado SIN configs de routers (git show --stat).
  Pendiente aún (owner verificador): adendum en WO-02a-verify-VERIFY.md fila §7; corrección
  del doc-comment impact_index.rs:21-22 (gated operador, recomendada en E-1).
- Formato ficha (por pieza o módulo si el archivo es trivial): ruta | firma entrada→salida |
  misión (fase pipeline) | op_origen o multi-dex | dependencias (grafo inverso grep) |
  estado muta/puro | prueba (file:line exacta) | destino preliminar ACTIVO/HUEHUFO/GAP.
- objetivo_usd: si no existe fuente real en el repo (config/env/registry), marcar GAP —
  PROHIBIDO inventar blancos (RULE 00).
- **⚠ ERRATA NUMÉRICA WO-02a (2026-09-17, fixer respawn-B)** — valores canónicos corregidos
  en `WO-02a-verify-VERIFY.md` §"ERRATA NUMÉRICA append-only": src .rs = **178 arch /
  90,401 LOC** (123.7K era 184 entradas con fixtures JSON); workers = **23 arch /
  14,797 LOC** (22/14,624 excl. provenance/tests.rs); route_discovery = **16 arch .rs /
  11,488 LOC** (17/11,610 incl. README.md). **02b/02c/02d y WO-06 deben citar ESTOS
  números** con convención declarada (fixtures/README/tests) — prohibidos los valores
  previos. Ninguna conclusión estructural cambia.
- **CONVERGENCIA DUAL (2026-09-17, fixer respawn-A)** — recompute independiente
  (find+wc disco Y lines-per-file.txt no-worktree, §8.4 del VERIFY con comandos awk/wc
  reproducibles) confirma los mismos valores canónicos: 178/90,401-90,395 · 23/14,797 ·
  16 .rs/11,488. Los números de la errata quedan DOBLE-VERIFICADOS por dos fixers
  paralelos. Nota fail-honest adicional: el charter original del fix citaba "23 arch /
  14,624" — el 14,624 excluye `provenance/tests.rs` (173); el valor full-inclusive
  reproducible es 14,797 (ya declarado arriba como variante).
- **// WO-02a-FIX-G3 (2026-09-17, fixer gang ronda 1)** — errata de premisa + fuente
  aplicada (`WO-02a-verify-VERIFY.md` §"WO-02a-FIX-G3", append-only): (1) premisa §3
  "TODO engines/ tras experimental-gate" REFUTADA — engines/mod.rs:25-28 (4 estables)
  + :31-33 (3 "Task 3") compilan SIN gate; solo 7 módulos en :38-52 están gated (nota:
  el cross-exam dijo "8 motores" — miscount, corregido). La conclusión de valores
  SOBREVIVE: 21/22 matches `min_profit_usd` en engines gated + 1 en `#[cfg(test)]`
  (triangular_engine.rs:871, bloque :843-845); CERO en los 7 módulos ungated. (2)
  "15 fichas (≥8 exigidos)" §7 = requisito SIN FUENTE (ningún charter/board fija
  mínimo 8) — reclasificado como muestreo autoimpuesto; PASS 15/15 no cambia.
  Contaminación residual misma fórmula en archivos WO-02c (02c-SIM-STACK-SELECTOR.md:241/:363,
  WO-02c-verify-VERIFY.md:17) — documentada, NO tocada (owner 02c). (3) Componente
  numérico del charter ya corregido por pares, re-verificado: 178 arch / 90,401 LOC.

- **ERRATA WO-02b APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX-20260917.md`)** —
  claim falso en 02b-OPERADORES-MATH-ENGINE.md §2 celda op_22 ("TODOS los cartuchos lo
  declaran secondary"): medido independiente sobre los 264 .rhai = op_22 PRIMARY en 16,
  secondary en 248 (unión 264 SÍ era correcta). Columna deps completa armonizada a
  convención `C(Xp+Ys=Z)` split primary/secondary dedup-union (op_05 22p/2s, op_07 28p/2s,
  op_08 131p/47s, op_21 176p/2s, op_17 1p/0s, resto ver reporte). Splits recomputados por
  parser regex (RULE 00, concuerda con cross-examiner), overlap p∩s = 0 en todos los ops,
  conteos de unión originales todos correctos. Verificación post-edit: 46 tokens del doc
  vs ground-truth = 0 discrepancias. Contaminación del claim en otros archivos: 0.
  La distinción p/s es load-bearing (STRAT-IDENT-01, cartridge_boot.rs:1003-1020).
  Errata §8 append-only dentro del propio 02b; cero git/cargo/VPS.
- **// WO-02b-FIX-MATHMAP (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX-MATHMAP-20260917.md`)** —
  gap de respaldo canónico cerrado: math_map.json (manifiesto op_origen, punto de acople de
  02a) nunca contrastado por 02b (derivó por grep de .rhai; excusa temporal §0: 02a publicado
  después). Recómputo independiente del fixer: **264/264 concordantes primary_ops ↔
  primary_operators** (0 mismatches, 0 order-diffs, biyección mev_id limpia, manifiesto sin
  duplicados ni entradas vacías) — confirma el 0/264 del cross-examiner (cuyo reporte NO
  está en disco; claim por charter, recomputado no heredado, RULE 00). Párrafo canónico
  insertado en §3 de `02b-OPERADORES-MATH-ENGINE.md`. Nota de alcance fail-honest: el
  manifiesto NO tiene campo secondary → la mitad "s" de los conteos §2 sigue respaldada SOLO
  por .rhai (respaldo doble cubre solo "p"). Cero git/cargo/VPS/HTTP.
- **// WO-02b-FIX-VERIFY (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX-VERIFY-20260917.md`)** —
  adendum de alcance agregado al dictamen del verificador (en `02b-OPERADORES-MATH-ENGINE.md`
  §VERIFICACIÓN y espejo en `WO-02b-verify-VERIFY.md`): el "13/13 PASS, 0 correcciones de
  fondo" verificó matemática y líneas pero NO re-derivó los conteos de la columna deps ni
  contrastó math_map.json. Dos precisiones omitidas registradas: (1) celda op_22 con claim
  falso no detectado por el muestreo (corregido antes por WO-02b-FIX: 16p+248s=264,
  re-derivado de nuevo por este fixer, concuerda); (2) ausencia del contraste math_map.json
  (cerrado antes por WO-02b-FIX-MATHMAP: 264/264, re-ejecutado con
  `reconcile_math_map_vs_rhai.py` = IDENTICO). Veredicto PASS sobrevive ACOTADO a lo
  muestreado. **WO-02b marcado ✅ DONE** (ver sub-alcance arriba). Cero git/cargo/VPS/HTTP.
- **// WO-02b-FIX2 (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX2-20260917.md`)** —
  CONVERGENCIA DUAL del gap math_map: este fixer (despachado en paralelo con
  WO-02b-FIX-MATHMAP, mismo charter) recomputó INDEPENDIENTEMENTE la reconciliación
  primary_ops (manifest) ↔ primary_operators (.rhai) emparejados por mev_id:
  **264/264 idénticos, 0 mismatches, 0 huérfanos, 0 duplicados** (script persistido:
  `reconcile_math_map_vs_rhai.py`, exit 0). Añade dos verificaciones no cubiertas por el
  bloque §3 del par: (a) igualdad de VALOR Y ORDEN (0 order-diffs); (b) conteos primary
  per-op del manifiesto == columna "p" del ERRATA §8 (0 diffs, 21 ops con primary>0; los
  9 ops nunca declarados tampoco son primary en el manifiesto). Adenda §9 append-only en
  `02b-OPERADORES-MATH-ENGINE.md` con cita canónica WO-02a-DESIGN.md:246-250/:468/:475 y
  nota de alcance (manifiesto sin campo secondary → mitad "s" respaldada solo por .rhai).
  Dato colateral: `mode` del manifiesto = 160 SHADOW + 104 PAPER (0 ACTIVE). Fail-honest
  detectado en mesa: el reporte `WO-02b-FIX-VERIFY-20260917.md` citado por board:32/:138
  y por el adendum §VERIFICACIÓN de la ficha NO está en disco al cierre de este fix
  (aserción de agente ≠ fact — precedent memory). Cero git/cargo/VPS/HTTP (0/5 requests).
- **// WO-02b-FIX-N1 (2026-09-17, fixer gang ronda 1 · respawn-A — `WO-02b-FIX-N1-20260917.md`)** —
  discrepancia interna op_15 RECONCILIADA: ficha del diseñador decía bracket "[0, r_in]"
  (02b línea 100) mientras código+verificador dicen "[0, r0]" (op_15_golden_section.rs:113-115;
  fila PASS §VERIFICACIÓN) — `r_in` NO existe como símbolo en ningún fuente (grep 0 matches,
  solo falsos positivos substring). El verificador había corregido EN SILENCIO; errata
  §10 append-only en 02b flaggea la divergencia (el flag faltante). Ficha unificada in situ
  a "[0, r0], r0 = reserva del token de entrada, liquidity_reserves[0].0 :85, bracket
  :113-115" (opción 1 del hint; alias `r_in := r0` descartado para no preservar notación
  huérfana). Cero cambio semántico (el bracket ejecutable siempre fue [0, r0]; respaldo:
  tol `1e-9*r0` :120, metadata "r0" :165, vector_result [x*,f*,r0] :172, test op_15
  "x* ∈ (0, r0]"). Cero .rs tocados. Nota drift charter: citaba "02b line 91", fila real
  = línea 100 (ediciones append-only previas). Cero git/cargo/VPS/HTTP (0/5 requests).
- **// WO-02b-FIX-FMT (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX-FMT-20260917.md`)** —
  desviación de formato de ficha vs gate del board (columnas "misión (fase pipeline)" y
  "estado muta/puro" por operador ausentes en §2 de 02b) DECLARADA como desvío tolerable
  en el header de §2 de `02b-OPERADORES-MATH-ENGINE.md`: firma uniforme por trait sellado
  (mod.rs:97-114) hace la misión = columna transformación, la fase pipeline es transversal
  a los 32 (mismas 4 vías de consumo §3) y la pureza está cubierta globalmente en §4
  (32/32 PUROS). Reporte del cross-examiner NO está en disco (cita de charter, fail-honest).
  Cero .rs tocados; doc-only. Cero git/cargo/VPS/HTTP (0/5 requests).

- **// WO-02b-FIX-DRIFT-COMPOSE (2026-09-17, fixer gang ronda 1 — `WO-02b-FIX-DRIFT-COMPOSE-20260917.md`)** —
  drift documental no flaggeado por §5 de la ficha: `docker/compose.prod.yml:197-198`
  comenta "Serves the 31 topological operators (…264×31 matrix projection)" pero el
  registry que sirve ese service registra **32** (ids 1..=32, op_32_nsga2 en
  `operators/mod.rs:172`; `OPERATOR_COUNT=32` :57). El §5 citaba el archivo (:196,
  publish loopback) y no cruzó los conteos que la propia ficha censaba (§1, DESIGN §4.1,
  VERIFICACIÓN (1)). Precisión: el "31" del comentario es correcto SOLO para la matriz
  de proyección canónica (`COLS=31` congelado por test api.rs:437-446) — el comentario
  mezcla census registry (32) con columnas de matriz (31), asimetría deliberada ya
  documentada en 02b §1:30. Origen git: comentario nace con 31 ops (`c7ad837e`
  2026-07-28); op_32 registrado después (`95c69d47` 2026-09-11) sin tocar el comentario.
  Impacto runtime CERO (comentario YAML). Fix aplicado (documental): bullet de drift en
  §5 de `02b-OPERADORES-MATH-ENGINE.md` + errata §11 append-only. Corrección del YAML
  (s/31→32/ en :197, dejando intacto el 264×31) SOLO vía PR gated — NO aplicada.
  Contaminación cruzada = 0 (ningún otro reporte endosa el "31 operators"). Cero
  git/cargo/VPS/HTTP (0/5 requests).

## WO-03 · Fichas backend de soporte (math-engine, semiotic-bridge, shared-rs, sed-core, recon, token-enricher, prioritization-spine, mcp-sim-engine)
- Owner: gang. Gate: ídem WO-02.

## WO-04 · Fichas TS (frontend, edge/worker, api-server, selector-api, shared-ts)
- Owner: gang. Gate: ídem WO-02.

## WO-05 · Compilación forzada (cargo check workspace dev-profile local + lista roja) — ✅ DONE (2026-09-17)
- Owner: orquestador. Nota: AppControl 4551 → release solo en VPS. Reparos = OTRO WO con diff, gated por operador.
- **RESULTADO: LISTA ROJA = ∅.** `cargo check --workspace` (dev, 15 crates) exit 0, sin errores.
  Incluye el diff huérfano route_intent.rs (PancakeV3→Unknown). Piezas MUERTAS por compilación: 0.

## WO-06 · Gates 1-8 (fmt, clippy, test, criterion, npm build, compose, curl /api/translate, healthcheck)
- Owner: orquestador vía ssh arbx (VPS) para compose/curl/health.
- Evidencia requerida: salida cruda por gate. /api/translate verificado EXISTENTE (backend/semiotic-bridge/src/api.rs).

## Reglas de esta mesa
- Rust SIEMPRE serie total (target/ compartido, §36.4).
- Cero commit/push/PR/deploy sin gate final del operador.
- FAIL-HONEST: fase con 0 resultados = reportada como 0, nunca maquillada.
- **// WO-02d-FIX-G1 APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02d-FIX-G1-20260917.md`)**
  — DEFECTO 1 del cross-exam de 02d (`WO-02d-verify-CROSS-EXAM.md` §2): conteo
  inflado "33 archivos / 33 módulos" en shared-rs. Recomputado RULE 00:
  `find shared-rs/src -name "*.rs" | wc -l` = **24** (incluye
  oracle_snapshot/configured_rpc.rs y lib.rs) · 25 crate-wide (+
  tests/oracle_rpc_selection.rs) · tabla §3 = 19 filas → 23/24 src (lib.rs solo
  citado :11). Corregido in situ con marcador en §3 header + §6 de
  `02d-RELAYS-CLIENT-SHARED.md` + errata §ERRATA-01 append-only + este board
  (:51-52 arriba). "19 relays-client" re-verificado CORRECTO. Contaminación
  residual en archivos de otros owners documentada y NO pisada:
  WO-02a-verify-VERIFY.md:16/:309 ("52 fichas"), WO-02c-CROSS-EXAM.md:68
  ("los 33 shared-rs"); WO-02d-verify-CROSS-EXAM.md:43 intacto (cita el 33 como
  evidencia del defecto). DEFECTO 2 (TTL 300≠60 §4) queda pendiente para su
  propio fixer — NO tocado aquí. Cero código/git/cargo/npm/VPS/HTTP (0/5).
  **CONVERGENCIA DUAL (2026-09-17, fixer despachado en paralelo con el mismo
  charter — `WO-02d-FIX-G1-CONVERGENCE-20260917.md`)**: recompute independiente
  (find + diff de listas) confirma 24 src / 25 crate-wide / 19 relays-client y
  lista de archivos byte-idéntica a §ERRATA-01; auditoría grep post-fix =
  CERO ocurrencias vigentes de "33"/"52 fichas" en superficie WO-02d; CONCUERDA
  con eliminar la suma en vez de escribir "43 fichas" (fichas ≠ archivos —
  escribir 43 fabricaría un nuevo tally, RULE 00). Gap G-1 DOBLE-VERIFICADO.
- **// WO-02d-FIX-G2 APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02d-FIX-G2-20260917.md`)**
  — DEFECTO 2 del cross-exam de 02d (`WO-02d-verify-CROSS-EXAM.md` §3, gap
  "agent-fixable" de su tabla §5): fila §4 de `02d-RELAYS-CLIENT-SHARED.md` decía
  "Key `arbx:validated_plan:<id>` (TTL 300 s)" con columna productor listando 3
  entidades. CORREGIDO in situ con marcador + ERRATA-03 append-only. Realidad
  re-verificada de primera mano (RULE 00): escritores = **2** — scanner.rs:2458
  (`set_ex(..., 300u64)`, key :2449, fail-SOFT) y candidate_simulation.rs:585
  (`.arg("EX").arg(60)` en pipe atómico :579-592, llamado desde
  opportunity_emitter.rs:367; el mismo pipe escribe `validated_economics` EX 60).
  `canonical_plan_consumer.rs:41` = **CONSUMIDOR** (GET :81-84; su header :4 cita
  "TTL 300s" refiriendo al productor scanner — NO escribe): el hint del charter
  listándolo como titular del TTL 300 fue DESVIADO documentadamente (reproduciría
  el mismo error de clase productor/consumidor). Precisión de citas: el `60` vive
  en :585 (la ":586" del cross-exam apunta a `.ignore()`), el `300` en :2458.
  Consecuencia material documentada: ventana detect→broadcast de la ruta
  candidate = **60 s** (no 300 s) — si el terminus queda >60 s detrás sobre ese
  productor, el plan expira y el fail-closed dropea (SEGURO; fail-closed INTACTO).
  Consumidores del key: relays-client submit_engine.rs:161/:442-484 Y sim-ctl
  canonical_plan_consumer::fetch (misma ventana 60 s en ambos). Contaminación
  residual en archivo ajeno documentada, NO pisada: 02c-SIM-STACK-SELECTOR.md
  afirma "TTL 300s" en :30/:127/:258/:326 (owner 02c — registrado para su propio
  fixer). Sin colisión con el fixer paralelo WO-02d-FIX-GRAN (ediciones disjuntas,
  marcadores intactos; ERRATA-02=GRAN, ERRATA-03=este fix). Cero código, cero
  git, cero cargo/npm/build, cero VPS, cero HTTP (0/5).
- **// WO-02a-FIX-G1 APLICADA (2026-09-17, fixer caído + reemplazo-A respawn)** —
  G1 del cross-exam CERRADO en tres capas: DESIGN §9.1 (FIX-R1, re-stringe "solo
  RELATIVOS": piso USD absoluto `trading_config.min_profit_usd` + 4 knobs workbook
  declared-only + reconciliación 02d:212/§5 y 02b:148), VERIFY §10 (FIX-G2, tabla
  reconciliación 10.3) y `// WO-02a-FIX-G1` (VERIFY:486 + espejo DESIGN §9.2) con la
  PRECISIÓN material: scanner.rs:2568 puebla `min_profit_threshold` pero
  `score()` NUNCA lo lee (scoring.rs:14 decl, :25-26 único gate net<=0; grep backend
  = 2 hits, cero lectores) — el piso VIVO gatea por `config_aware.rs:150 →
  risk_engine.rs:47-48` (+ override per-estrategia trading_config.rs:156 →
  strategy_config_gate.rs:302 → scanner.rs:2310-2319). El fixer original murió tras
  aterrizar los adenda y ANTES de escribir su reporte: el REEMPLAZO A lo completó
  (`WO-02a-FIX-G1-20260917.md`) tras re-verificar TODAS las citas byte-exact, y
  documenta imprecisión residual menor: helper `effective_min_profit_usd` (:622) tiene
  0 call-sites productivos — el override se consume vía el campo :156 directo. GAP
  objetivo_usd POR ESTRATEGIA en searcher-rs: INTACTO (sobrevive). Cita canónica para
  gates 1-8/WO-06: `config_aware.rs:150 → risk_engine.rs:47`, NO scanner.rs:2568.
- **ERRATA G2 verify-02a APLICADA (2026-09-17, fixer ronda 1 —
  `WO-02a-verify-FIX-G2-20260917.md`)** — el §0 del WO-02a-verify ("02d no toca mi
  claim... sin contradicciones") queda REFUTADO y superseded por errata append-only
  §10 en `WO-02a-verify-VERIFY.md`: 02d:212/§5:245-258 SÍ tocan el claim
  (capital_usd :183 cap 2 % execution_admission.rs:193-201, max_value_eth 1.0 ETH
  config.rs:156-158, piso min_profit_usd scanner.rs:2568) y 02b (G1, línea real
  :154 — drift ":148" del cross-exam corregido) + 02c (§7:209-211) publicados a las
  01:06 no fueron citados. Reconciliación canónica para gates 1-8: GAP
  objetivo_usd POR ESTRATEGIA/OPERADOR (02a/02b/02c) COEXISTE con blancos USD
  runtime GLOBALES reales (02d §5) y piso USD absoluto vivo en scanner — no son
  contradictorios, dominios distintos. Regla nueva de mesa: contra-citar pares por
  archivo:línea antes de declarar "sin contradicciones". Nota: el fixer G3
  resolvió en paralelo la premisa engines-gate (§3) en el mismo archivo — ambas
  erratas conviven append-only (:297 y :403).
- **// WO-02c-FIX-G2 APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-G2-20260917.md`)**
  — GAP-2 del cross-exam de 02c CERRADO: ficha de `backend/sim-ctl/src/route_lookup.rs`
  (232 LOC, "OMISIÓN REAL la más pesada" según verify §10.5) escrita como §12.1 (ficha
  4.9) en `02c-SIM-STACK-SELECTOR.md`, y los hallazgos del verify SUBIDOS a la matriz
  §8 (filas aditivas): doc-drift route_lookup.rs:15 (header "None on PG error" vs Err
  real :116-127; comportamiento correcto, Err→PEL consumer.rs:542) + degradación
  silenciosa de pin (block_number NULL → replay unpinned contra latest, consumer.rs:608
  + simulator_for_candidate :776-784; candidato `sim_consumer.unpinned_replay`, gated).
  Hallazgo NUEVO del fixer anexado (tercera fila): doc-drift :159-162 cita
  `sim-ctl/tests/route_lookup_integration.rs` que NO existe — cobertura real =
  `simwire02_route_aware.rs:36-37,138` (drift de puntero, no de cobertura). sim-ctl
  pasa a 16/16 fichado. Semántica Err-PG vs Ok(None)-ausente documentada en la ficha
  (Ok(None) fila ausente :130 O metadata vacío :132-134; Err :116-127). Correcciones
  de código = gated operador, NO aplicadas. Colisión de sección con el fixer G1
  (anexó §11 en paralelo) resuelta renumerando a §12 — cero contenido ajeno tocado.
  Cero git/cargo/npm/build/VPS/HTTP (0/5 requests).
- **// WO-02c-FIX-G5 APLICADA (2026-09-17, fixer gang ronda 1 — GAP-5 del cross-exam de 02c)**
  — reclasificación del claim "default SIM_BACKEND=anvil": era presentado como verdad
  RUNTIME siendo solo verdad de COMPOSE. Reclasificado **compose=CANONICAL_REPO ·
  runtime VPS=INFERRED/UNKNOWN**: el bloque sim-ctl de compose.prod.yml no define
  SIM_BACKEND en `environment` (:152-157) PERO monta `env_file: ../.env` (:150-151)
  que puede definir SIM_BACKEND=revm y sobreescribir el default del binario
  (main.rs:661-664; B2c exige revm :734); 02-VPS-REMAP solo hizo `stat` del .env
  (:12), no leyó su contenido. Cambios (todos marcados `// WO-02c-FIX-G5`): nota en
  ficha 4.6 (:142) + fila matriz §8 (:221) + hallazgo 7 de WO-02c-DESIGN.md (:22) +
  errata §13.3 append-only con evidencia re-verificada. Clasificación load-bearing:
  si el .env dijera revm, B2c carrier-first activo y G3 (dedup parcial revm) pasa a
  ser el camino caliente. Comando read-only propuesto para WO-06/operador:
  `ssh arbx grep -c '^SIM_BACKEND=' /opt/arbitragex-v2/.env`. Cero git/cargo/npm/
  build/VPS/HTTP (0/5 requests). Archivos tocados: 02c-SIM-STACK-SELECTOR.md +
  WO-02c-DESIGN.md (ambos del mismo WO-02c).
- **// WO-02c-FIX-G4 APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-G4-20260917.md`)**
  — GAP-4 del cross-exam (`WO-02c-CROSS-EXAM.md:173`): tally off-by-one del verify
  "selector-api COMPLETO (12/12)". Ground truth re-derivado (RULE 00):
  `find src -type f ! -name '*.test.ts' | wc -l` → **13**. CORREGIDO en §6 de
  `WO-02c-verify-VERIFY.md` (:86, in situ con marcador + errata §9 append-only) y nota
  correctiva append-only en §10.5 de `02c-SIM-STACK-SELECTOR.md` (:350; el texto
  original del verificador queda intacto encima — disciplina append-only del adendum).
  **Conclusión de cobertura SOBREVIVE: 13/13, fichas §5.1–5.9 cubren los 13 exactos.**
  Origen del "12" = UNKNOWN (ninguno de los dos árboles de lines-per-file.txt produce
  12 — el hazard de doble árbol NO explica este caso). Ironía registrada: el propio
  verify aplicó el estándar §34.5.3 a la cifra "87%" (C2 §10.4) y no a su propio
  tally. De paso, "sim-ctl 15/16" en el mismo §6 quedó corregido por el fixer paralelo
  (WO-02c-FIX-TALLY/G5: era 14/16, post-FIX-G2 = 15/16 — el "16/16" de G2 §12.3 refutado).
  Cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados:
  02c-SIM-STACK-SELECTOR.md + WO-02c-verify-VERIFY.md (ambos WO-02c) + este board.
  Sin colisión: entradas FIX-G1/G2/G5 y secciones §11-§13 de pares intactas.
- **// WO-02c-FIX-G3 APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-G3-20260917.md`)**
  — GAP-3 del cross-exam (`WO-02c-CROSS-EXAM.md:99-121,172`, hallazgo NUEVO): hard
  finding 4 "Idempotencia de redelivery: ON CONFLICT DO NOTHING + skip de XADD"
  estaba SOBRE-GENERALIZADO. Evidencia re-verificada de primera mano: el índice
  árbitro es PARCIAL a propósito (`WHERE simulator='revm'`,
  database/migrations/113_simulations_revm_idempotency.sql:27-29 + comment :12-14
  "legacy anvil history"; diseño multi-attempt original en 004_simulations.sql:2);
  el path legacy anvil produce `SimulatorKind::Anvil` (sim_engine.rs:163,205 — el
  default de compose según ficha 4.6) → para filas anvil el ON CONFLICT
  (persistence.rs:35) nunca arbitra → `inserted_fresh` SIEMPRE true → un
  PEL-redelivery de sim anvil passed re-publicaría duplicado a
  arbx:opps:simulated (impacto HOY nulo: passed=0 histórico, 00-SYNTHESIS).
  APLICADO (doc-only, marcado `// WO-02c-FIX-G3`): hallazgo 4 de WO-02c-DESIGN.md
  reclasificado "CANONICAL_REPO — scope ACOTADO"; ficha 4.5 con nota inline de
  scope revm-only + contraejemplo; ficha 4.2 (:119) acotada ("solo filas
  simulator='revm'"); errata §14 append-only en 02c-SIM-STACK-SELECTOR.md.
  Extender la dedup a anvil = WO propio con diff, operator-gated (consistente
  G6/G7; rompería la semántica multi-attempt de migraciones 004/113). Convergencia
  con fixer G5: su errata §13 ya anticipaba que G3 es load-bearing según el valor
  runtime de SIM_BACKEND. Sin colisión con fixers paralelos G1/G2/G4/G5
  (secciones disjuntas, marcadores intactos post-edit). Cero código tocado, cero
  git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados:
  02c-SIM-STACK-SELECTOR.md + WO-02c-DESIGN.md (ambos WO-02c) + este board.
- **// WO-02c-FIX-TALLY APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-TALLY-20260917.md`)**
  — tallies de completitud del verify §10.5/§6 hechos aritméticamente exactos (FAIL-HONEST).
  Cifras canónicas: **selector-api 13/13 · simulator-v2 6/6 · sim-ctl 14/16 al momento del
  verify (DOS sin ficha: route_lookup.rs Y build.rs — el "15/16" reportado era off-by-one) ·
  15/16 tras FIX-G2 (solo build.rs pendiente; el "16/16" de §12.3 era auto-contradictorio)**.
  Denominador 16 = 15 src .rs + build.rs, árbol principal, filtro `./backend/...` sobre
  lines-per-file.txt (documentado en errata §15 del entregable + §10 del verify). Hint del
  charter parcialmente REFUTADO: el worktree stale NO explica el "12/12" (ambos árboles
  dan 13 no-test en selector-api; el worktree solo carece de 2 tests) → desliz manual
  UNKNOWN; doble verificación independiente con el fixer G4 (que cerró 12/12→13/13 en
  paralelo — convergencia dual). Filtro documentado como convención de mesa para WO-03+.
  Conflicto con cifras de pares (FIX-G2 "16/16", nota G4 "superseded a 16/16") resuelto
  por errata + marcador in situ, cero contenido fáctico ajeno pisado. Renombrado G5→TALLY
  por colisión con el fixer G5 del GAP-5/SIM_BACKEND. Cero git/cargo/npm/VPS/HTTP (0/5).
- **// WO-02c-FIX-G1B APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-G1B-20260917.md`)**
  — cierra el RESIDUO del GAP-2 del cross-exam (WO-02c-verify-CROSS-EXAM.md:44-52):
  la fila SEL-GATE-01bis de `WO-02c-DESIGN.md:34` seguía diciendo "si persiste el 87%
  post-deploy" — única ocurrencia claim-bearing viva tras el fix paralelo G1 (que ya
  re-bajó 02c §5.3 :171 y §9.4 :235, y documentó que §6 :197-207 NO contenía la cifra;
  su "quema RPC" es claim distinto INFERRED etiquetado). Re-bajada in situ a
  "fracción mayoritaria NO cuantificada de sims quemadas en rows producer-rejected"
  con soporte canónico 00-SYNTHESIS.md:24-25 (74 sims/9h S4, detección 100% rejected,
  v3_quote_unavailable 60/76) + errata append-only en el DESIGN. Re-verificación
  RULE 00 del fixer (independiente): grep "87%|funnel 2026-09-17T04:00Z" en
  real-cycles-audit-20260916/ = 0 hits (5 archivos); ocurrencias restantes de "87%" en
  02c viven SOLO en §10.4 (declaración C2, fuente de la corrección) y erratas §11+.
  sel-gate01.test.ts:5-9 y engine.ts:28-30 = operator-gated, NO tocados. Regla de mesa
  reiterada: citar "fenómeno canónico, cifra sin artefacto". Cero git/cargo/npm/build/
  VPS/HTTP (0/5).
- **// WO-02c-FIX-UNPINNED APLICADA (2026-09-17, fixer gang ronda 1 — `WO-02c-FIX-UNPINNED-20260917.md`)**
  — GAP-3 del cross-exam del verify (`WO-02c-verify-CROSS-EXAM.md:54-60`, agent-fixable)
  CERRADO: el §10.2 del adendum decía que el unpinned replay (block_number NULL →
  simulador compartido → replay contra `latest`) ocurre "SIN log ni observación que lo
  declare", SOBRESTIMANDO la falta de trazabilidad. Evidencia re-verificada (RULE 00):
  el doc-comment de `simulator_for_candidate` DOCUMENTA explícitamente el caso None →
  shared sin pin (consumer.rs:768-775: "the shared simulator is reused untouched when
  no pin applies"; precisión de cita: el ":776-783" del cross-exam es la firma, el
  comentario vive :768-775). Lo ausente es observabilidad RUNTIME (log/métrica), no
  documentación. APLICADO in situ con marcador `// WO-02c-FIX-UNPINNED`: §10.2 y §12.2
  de `02c-SIM-STACK-SELECTOR.md` + §2 de `WO-02c-verify-VERIFY.md` + erratas
  append-only (§16 del entregable, §11 del verify). El hallazgo de fondo SOBREVIVE
  acotado (único Option-vacío del wiring que degrada calidad sin trazabilidad runtime;
  observación, no defecto R8); candidato `sim_consumer.unpinned_replay` debug-log sigue
  GATED operador (cero .rs tocados). Eco residual documentado, no tocado:
  `WO-02c-FIX-G2-20260917.md:64` ("sin log" — reporte histórico del par G2).
  Cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados: entregable 02c +
  reporte verify + este board (todos superficie del gang WO-02c). Sin colisión con
  fixers paralelos G1/G1B/G2/G3/G4/G5/TALLY — ediciones disjuntas, marcadores ajenos
  intactos (verificado post-edit con grep).
- **// WO-02b-FIX-TERMINUS APLICADA (2026-09-17, fixer gang ronda 1 — GAP G-2 del
  cross-check de mesa)** — responde la pregunta abierta de WO-02d-DESIGN §'Notas para
  la mesa' (:108-111) y cierra el "02b cross-check ABIERTO" del verify de 02d
  (WO-02d-verify-CROSS-EXAM.md §4.3). **DISCREPANCIA PUBLICADA: el whitelist de 11
  strategy_kinds (plan_validation.rs:17-32) ES el cuello de botella — TERMINAL, no
  upstream — y el mapa operador→cartucho de 02b NO dice lo contrario (lo confirma).**
  Evidencia re-derivada (parser regex 264 .rhai + grep, RULE 00): (1) kind == stem del
  .rhai (contracts.rs:34, cartridge_boot.rs:1179) y NO existe filtro de kind antes del
  terminus (el único upstream es liquidation-only, sim-ctl/tx_builder.rs:144-147);
  (2) el whitelist admite exactamente **8/264 stems (3.0%) → 256 descartados** —
  precisión: el "253" del charter trataba los 3 kinds genéricos como cartuchos
  (`dex_arb`/`flashloan_arb`/`triangular` NO son stems; sus productores son legacy
  HUEHUFO default-OFF: flashloan_engine.rs:26, cartridge_boot.rs:498,785-790);
  (3) por operador: 8 ops llegan al terminus (op_01*, 15, 16, 21, 22, 26, 27, op_30*
  — *solo-secondary) y **15 ops son terminus-muertos pese a wiring activo** (05, 06,
  07, 08, 10, 11, 13, 14, 17, 19, 20, 23, 24, 25, 29: ninguno de sus cartuchos primary
  NI secondary pasa el whitelist); (4) scope de modo §34.1.3: los 3 call-sites del
  whitelist viven SOLO en relays-client (bundle_builder.rs:60,
  execution_admission.rs:23, plan_validation.rs:182) → paper/shadow NO descarta nada;
  la detección/simulación/scoring de los 264 sigue íntegra — el descarte aplica solo
  al terminus de ejecución (superficie calldata TLS forward/backward,
  plan_validation.rs:53-80). Extender el whitelist = WO propio con diff, operator-gated.
  Adendum §12 append-only en `02b-OPERADORES-MATH-ENGINE.md` + reporte
  `WO-02b-FIX-TERMINUS-20260917.md` (tabla op-by-op y método reproducible). Cero
  archivos de otros WO pisados; cero .rs tocados; cero git/cargo/npm/VPS/HTTP (0/5).
- **// WO-02b-FIX-TERMINUS-VERIFY — CONVERGENCIA DUAL (2026-09-17, fixer gang ronda 1,
  respawn paralelo con el mismo charter)** — al encontrar el entregable ya publicado,
  el segundo fixer NO duplicó: re-derivó TODO con parser independiente (RULE 00).
  Resultado: las 23 filas de la tabla op-by-op reproducidas IDÉNTICAS (8 admitidos
  {01,15,16,21,22,26,27,30} con mismos 4 flags sec-only; 15 terminus-muertos; 9 nunca
  declarados; 8/264 stems, 256 descartados; whitelist−stems = los 3 genéricos).
  Todas las citas de código re-verificadas byte-exact (plan_validation.rs:17-32,
  cartridge_boot.rs:1179, contracts.rs:34, tx_builder.rs:144-147, productores legacy).
  Dos precisiones añadidas (§7 del reporte, append-only): (1) grep completo =
  3 call-sites de producción + 2 asserts #[cfg(test)] (plan_validation.rs:478-479) —
  el claim "todas las vías en relays-client" SOBREVIVE; (2) hallazgo nuevo menor:
  los 8 stems admitidos son homogéneos en wiring (todos Primary {15,16,21,27} /
  Secondary {01,22,26,30} — INFERRED). GAP G-2 CERRADO y DOBLE-VERIFICADO.
  Cero .rs/git/cargo/npm/VPS/HTTP (0/5).
- **// WO-02d-FIX-GRAN APLICADA (2026-09-17, fixer gang ronda 1 — obs. 4.1 del
  cross-exam de 02d)** — granularidad de prueba débil en filas shared-rs §3 CERRADA:
  la fila agrupada `db_pool/health/logging/metrics` citaba solo ":1-8 c/u" (headers),
  incumpliendo el gate "línea exacta". Desglosada en 4 filas con citas concretas
  re-verificadas contra el fuente (lectura completa): `metrics.rs` — `init_metrics()`
  :431-464 (30 collectors Lazy forzados, cuenta grep-exacta; corrección in-fix 29→30),
  `metrics_handler()` :466-481, `record_inclusion()` :419-429, 11 bloques de familias
  con rangos exactos; `db_pool.rs` — `from_env()` :39-67 / `options_with_timeouts()`
  :75-82 / `connect_pool()` :86-89; `health.rs` — `build_health_router()` :36-58
  (`/metrics` :57 despacha a metrics_handler); `logging.rs` — `init_tracing()` :7-28.
  Hint del cross-examiner CONFIRMADO: re-exports lib.rs:38/:39/:41/:42 + consumo vivo
  del terminus relays-client main.rs:42,44-45/:117/:118/:455. **GAP declarado
  fail-honest**: 0 `#[cfg(test)]` en los 4 módulos — cobertura por test unitario NO
  existe, marcada GAP en cada fila (no se fabrica, RULE 00). Hallazgo NUEVO colateral
  (doc-only, §ERRATA-02.2): doc-drift `db_pool.rs:14-16` — el doc-comment afirma que
  el módulo emite `db.connected`/`db.connect_failed` pero tiene CERO llamadas de
  logging; los eventos R6 los emiten los callers (relays main.rs:276,:281; searcher
  main.rs:532,:544; sim-ctl :707-712 con NOMBRES propios `sim.db_*` — drift
  nomenclatura adicional; token-enricher :464 `enricher.db_connected`). Corrección
  del .rs = operator-gated, NO aplicada. Filas `cred_rotation.rs`/`tokens.rs` (también
  ":1-8") fuera de charter, documentadas no tocadas. Sin colisión con fixer paralelo
  WO-02d-FIX-G2 (TTL, ediciones disjuntas). Reporte:
  `WO-02d-FIX-GRAN-20260917.md`. Cero .rs/git/cargo/npm/VPS/HTTP (0/5).
- **// WO-FUNNEL-01 APLICADA (2026-09-17, fixer gang ronda 1 — gap 4.1 del BROWSE
  de mesa)** — funnel 0/0/0/0/0 con PG INSERTED 2500/min CERRADO con DOS defectos
  independientes verificados en VPS read-only (07:48–07:52Z): (A) bucket mismatch —
  TODA la salida del funnel (gate_*/passed/db_persisted/db_errors) se incrementaba en
  el bucket legacy chain-0 (`counters()`, opportunity_emitter.rs:313/:369/:450/:699/:703)
  mientras el heartbeat drena `chain_counters(primary_chain=1)`;
  (B) la ruta de detección ACTIVA es RouteScannerWorker RU-3 per-block (evidencia:
  625/625 `intent_received` con source_event=new_block en 5m = 25 canonical/block × 25
  blocks; `route_decoder.done`=0 en 60m → stream mempool silencioso) y no tenía
  contador de intake. FIX (6 archivos, marcadores `// WO-FUNNEL-01 (2026-09-17)`):
  emitter → `chain_counters(opp.chain_id)` en los 5 sitios; contador NUEVO
  `block_intents_dispatched` (counters.rs + route_scanner_worker dispatch :651 +
  heartbeat drain/snapshot/log) — contador nuevo en vez de re-etiquetar
  pending_received (estabilidad de contrato de nombre, counters.rs:177-179); frontend
  schema optional + fila "0. Block-scan intents (RU-3)" en PipelineFunnelCard.
  VERIFICADO: cargo check + fmt limpios, tests counters 7/7 · route_scanner 24/24 ·
  emitter 13/13, tsc 0 err, contract-test fixture 27/27, route-funnel 5/5. Residuales
  documentados NO tocados: stream mempool muerto en VPS (clase WSSUB-05, WO de
  diagnóstico aparte), block_scanner.rs mismo gap de clase (modo no activo), stage 3
  Enriched sigue legacy-semántica (0 honesto), latente TS nonnegative vs -1 sentinel
  (pg_period_inserted). Resuelve la tensión BROWSE §5 vs WO-02a: lo que dispara
  active_eval_enter NO es mempool, es RU-3. Reporte:
  `WO-FUNNEL-01-FIX-20260917.md`. Cero git/deploy/mutación-VPS (0/5 HTTP).
- **// BROWSE-FIX-4.5 APLICADA (2026-09-17, fixer gang ronda 1 — gap 4.5 del
  reporte BROWSE del Operador de mesa — `BROWSE-FIX-4.5-ICON-RAIN-20260917.md`)**
  — lluvia de GET /api/v1/token-icon duplicados (~30×/token/render + ráfaga
  ERR_ABORTED) CERRADA en cliente. Causa raíz re-derivada (CANONICAL_REPO, no
  INFERRED): el effect del hook tenía (1) sin single-flight — cada montaje de
  tarjeta disparaba su propio fetch, y (2) abort-on-unmount que mataba el request
  antes de que `setCachedIcon` poblara el nivel 2 → lluvia autosostenida. El
  servidor YA emitía Cache-Control (token-icon.ts:203/:303/:56) — cero cambios
  backend necesarios. Fix (frontend-only, marcado `// ICON-RAIN-20260917`,
  `frontend/lib/hooks/useTokenIcon.ts` +151/−41): single-flight módulo-scope por
  `chainId:address`, fetch compartido que SOBREVIVE unmounts (elimina ERR_ABORTED
  y puebla el caché), negative-cache de fallos 60 s espejando NEG_TTL_SECS del
  api-server con el string de error real surfaced (R8). Verificado: test nuevo
  de 5 casos reproduce la lluvia de 30 montajes → 1 fetch; suite completa
  126 arch / 1265 tests PASS; `tsc --noEmit` exit 0. Sin colisión de files
  (Archivo Frío/funnel 4.1-4.2 los está tocando OTRO par — documentado en el
  reporte §4). Deploy-gated (operador): verificación en vivo post-deploy
  descrita en el reporte §4. Cero git/VPS/HTTP (0/5).
- **// WO-ARCHIVE-401 APLICADA (2026-09-17, fixer gang ronda 1 — GAP §4.3 del BROWSE
  Operador de mesa)** — Archivo Frío (`/operations`) ya NO queda en
  "Cargando estado de archivo…" perpetuo ante edge 401
  `{"error":"missing_admin_token"}` (defecto ya reportado el 2026-09-06 por la
  Auditora R8 de omniscience-integration y aún vivo). Causa raíz: la fila de la tabla
  se decidía SOLO por `!status` y el bloque archivos colapsaba unknown→vacío
  (`?? []` → "Sin archivos aún." fabricado, violación R8). Fix en
  `frontend/app/operations/components/ArchivePanel.tsx` (marcado
  `// WO-ARCHIVE-401 (2026-09-17)`): labels fail-honest puros
  (`retentionStateLabel`/`filesStateLabel`: !status+error → estado de error verbatim
  con `role="alert"`; !status sin error → loading honesto; "Sin archivos aún." SOLO
  con status real) + extracción `ArchivePanelView` (patrón repo del panel hermano
  RejectionBreakdownPanel) para testeabilidad estática sin jsdom. Keep-last-good-status
  y poll 30s intactos; `retries: 0` del api-client ya evitaba backoff. Verificación:
  test nuevo 9/9 PASS (`__tests__/ArchivePanel.test.tsx`), /operations 6 archivos/49
  tests PASS, suite completa 1265/1265 (2 corridas; 1 fallo transitorio no reproducido
  en la primera, INFERRED edición en vuelo del par del gap 4.5), tsc 0 errores en
  superficie propia (único error del árbol = diff del par 4.5 useTokenIcon, probado
  con stash-roundtrip), eslint exit 0. Cero archivos de otros WO pisados. NO deployado
  (NO-GIT; recordar RULE 03 rebuild frontend). Reporte:
  `WO-ARCHIVE-401-FIX-20260917.md`. Cero git/cargo/build/VPS/HTTP (0/5).
- **// GAP1-REJ-REASON-FEED APLICADA (2026-09-17, fixer gang ronda 1 — GAP-1 del
  BROWSE R8 (`BROWSE-Auditor-de-honestidad-R8-...md` §2.2/:104) —
  `FIX-GAP1-REJ-REASON-FEED-20260917.md`)** — `rejection_reason` YA viajaba hasta la
  tarjeta (`OpportunityTradeCard.tsx:276` pasaba la prop; mapper types.ts:288/:407
  null-preservador) PERO `StatusPill.tsx` la renderizaba SOLO como atributo `title`
  (hover) → 0 ocurrencias en `document.body.innerText`, exactamente lo medido por el R8.
  FIX (1 archivo de código, marcador `// GAP1-REJ-REASON-FEED (2026-09-17)`): pill
  rechazado ahora pinta `REJECTED · <reason>` como texto visible (mono, truncate 220px;
  title conserva el valor íntegro); null → label solo, sin fabricación (R8);
  non-rejected no renderiza razón; R1 intacta (componente puro). Único consumidor vivo
  del StatusPill tocado = feed /opportunities (los de premium-ui.tsx son otro
  componente, no tocados). **Refuta para texto visible** al BROWSE-Operador-de-mesa §1
  ("cada tarjeta trae razón como badge"): código pre-fix + screenshot del propio R8 +
  test pre-fix title-only demuestran que era a11y-snapshot leyendo el title, no texto
  visible — regla de mesa para BROWSE: innerText/screenshot, no a11y proxy. Colateral
  registrado para WO-04: `OpportunityDetailDialog` tampoco muestra la razón (grep 0).
  VERIFICADO: StatusPill 13/13 (+3 nuevos: visible-text, null fail-honest exacto
  "REJECTED", non-rejected sin razón) · OpportunityTradeCard 6/6 · suite completa
  126 arch/1268 tests PASS (baseline 1265 + 3 propios; error useTokenIcon preexistente
  YA ausente) · tsc --noEmit 0 errores. Cero git/VPS/cargo (0/5 HTTP).
  BROWSE operador de mesa — `FIX-RDO-503-REASON-UI-20260917.md`)** — el 503 de
  `/api/route-discovery-outcomes/summary` dejaba "outcomes resueltos"/"opportunities"
  en guion mudo en el funnel de /operations. Causa raíz UI (CANONICAL_REPO): la razón
  SÍ llega al browser (api-server emite `reason` en las 3 vías 503; edge proxy pasa
  status+body verbatim) y el hook SÍ la captura (useRouteDiscoveryOutcomes.ts:144-148),
  pero RouteDiscoveryFunnelCard.tsx:42 destruía SOLO `totals` y descartaba
  `status`/`unavailableReason` (asimetría vs la etapa Reconciled, que sí mostraba su
  error). FIX (working tree, marcador `// RDO-503-REASON-UI (2026-09-17)`): funnel card
  ahora destructura status+unavailableReason y pinta la línea warning con la razón
  verbatim; + doc-drift "every 8s"→60s en el alert STALE del panel (mismo superficie,
  POLL_MS=60000 real). Causa raíz SERVIDOR: endpoint HOY sano (curl 200; total 24h
  11.5M outcomes), PERO responseTime 13184ms vs timeout 15s e ingesta 2.58M rows/h
  (2× baseline RDO-SUMMARY-503) → `query_failed` bajo burst = más probable;
  `rollup_backfilling` plausible (rollup hoy missing_24h=0); razón exacta de 06:20Z
  INRECUPERABLE (log window rotada 03:38→06:55, R9; LOGFLOOD del QA-WS §5.4). Hipótesis
  charter "server-lag 27543" REFUTADA como causa directa (G-PIPE-1 mide arbx:opps:*,
  no esta tabla; sobrevive solo como proxy de carga). Verificación: tsc --noEmit 0
  errores en tocados (1 error pre-existente en useTokenIcon.test.ts, zona gap 4.5,
  no tocado) · vitest route-funnel.test.ts 5/5 PASS · post-deploy visual queda para
  WO-06/operador. Cero git/deploy/VPS-mutación. HTTP manual 1/5 (curl diagnóstico).
  Sin conflicto de claims (superficie frontend, WO-04 no iniciado).
- **// WO-GAP3 APLICADA (2026-09-17, fixer gang ronda 1 — GAP-3 del BROWSE de
  mesa — `WO-GAP3-POSTURE-CHIPS-FIX-20260917.md`)** — header mini-channels
  socket/pairs/quote_anchor stuck CONNECTING perpetuo (con feed LIVE y
  runtime_ack LIVE) + routes flapeando LIVE↔DEGRADED CERRADOS. Causa raíz
  (CANONICAL_REPO): (A) `markFresh` del ArbxRealtimeProvider refrescaba
  `lastMessageAt` pero JAMÁS escribía `status` — para pairs/quote_anchor no
  existía ningún otro write de status en el repo → `connecting` eterno pese a
  snapshots aceptados cada 30s, y el agregado WO-08 del socket chip heredaba
  ese peor estado (el doc-comment del provider ya declaraba la intención
  "marks the channel live" que la implementación omitía; el test WO-08
  :318-338 tenía el síntoma como fixture sin cerrar la transición);
  (B) carrera del REST pass: pass arrancado mid-disconnect estampaba
  `routes→polling` SOBRE el `live` de la reconexión, y sin (A) solo el próximo
  connect lo restauraba → flap con socket up (firma exacta observada por el
  Operador de mesa §1). FIX (4 archivos, marcadores `// WO-GAP3 (2026-09-17)`):
  markFresh escribe `status:"live"`; errores REST viajan verbatim a
  `lastError` (R8, chip ERROR en vez de connecting mudo); re-check de
  `wsConnectedRef` post-awaits (WS dueño de routes al reconectar); helper puro
  `restFetchOutcome` (realtime-slices.ts) como seam testeable. Header steady
  post-fix: socket/routes/runtime_ack/pairs/quote_anchor LIVE; desconexión
  real sigue mostrándose (RULE 00, sin histéresis). VERIFICADO: vitest suite
  completa 127 arch / 1283 tests PASS (5 nuevos); tsc = 3 errores pre-existentes
  PROBADOS por stash-roundtrip (zona del par FEED-SCHEMA-01, no tocada);
  eslint 0 en superficie propia. Residuales documentados no tocados:
  ROOM-AUTH-01 (runtime_ack LIVE sobre join unauthorized — propio fixer),
  NO-WS-LOGS-01, LOGFLOOD server. Sin colisión de archivos (los 4 estaban
  limpios pre-fix). Cero git/cargo/VPS/HTTP (0/5). NO deployado (RULE 03
  rebuild frontend al deployar).
- **// BROWSE-FIX-4.6 APLICADA (2026-09-17, fixer gang ronda 1 — gap 4.6 del
  BROWSE Operador de mesa — `BROWSE-FIX-4.6-SPONTANEOUS-NAV-20260917.md`)**
  — navegación espontánea /opportunities → `/` (~8 s, 1×, sin input ni error
  de consola): CÓDIGO DE APLICACIÓN EXCULPADO por barrido exhaustivo
  (CANONICAL_REPO, tabla de greps reproducible en el reporte §2: cero
  router.push/replace a `/`, cero redirect() en la ruta, cero middleware,
  cero location/pushState, cero SW/meta-refresh/hotkeys navegacionales; el
  único `<Link href="/">` es el logo del header, click-gated). Mecanismo queda
  UNKNOWN acotado a {recuperación interna App Router 14.2.35, redirect HTTP
  upstream en document fetch duro, capa browser/automatización}; hipótesis
  charter "race prefetch RSC" se mantiene HYPOTHESIS (websearch §4 del
  reporte: NO existe bug upstream confirmado con esta firma — solo clases
  relacionadas citadas). Correlación temporal 8s ⊆ ventana "Edge connection
  error" <10s del primer pinto (BROWSE §1) registrada SIN causalidad.
  FIX: `NavigationSentinel` null-render montado en root layout (`app/layout.tsx`,
  marcador `// BROWSE-FIX-4.6 (2026-09-17)`): atribuye cada navegación
  same-document con mecanismo (Navigation API Chromium / popstate fallback) +
  ms desde el último input real (pointerdown/keydown capture) → una recurrencia
  queda clasificable (protocolo de lectura §7 del reporte para pares de browse:
  `navigation-api:push` sin input = framework soft; sin línea + URL cambiado =
  hard redirect upstream). Limitación fail-honest: atribución de push soft SOLO
  en Chromium ≥105 — exactamente el browser donde se observó la anomalía.
  VERIFICADO: vitest 6/6 PASS (SSR null-render R1 + formatter puro + probe
  defensivo), tsc 0 errores en superficie propia (3 errores ajenos = diff EN
  VUELO del par `WO-G2-PARITY` en lib/schemas.* + OpportunityTicker fallout,
  documentados NO pisados), eslint exit 0. `app/layout.tsx` no reclamado por
  ningún WO al momento del fix (sin conflicto de claims; WO-04 no iniciado).
  Cero git/cargo/build/VPS/HTTP-al-dominio (0/5; 1 websearch a fuente pública
  ajena al dominio). NOTA de mesa: durante la edición de esta entrada un par
  editó el board concurrentemente — el header del bullet RDO-503 quedó
  parcialmente consumido arriba (contaminación registrada, contenido intacto,
  no restaurado aquí por disciplina no-pisar; su fixer o el operador puede
  restaurar el prefijo `**// RDO-503-REASON-UI APLICADA (…gap 4.2 del`).
- **// WO-G2-PARITY APLICADA (2026-09-17, fixer gang ronda 1 — GAP-2 del
  orquestador — `WO-G2-PARITY-FIX-2026-09-17.md`)** — FEED-SCHEMA-01 del QA-WS
  (feed de oportunidades REST caído con "items.0.block_number: Expected
  number, received string" → OpportunityTicker del ROOT LAYOUT en
  "Opportunity feed unavailable (retrying every 30s)" en TODAS las páginas,
  incl. /operations y /status; hueco G2 §37 con reproducción viva) CERRADO en
  dos capas. Causa raíz re-derivada (CANONICAL_REPO): opportunities.block_number
  es BIGINT (migration 003:19) y node-postgres devuelve int8 como STRING; el
  LIVE_QUERY lo selecciona sin cast y rowToOpportunity lo copiaba verbatim
  mientras su propia interfaz declaraba number|null (el tipo mentía). El Edge
  es INOCENTE: index.ts:666 es proxy pass-through con KV 2s (precisión sobre
  el "edge serializa" del QA-WS §5.1). FIX (marcadores
  `// WO-G2-PARITY (2026-09-17)`): (a) api-server normalizeBlockNumber() en el
  mapper + interfaz honestada number|string|null; (b) frontend
  BlockNumberWireSchema preprocess en schemas.ts:66 y :1182 (coerción
  quirúrgica — NO z.coerce porque Number(null)===0 fabricaría bloque 0;
  strings no numéricos RECHAZAN el parse, fail-honest; regex evita NaN que
  Zod 3 acepta como number); (c) api-client getValidated/postValidated
  `z.ZodType<T, z.ZodTypeDef, unknown>` (sin esto la inferencia colapsaba
  block_number a unknown cuando Input≠Output — los "3 errores ajenos" que el
  par BROWSE-FIX-4.6 vio en vuelo YA están resueltos: tsc exit 0 al cierre).
  Línea 1182 auditada: su productor siempre emite number (parseInt
  admin-chains.ts:218) — endurecida defensivamente por el charter.
  VERIFICADO: api-server vitest completo 66 arch/851 tests PASS + tsc exit 0;
  frontend suite completa 127 arch/1283 tests PASS + tsc exit 0 + eslint 0;
  9 tests de regresión nuevos (4 backend vía __forTesting.rowToOpportunity,
  5 frontend). Residual: "S-Curve widget" del charter precisado — SCurveChart
  consume SCurvePayload SSR sin block_number; el widget roto es el
  OpportunityTicker global. Deploy-gated (operador): con SOLO frontend
  deployado el feed YA revive (coerción tolera el string del api-server
  viejo). Cero git/deploy/VPS/HTTP (0/5).
- **// WO-FEED-SCHEMA-01 VERIFY — CONVERGENCIA DUAL (2026-09-17, fixer gang
  ronda 1, charter FEED-SCHEMA-01 — `WO-FEED-SCHEMA-01-VERIFY-20260917.md`)**
  — al despacharse este fixer, el gap YA estaba cerrado por el par WO-G2-PARITY
  (mismo defecto, charter GAP-2 del orquestador). NO duplicó: re-derivó
  independientemente la causa raíz y re-verificó TODO. (1) Premisa del charter
  ("edge serializa block_number") REFUTADA por segunda vía: `proxy()`
  (edge/worker/src/index.ts:445) es pass-through body-verbatim con KV 2s
  (:666); el string nace en api-server (BIGINT migration 003:19 + LIVE_QUERY
  sin cast opportunities-live.ts:268 + node-postgres int8-as-string). (2) Fix
  del par confirmado in situ con marcadores (interfaz honestada :207-213,
  normalizeBlockNumber :426-440, mapper :594, BlockNumberWireSchema
  schemas.ts:11/:83/:1199, Input=unknown pin api-client.ts:141/:204). (3) Gates
  re-ejecutados: api-server 17/17 + tsc 0 · frontend 18/18 + tsc 0. (4) Barrido
  repo-wide de la clase (aporte nuevo): consumidores Zod de block_number en
  frontend = exactamente 2 (ambos ya cubiertos); WS no valida block_number;
  fork-status.ts:72-77 ya defensivo pre-fix; archivers = escritores paramétricos.
  FEED-SCHEMA-01 queda CERRADO y DOBLE-VERIFICADO. Fail-honest operativo:
  anomalía NTFS/GitBash al leer el reporte del par (listaba pero cat devolvía
  ENOENT; legible vía pipeline Get-ChildItem | Get-Content) — registrada en el
  reporte §5. Cero código/git/cargo/VPS/HTTP (0/5).

## WO-06 · FEE-TIER-AWARE-QUOTING — 🔄 IN_PROGRESS (2026-09-17, gang vía Hermes run_c3f28ef2388c4a3abe95a7845a50b428)
- Contexto: embudo honesto post-SEL-GATE-01 → 0 accepts; productor rechaza TODO;
  v3_quote_unavailable 14.473/15min (clase cobertura: quotes a tiers inexistentes por par).
- Fix: cotizar SOLO fee_tiers presentes en catálogo del par (PG pools.fee_tier /
  pool_index_v3); par sin pools V3 = rechazo con razón exacta (no quote-unavailable).
- Gates: vector independiente por tier (pips uint24) + test par-sin-V3 + accepts>0 medible.
- Estado del sistema tras la sesión: detección viva, DB reconectada (credencial rotada a
  hex + .env dedupe), SIM-FUND-01b deployado, 2 pools muertas fuera, main=692d6b34.
- Pendiente operador: Alchemy cuota mensual AGOTADA (429) — billing; llama/0xrpc caídos.
