# HP-01 — CENSO DE VERDAD: Guía del operador (SEED v1+v2) vs canon del repo

> **⚠️ PROVENANCIA (fail-honest, RULE 00)**: HP-01 (agente censador designado en
> GOAL-WORKORDERS.md) **NO aterrizó** — al cierre de HP-04 este archivo no existía.
> HP-04 (strategy-architect) ejecutó el censo de las secciones que son su insumo
> ((b) familias/conteos y (c) edges) más la localización del espejo TS, porque
> `HP-01-CENSO.md` está bajo el claim de HP-04 y nadie más lo escribiría.
>
> **EXTENDIDO por HP-01-B (2026-09-08, RESPAWN-2 mitad B — ítems (d)/(e)/(f))**:
> el agente HP-01 original murió; el reemplazo B cubre la segunda mitad del charter
> con rigor completo y **EXTIENDE** este archivo como su nota de provenancia exige
> (las secciones (a)-(c) de HP-04 quedan intactas). Cambios: (d) censo completo
> HopMask/hops-vs-tiers (era NOT-COVERED), (e) lista literal de claims completados
> con estado AL CAPTURAR vs HOY (HP-03/HP-04/HP-05 ya aterrizaron en el working tree
> — el censo distingue ambos instantes), (f) verificación adversarial del espejo TS
> con el probe literal del charter. Todo con evidencia file:line verificada en disco.
>
> Todo lo abajo es evidencia propia verificada en disco el 2026-09-08 con
> `py` (encoding utf-8 explícito). Clasificación: CANONICAL_REPO salvo indicación.

## (a) Los 31 operadores reales vs la tabla de 32 del documento

- Repo real: `backend/math-engine/src/operators/op_01_svd.rs` … `op_31_drl_agent.rs`
  (31 archivos, verificados por ls). **op_32 NO existe** — `operators/mod.rs:6` es la
  INSTRUCCIÓN ("Añadir op_32 → crear archivo + registrar en registry.rs"), no un registro.
- Registro vivo: `OperatorRegistry::register_all()` (`operators/mod.rs:106-140+`) con
  ids 1..31; `TopologyMap` hardcodea `COLS: usize = 31` (`matrix/topology_map.rs:20`).
- Desfase de índices SEED↔repo (verificado): SEED op_01="Descenso de Gradiente" vs repo
  op_01=svd; SEED op_02..04 (SGD/CoordAsc/BayesOpt) vs repo pca/eigen/von_neumann;
  SEED op_12="BFS" vs repo op_12=mle; SEED op_18="Monte Carlo" vs repo op_18=lagrangian
  (Monte Carlo real = repo op_22); SEED op_28="Shapley" vs repo op_29=shapley (repo
  op_28=jit_liquidity); SEED op_30="Lévy" vs repo op_09=levy (repo op_30=gnn_encoder);
  SEED op_31="Supervivencia" vs repo op_31=drl_agent. COINCIDEN: op_05..op_11, op_13..op_17,
  op_19..op_27. **Consecuencia para HP-04**: el "op_32 NSGA-II" del SEED es un NUEVO
  operador (no rellena ningún hueco de numeración) — correcto asignarle id 32.
- SEED v2 (`OPERADOR-SEED-V2-2026-09-08.md`) mantiene el mismo desfase; su enum Rust
  (líneas 120-153) usa la numeración del documento, NO la del repo.

## (b) Familias y conteos (11 familias, 264 estrategias)

Conteos derivados de `knowledge_graph.jsonl` (BELONGS_TO) y `capability_matrix.json`
(265 entradas = 264 estrategias + `Legend`); ambos coincide con la tabla del SEED v1
(líneas 33-40) EXACTA:

| Familia | Detector(es) presentes | Estrategias | op_16 Kelly | op_22 MC | op_23 Queueing |
|---|---|---:|---:|---:|---:|
| MEV-01 spot DEX misma cadena | R_CLOSED_CYCLE=25, R_ORDERBOOK=4, R_DIRECT_INDIRECT=2, R_SPLIT=2, R_BASKET_NAV=2, R_COW=1 | 36 | 31 | 36 | 5 |
| MEV-02 curvas AMM | CF_DYNAMIC=4, CF_BATCH=2, CF_CPMM/CF_CONSTANT_SUM/CF_STABLESWAP/CF_WEIGHTED/CF_CLAMM/CF_LB/CF_PMM/CF_BOND/CF_VAMM/CF_TWAMM/CF_CROSSINV=1 c/u | 17 | 4 | 17 | 2 |
| MEV-03 eventos de estado | E_STATE=15, E_POST=6, E_LATENCY=4, E_ORACLE=2, E_AUCTION=2, OBSERVE=2 | 31 | 4 | 31 | 6 |
| MEV-04 paridad/redención | P_PEG=7, P_WRAP=5, P_NAV=5, P_LST=4, P_4626=3, P_YIELD=3, P_PTYT=3, OBSERVE=1 | 31 | 6 | 31 | 4 |
| MEV-05 CEX-DEX | C_CEXDEX=10, C_CEXDERIV=2, E_LATENCY=2 | 14 | **14** | 14 | **14** |
| MEV-06 cross-chain | X_BRIDGE=16, X_PREPOS=12, X_ORACLE=2 | 30 | 28 | 30 | **30** |
| MEV-07 derivados | D_BASIS=13, D_OPTIONS_SURFACE=6, D_SETTLE=5, D_OPTIONS_PARITY=4, D_FUNDING=2 | 30 | 15 | 30 | 20 |
| MEV-08 lending/liquidación | L_RATE=8, L_LIQ=7, L_COLLATERAL=5, L_AUCTION=4, L_LOOP=1 | 25 | 16 | 25 | 11 |
| MEV-09 intents/solvers | I_ROUTE=9, I_BATCH=4, I_ORDERFLOW=4, OBSERVE=2, I_DUTCH=1 | 20 | 0 | 20 | 14 |
| MEV-10 NFT/gaming | N_IDENTICAL=6, N_FLOOR=5, N_REDEEM=4, N_AMM=2, N_LIQ=1 | 18 | 1 | 18 | 7 |
| MEV-11 predicción | M_LOGIC=4, M_COMPLETE=3, OBSERVE=3, M_CROSS=1, M_AMM=1 | 12 | 0 | 12 | 1 |

Estrategias OBSERVE (sin decisión de ejecución): MEV-03-029/030, MEV-04-031,
MEV-09-019/020, MEV-11-009/010/011 — **8 en total**.

## (c) Censo de edges del grafo canónico

`skills/arbitragex-ultra/knowledge_graph.jsonl` — **2,511 edges, 0 errores de parseo,
0 duplicados** (verificado línea a línea):

| type | rel | n |
|---|---|---:|
| Strategy->Detector | DETECTED_BY | 267 |
| Strategy->Operator | USES_PRIMARY | 883 |
| Strategy->Operator | USES_SECONDARY | 833 |
| Strategy->Family | BELONGS_TO | 264 |
| Strategy->Surface | REQUIRES_SURFACE | 264 |

- Layout del archivo: sección 1 = por estrategia [DETECTED_BY + sus op edges en orden
  ascendente de op (roles intercalados)]; sección 2 = 264 BELONGS_TO; sección 3 = 264
  REQUIRES_SURFACE. Schema exacto (línea 1): `{"from", "rel", "to", "type"}`.
- **Solo 23 de 31 operadores tienen edges**: SIN edges op_02, op_03, op_04, op_09,
  op_12, op_18, op_28, op_31. op_22 (Monte Carlo) es el único cuasi-universal (264).
  → El canon NO rellena simetría: un operador sin justificación se queda a cero edges
  (precedente directo para la disciplina R8 de HP-04).
- Roles por operador (primary/secondary): op_21 176/2, op_08 131/47, op_22 16/248,
  op_30 0/27, op_27 53/18 — los operadores de EVALUACIÓN/SELECCIÓN viven en
  USES_SECONDARY (op_22, op_30). op_32 (selector Pareto) pertenece a esa clase.
- **No existe ningún edge op_32** (grep del archivo, 0 matches).
- Mapeo MEV-01 del SEED v1 (línea 161) vs canon: SEED declara primarios
  "op_15, op_21, op_27, op_16" — kg MEV-01-016 tiene exactamente primary op_15, op_16,
  op_21, op_27 (strategies/MEV-01-016/SKILL.md) → esa parte del SEED NO es marketing,
  coincide con el canon. El claim "op_32 ACTIVO" SÍ es aspiracional (no existe ni en
  Rust ni en el grafo).

### Invariante de 3 almacenes (descubrimiento clave para HP-04)

`knowledge_graph.jsonl` (op-edges por estrategia) ≡ `capability_matrix.json`
(`operators[]` por estrategia) ≡ `strategies/MEV-XX-YYY/STRATEGY.json`
(`primary_operators`∪`secondary_operators`) — **264/264 consistentes, 0 mismatch**
(verificado). `strategies/*/SKILL.md` espeja STRATEGY.json (línea `**Secondary**`,
264/264 formato limpio ascendente). TODO cambio de matriz debe tocar los tres o rompe
el canon contra sí mismo.

### Consumidores de la matriz (riesgo de build)

- `backend/search-engine`… NO: el grafo es capa de documentación/auditoría; el hot-path
  lee cartridges Rhai (`CartridgeMetadata.primary_operators`/`secondary_operators`,
  `backend/searcher-rs/src/cartridge/types.rs:24-30`, ids 1-31).
- `backend/searcher-rs/src/math_evidence.rs:87-104`: evalúa ops por id contra el
  registry y devuelve `None` honesto para ids no registrados → **sin riesgo de build**
  al introducir op_32 en datos (los cartridges no lo declaran aún).
- `scripts/excel_canon/build_canonical_artifacts.py:626-641` (`strategy_evidence`):
  compara `STRATEGY.json` vs workbook 11_STRATEGY_CATALOG (Primary_Ops/Secondary_Ops).
  **Consecuencia**: si HP-04 añade op_32 a STRATEGY.json y el workbook del operador no
  lo tiene, esas estrategias caen a PARTIAL en el próximo run de cobertura — divergencia
  HONESTA y visible (correcta bajo RULE 00); el cierre es follow-up del OPERADOR sobre
  el workbook (12_OPERATOR_CONTROL fila nueva + columna op_32 en 13_STRAT_OP_MATRIX).

## Espejo TS (localizado — HP-04 no lo toca, FUERA de su claim)

- **No existe un enum TS de operadores matemáticos op_XX**. `OperatorIdSchema`
  (`frontend/lib/apex/schemas/_primitives.ts:95-97`) es un UUID del OPERADOR HUMANO
  (soberanía C9.4, `frontend/lib/operator/types.ts`) — distinto dominio.
- El único espejo TS de la matriz es **generado**: `docs/quotebase_strategy_hop_map.json`
  (264 filas, workbook 11_STRATEGY_HOP_MAP) + `docs/quotebase_detector_policy.json`
  (60 filas, workbook 25_DETECTOR_POLICY) → `scripts/gen_quotebase_catalog_ts.py` →
  `backend/api-server/src/generated/quotebase_catalog.ts` (encabezado líneas 1-9:
  "GENERATED FILE — DO NOT EDIT") → servido VERBATIM por `GET /api/strategies/catalog`
  y `GET /api/detectors/catalog` (EMIT-07/08) → consumido por
  `frontend/lib/apex/schemas/strategies.ts:66` y `detectors.ts:64-65` (Zod
  `primary_ops`/`secondary_ops` como string[]).
- Esa cadena NO se alimenta de `knowledge_graph.jsonl`/`capability_matrix.json` —
  fluir op_32 al frontend exige regenerar `docs/quotebase_strategy_hop_map.json`
  (fuente workbook) + correr el generador. **Decisión de BOARD/orquestador**, no de
  HP-04 (archivos fuera de claim).

### (f) Verificación adversarial del espejo (HP-01-B — CONFIRMA a HP-04)

Probe literal del charter (`grep op_31|drl_agent|Op31` en `backend/api-server/src` +
`frontend/{lib,app,components}`): **0 matches**. El espejo no existe como enum/registro
TS de operadores — confirmo el hallazgo de HP-04 con evidencia propia (sincronía: no lo
contradigo, lo refuerzo). Detalle adicional que HP-04 no documentó:

- La forma del espejo es **parcial por diseño**: por ESTRATEGIA solo lleva
  `primary_ops` (fuente `docs/quotebase_strategy_hop_map.json`, 264 filas, keys
  `Primary_Ops`/`HopMask_u8`/`H2..H7`/`Min_Legs`/`Max_Legs` — sin secondary por
  estrategia); los `secondary_ops` viven por DETECTOR
  (`docs/quotebase_detector_policy.json`, 60 filas, keys `Primary_Ops`/`Secondary_Ops`/
  `Hop_Use`) → 60 ocurrencias de `"secondary_ops"` en `quotebase_catalog.ts`, 0 en el
  hop-map. Los ids aparecen como strings de display (`"op_27 Path Ordering"`,
  `quotebase_catalog.ts:61`), jamás como enum numérico o slug (`op_31`/`drl_agent`:
  **0 apariciones incluso como string** — consistente con §(c): op_31 tiene 0 edges).
- ÚNICA aparición de op_32 en TS hoy: `frontend/components/PreferenceVectorPanel.tsx`
  (HP-05, untracked) — es un COMPONENTE espejo de la matemática de preferencias
  (header declara "ESPEJO EXACTO de op_32:561-580"), no un registro de la matriz; y
  NO está cableado: grep de importadores = solo el archivo mismo.
- **Consecuencia normativa para HP-02/03/04/05**: fluir op_32 a catálogos/UI exige
  (1) columna op_32 en el workbook del OPERADOR (11_STRATEGY_HOP_MAP/13 detector
  policy) → regen de ambos JSON → `py scripts/gen_quotebase_catalog_ts.py`. Sin el
  paso (1) del operador, el espejo NO puede moverse sin romper DO-NOT-EDIT (P-∅).

## (d) HopMask/hops 2-7 del SEED vs hop tiers reales del scanner — HP-01-B

**Hallazgo central: el HopMask del SEED NO es un concepto nuevo — es canon vivo del
repo, y el SEED omite la SEGUNDA capa (tiers de ancla) que gobierna el despacho.**
El universo de hops real es de DOS capas; el SEED presenta solo la primera como si
fuera la frontera operativa completa.

### Capa 1 — Admisibilidad por estrategia (esto es lo que el SEED describe)

- `backend/searcher-rs/src/strategy_hop_mask.rs:1-18` — tabla estática
  `STRATEGY_HOP_MASKS: [(&str, u8); 264]` GENERADA del workbook
  `11_STRATEGY_HOP_MAP` (`docs/quotebase_strategy_hop_map.json`) por
  `py scripts/xls/gen_hopmask_rs.py` (fail-fast si el fuente drifta de los agregados
  del workbook). Encoding **idéntico al del SEED**: bit `h-2` para `h in 2..=7`,
  máscara 63 = todos los hops (`strategy_hop_mask.rs:9`). 264 estrategias →
  **1,436 combos Strategy×Hop válidos** (`:10`); distribución por hop (test
  `hop_distribution_matches_16_coverage`, `:404-410`): **245/262/260/233/233/203**
  para h=2..=7.
- Claim v1:161 "MEV-01 … hops 2-7 (HopMask 0b00111111=63)": **PARCIAL**. Censo de las
  36 máscaras MEV-01 (tabla `:19-54`): máscara 63 en **26/36** (arquetipo
  MEV-01-001..014, 023-026, 028, 030-036); máscara **62** (hops 3-7, excluye 2-hop)
  en 7 (MEV-01-018..022, 027, 029); y máscaras de hop único: MEV-01-015=1 ({2}),
  **MEV-01-016=2 ({3})** — el propio ejemplo triangular del operador confirma el
  canon—, MEV-01-017=4 ({4}).
- Claim v2:107-117 `StrategyConfig{min_hops, max_hops, hop_mask}`: el repo LO TIENE
  pero **repartido, no como struct por estrategia**: knobs globales
  min/max_hops (`backend/searcher-rs/src/canonical_knobs.rs:52`, default 2/7
  `:173`, env `ARBX_KNOB_MAX_HOPS` `:275`, validación rango canónico 2..=7
  `:388-397`) + máscara estática por MEV_ID consultada por tabla (binary-search,
  `strategy_hop_mask.rs:287-310`) e intersección
  `admissible_hop_bounds(mev_id, min, max)` (`:325-335`, consumer XLS-QB-03:
  multi-hop pass de `route_discovery_worker` + `discovery_workload.rs:42,259-262`).
  Los cartridges NO llevan campos hop (`grep hop|leg cartridge/types.rs` = 0).
  **P-∅: prohibido duplicar — el HopMask ya existe; el struct del SEED sería
  re-implementación de una tabla viva.**
- Envelope familiar: `detector_policy.rs:20-22` — `hop_use` (envelope de familia)
  INTERSECTA la máscara por estrategia (`envelope_hop_bounds`): una estrategia nunca
  escapa a su familia. Tercera restricción que el SEED tampoco menciona.

### Capa 2 — Tiers de ancla por CICLO (lo que el SEED omite; gobierna el DESPACHO)

`backend/searcher-rs/src/workers/route_scanner_worker.rs:160-183` (RU-3, pure gate):

| Hop count | Veredicto | Condición | Evidencia |
|---|---|---|---|
| 2..=3 | `Dispatch` (universo completo) | sin requisito de anclas | `:171`, `:178` |
| 4..=5 | `Dispatch` | ≥1 token ancla en el ciclo | `:172`, `:179` |
| 6..=8 | `ShadowForced` (telemetría, NUNCA despachado) | ≥2 anclas | `:165-166`, `:173`, `:180` |
| otro (<2, >8) | `None` (rechazado) | — | `:174-175`, `:181` |

- Anclas canónicas: `DEFAULT_ANCHORS` = 5 tokens mainnet WETH/USDC/USDT/DAI/WBTC
  (`:84`, `:204-207`); override env `ARBX_ROUTE_SCANNER_ANCHORS` (`:232-235`).
- Techo de búsqueda: `max_hops` clamp **2..=7** (`:247`; `DEFAULT_MAX_HOPS=7` `:69`).
  El brazo 6..=8 de la política es spec-complete por si el clamp abre — de ahí el
  vocabulario `Max_Legs=8` del workbook (visible en el espejo TS,
  `quotebase_catalog.ts:61` `"max_legs": 8`).
- Presupuesto por bloque: 500 ciclos + 250ms de enumeración (`:63-66`) — overshoot
  se REPORTA, no se oculta (R8, `:151-158`).
- **Consecuencia contra el SEED**: "CAPA DE HOPS (2-7 saltos)" (v1:50-51) es el
  conjunto ADMISIBLE (máscara), NO el despachable. Aunque una estrategia admita
  h=6-7 (203 estrategias admiten h=7), todo ciclo 6-7 es ShadowForced (observación);
  h=4-5 requiere ancla. La frontera operativa declarada por el SEED es
  **sobrestimada en una capa**: el scanner nunca despachará un 6-hop al pipeline de
  evaluación canónica por la vía RU-3.
- Evolución viva (branch actual, commit `27aca289` HOPS-LIVE-01):
  `ScannerConfig.canonical_per_block` (`route_scanner_worker.rs:218-227`) — ciclos
  rentables de RU-3 ahora TAMBIÉN se despachan por la evaluación canónica
  (`on_route_intent` → engines → sizing → emit), mode-invariante §34.1, cap bajo por
  costo. La admisión a S4 ya no colapsa stems: gate por ESTRUCTURA de ruta
  (`BR-00-APPLY.md` §0-1 — el censo lo cita como gate estructural obligatorio).
- Estado vivo medido por par: RU-3 degradado ~50× por el incidente de disco P0
  (5/264 bloques con ciclos, `tick_snapshot_set_failed` ×270/90min —
  `HP-07-DESIGN.md:19-28`). El universo de hops práctico está hoy limitado por la
  salud del pipeline (disco/PG/Redis), no por la máscara ni por las anclas.

## (e) Checklists/claims "[x]" del documento — lista literal — HP-01-B

**Precisión forense primero (fail-honest)**: los archivos SEED capturados NO
contienen marcas `[x]` literales — el "✅ CHECKLIST DE IMPLEMENTACIÓN" de v2
(`OPERADOR-SEED-V2-2026-09-08.md:763-802`) está **todo en `[ ]`**. Las referencias a
"[x]" son de la advertencia del orquestador (v1:9, v2:15) describiendo el mensaje
original del operador. El censo reconstruye la lista de claims DE COMPLETADO de las
frases declarativas y adjudica cada uno con DOS instantes — (i) al capturar el SEED
y (ii) HOY 2026-09-08 tarde, porque HP-03/HP-04/HP-05 aterrizaron en el working tree
DURANTE este programa:

| # | Claim literal del documento | (i) Al capturar el SEED | (ii) HOY (working tree) | Evidencia |
|---|---|---|---|---|
| 1 | "operador 32 desarrollado" (v1:17) · fila op_32 en tabla (v1:119) · `Op32NSGA2` (v2:152) | **FALSO** — repo tenía op_01..op_31; `mod.rs:6` era la INSTRUCCIÓN, no un registro | **AHORA EXISTE (uncommitted)**: `op_32_multi_objective.rs` (1,036 líneas; NSGA-II: `dominates`:96, `fast_non_dominated_sort`:114, `crowding_distance`:161, `evaluate_objectives`:228, run:325) + registro id 32 en `mod.rs` (`32 => MultiObjectiveOperator::new()`, marcado `// HP-03 (2026-09-08)`). Sin reporte HP-03 aterrizado al cierre de este censo — **HP-08 debe verificar** | `git status`: archivo `??` untracked |
| 2 | "op_32 ACTIVO" MEV-01 / "op_32 CRÍTICO" MEV-06 / MEV-08 (v1:161-163) | **FALSO** — 0 edges op_32 en el grafo (§(c) HP-04) | **PARCIAL**: 182 edges `USES_SECONDARY` ahora en kg/capability_matrix/STRATEGY.json (HP-04-APPLY §1, uncommitted); "ACTIVO" en runtime sigue **FALSO** — cartridges no lo declaran, `math_evidence.rs` evalúa por id contra registry, wiring de matriz pendiente (ver #6) | `HP-04-APPLY.md:16` (+182) |
| 3 | "Matriz de pruebas declarada (10 filas ✅ PASS)" (v1:164) | **SIN EVIDENCIA** — 0 tests op_32 en disco | **11 funciones test existen** (≠ la matriz de 10 filas del operador): 9 property tests dentro de op_32 (`:800` pareto_front_pairwise_nondominated, `:822` sort_vs_bruteforce, `:849` crowding_boundaries, `:875` preference_selects_front, `:924` single_objective_==op_15, `:950` bit_identical_determinism, `:980` bounded_budget, `:1003` r8_none_honest, `:1030` single_pool) + 2 de registry (`real_ops_tests.rs:456` dispatch_op_32, `:489` all_32_fail_honest). **Estado PASS = NO VERIFICADO por este censo** (no ejecuté cargo: AppControl 4551 + censo read-only) | grep `#[test]` = 9 en el archivo |
| 4 | UI "sliders Rentabilidad/Riesgo/Velocidad + toggle Pareto" (v1:156; lectura [x] del orquestador v1:9) | **NO EXISTÍA** | **Componente existe, SIN WIRING**: `frontend/components/PreferenceVectorPanel.tsx` (untracked) espeja la matemática de op_32:561-580 — pero **ningún page/component lo importa** (grep importadores = solo el archivo). No visible para usuarios; HP-05 sigue en curso | grep `PreferenceVectorPanel` |
| 5 | "desplegado/deploy" (lectura [x] del orquestador v1:9) | **FALSO** | **SIGUE FALSO** — NO-GIT vigente: 0 commit/push/PR/deploy de este programa; VPS corre código anterior (HP-07 midió HEAD e65040f1 con BR-00 SIN desplegar; pipeline además caído por disco desde 02:49Z) | `HP-07-DESIGN.md:12-14,19-28` |
| 6 | **BONUS (half-landed descubierto por este censo)**: la Master Matrix que el programa dice extender (264×31→264×32) | — | **DESAZADA VIVA**: registry ya despacha 32 operadores PERO `backend/math-engine/src/matrix/topology_map.rs:14` sigue `COLS: usize = 31` (sin diff). El working tree declara 32 en el registry y sigue proyectando 31 columnas — HP-03/HP-08 deben cerrarlo antes de cualquier claim de "matriz extendida" (HP-04-APPLY §6.3 ya lo anticipó) | `git diff topology_map.rs` = vacío |

Nota v2: su checklist (v2:763-802) está TODO `[ ]` — sin [x] falsos EN v2; el
sub-claim Fase 4 "MEV-01: 36 estrategias…" sin marcar es de hecho sub-claim (las 264
ya existen como cartridges, §(b)); los [x] aspiracionales viven en las frases
declarativas de v1 (tabla arriba).

## Mapeo normativo, invariante y gate (mitad B — para HP-02/03/05/08)

- **INV-HP01-D (invariante de dos capas)**: admisibilidad (máscara workbook, por
  estrategia) y despachabilidad (tiers de ancla, por ciclo) son capas DISTINTAS y
  JAMÁS se colapsan en una sola "CAPA DE HOPS 2-7". Cualquier diseño heredado del
  SEED (HP-02/HP-03) que suponga que "hops 2-7" es el conjunto despachable es
  defectuoso por construcción: h6-7 es observación estructural (ShadowForced), h4-5
  exige ancla. Análogo exacto de la directiva de BR-00: el gate es por ESTRUCTURA,
  no por etiqueta.
- **GATE-HP01-COLS**: nadie declara "matriz 264×32" hasta que
  `matrix/topology_map.rs:14` diga `COLS = 32` Y los tests de registry verdes
  (HP-08). El estado actual (registry 32 / COLS 31) es internamente inconsistente —
  es exactamente el tipo de half-landed que el censo existe para cazar.
- **GATE-HP01-CATALOG**: el espejo TS (quotebase) solo se mueve con columna op_32 en
  el workbook del OPERADOR → regen JSON → generador. Prohibido hand-editar
  `generated/quotebase_catalog.ts` (DO-NOT-EDIT, P-∅).
- **Para HP-05**: `PreferenceVectorPanel` existe pero está huérfano — su wiring a
  página existente de configuración (con snapshot R1) es el gap real, no el componente.
- **Para HP-07 (ya entregado)**: el "hops 2-7" del SEED no cambia el modelo
  económico — la frontera despachable h≤5 (h6-8 shadow) acota el universo de
  Holonomic Loop Resolution profundas que el SEED presume.

— HP-04 (strategy-architect), 2026-09-08. // HP-04 (2026-09-08)
— Extendido por HP-01-B (data-analytics PhD, RESPAWN-2 mitad B), 2026-09-08. // HP-01 (2026-09-08)
