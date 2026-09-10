# BR-00 -- APPLY: cierre del P0 (D-SIM-01, desajuste detector-simulador)

> **WO:** BR-00-APPLY - **kind:** apply - **agente:** ecc:rust-reviewer (Gang Omniscience, mesa CEREBRO)
> **Fecha:** 2026-09-08 (arbol feat/hops-live-01 @ 27aca289 + cambios de pares no-Rust).
> **Insumos:** BR-00-VERIFY.md (baseline + protocolo 1.4/5), BR-01-FORENSE-EMBUDO.md 3.3,
> BR-03+04+07-VERIFY.md, directiva del operador 2026-09-07 (stems jamas colapsados;
> decision por ESTRUCTURA DE RUTA).
> **NO-GIT cumplido:** 0 commit / 0 push / 0 PR. VPS: 0 acceso.

## 0. Que aterrizo (resumen ejecutivo)

El gate `opp.strategy_kind != StrategyKind::dex_arb()` (la 1/40 del censo) fue ELIMINADO.
La admision al simulador S4 ahora se decide por **ESTRUCTURA DE RUTA** -- la forma que el
tx_builder ya sabe construir (swap de un solo hop V2/V3 con dos tokens distintos y un router
del catalogo en `dex_a`) -- nunca por igualdad de string con "dex_arb", y **jamas colapsando
stems**: `strategy_kind` fluye end-to-end sin mutacion y cada rechazo lleva la kind EXACTA
en el reason. Tres archivos tocados en `backend/sim-ctl/src/`, todo hunk marcado
`BR-00 (2026-09-07)`:

| Archivo | Hunks |
|---|---|
| `tx_builder.rs` | gate estructural + CyclicRouteNotRepresentable + helper is_non_swap_strategy_kind + 5 tests (uno reemplaza a non_dex_arb_rejected) |
| `sim_engine.rs` | reasons per-kind (2 familias) + decode por forma ABI + fail_reason honesto output_undecodable + 3 tests de decode |
| `persistence.rs` | clasificador de capability-gaps absorbe las 2 familias nuevas + output_undecodable + 1 test |

**BONUS -- bug pre-existente encontrado y corregido por los tests nuevos:** el decoder
decode_amount_out intentaba uint256 ANTES que uint[]; para retornos V2 (uint[]) la palabra
head es el offset de datos (32), asi que TODO probe V2 exitoso decodificaba amountOut=32
(constante) y computaba slippage contra basura. Ahora array-primero. El bug existia desde
antes de BR-00 (estaba gated a dex_arb, o sea activo en el 100% del flujo simulado V2 de
hoy). Lo encontro el test nuevo decode_v2_array_shape_takes_last, que FALLO con Some(32)
vs Some(2) antes del fix.

## 1. Clasificacion EXACTA de kinds (vertiente a + b)

La regla operativa es el PAYLOAD, no una enumeracion de nombres (directiva del operador:
cada cartridge stem es UNO solo y no se parece a ningun otro). Aplicada al censo de 40:

### 1.1 Kinds admitidas al probe (vertiente a -- MISMO probe que dex_arb recibe hoy)

Regla: cualquier kind/stem cuyo payload sea un hop de dos tokens distintos
(token_in != token_out) con dex_a resoluble en el catalogo de routers V2/V3.

- dex_arb -- comportamiento invarierto (regresion cubierta por test).
- Todo stem de cartridge (mev_01_* ... mev_11_*, 264 stems canonicos; ~36 de ellos en el
  censo de 40 vivo) -- el payload del cartridge path ES un hop (cartridge_boot.rs:1138-1160:
  token_in = primer leg in, token_out = ultimo leg out, dex_a = dex_hint del primer leg).
  Rutas ABIERTAS simulan; rutas CERRADAS caen en 1.2.
- backrun y cualquier re-rotulacion swap-shaped -- la kind no decide, el payload si
  (test unknown_open_route_kind_builds con mev_99_001_brand_new_kind).

Declaracion de fidelidad (RULE 00): el probe S4 es y sigue siendo un probe de UN hop
(semantic pre-existente documentada: "S5 adds counter-trade"). Para kinds admitidas con
rutas multihop, el probe mide el hop directo token_in -> token_out en el router de dex_a:
si ese hop no existe como pool, el eth_call revierte en el fork y la sim muere con
reverted:* (fail-closed real, no maquillaje). El resultado de la sim es una afirmacion
sobre el PROBE, igual que hoy para dex_arb.

### 1.2 Kinds rechazadas con label NUEVO especifico (vertiente b -- per-kind/per-stem)

(i) Cyclicas -- strategy_cyclic_route_not_simulatable_in_s4:KIND (nueva familia):
token_in == token_out (comparacion de Address parseada, case/prefix-proof). El probe de
un solo hop no puede representar una ruta cerrada y el payload de Opportunity NO lleva los
hops intermedios para reconstruirla. Cubre por construccion (verificado en codigo emisor):
- triangular -- triangular_worker.rs:1529-1530 SIEMPRE emite token_in==token_out (ciclo A-B-C-A).
- flashloan_arb -- flashloan_arb_worker.rs:1085-1086 SIEMPRE token_in==token_out.
- stems de cartridge con rutas cerradas (la mayoria de los ARB ciclicos) -- por payload.

(ii) No-swap estructurales -- strategy_not_simulatable_in_s4:KIND (familia vieja, ahora
con la kind exacta como sufijo -- cierra el gap 2.3 de BR-00-VERIFY):
- liquidation y liquidation_snipe -- topologia repay-debt + seize-collateral contra un
  pool Aave V3 (liquidation_worker.rs:1042-1069: dex_a = "aave-v3:<pool>", monto en Aave
  base units). No es un swap de router, jamas lo sera con este tx path; el rechazo por
  kind da el reason especifico en vez de un router-not-in-catalog enganoso. Match
  case-insensitive (drift PascalCase = clase de anomalia 2026-08-18).

El label VIEJO sin sufijo queda congelado en la historia de PG -- NINGUNA fila nueva lo
lleva (0 emission sites). Backward-compat de dashboards que agrupan por prefijo
split_part(fail_reason, dos puntos, 1): preservada para la familia (ii); la (i) es una
familia nueva distinta a proposito.

### 1.3 Impacto esperado sobre la metrica del P0

Del flujo que hoy muere en strategy_not_simulatable_in_s4:
- cyclicas (triangular 9.2% del mix post-#555, flashloan, cartridges cerrados) -> familia (i).
- liquidation* -> familia (ii) con sufijo.
- cartridges con ruta abierta + par real -> AHORA SIMULAN DE VERDAD (pasaran a
  reverted/build_error/slippage -- rejects que siguen siendo rejects, con evidencia real).
La cuota prefix-strategy_not_simulatable_in_s4 queda reservada SOLO a no-swap kinds.
El mix-shift advertido en BR-00-VERIFY 1.4 sigue aplicando: la medicion post-deploy DEBE
usar el protocolo 1.4 (era-separada + conteos absolutos por kind + query de seccion 4).

## 2. Diffs por archivo (ver git diff backend/sim-ctl/ para el texto exacto)

### 2.1 backend/sim-ctl/src/tx_builder.rs
1. Doc de modulo: scope re-declarado (estructura, no igualdad de string).
2. BuildError::CyclicRouteNotRepresentable(StrategyKind) -- variante nueva tipada.
3. Gate: != dex_arb eliminado -> is_non_swap_strategy_kind() (solo
   liquidation/liquidation_snipe, case-insensitive) + guard token_in == token_out
   (Address parseada) ANTES de encode. Nada mas del flujo cambia.
4. Helper is_non_swap_strategy_kind con justificacion file:line de los workers.
5. Tests: non_swap_kinds_rejected (incl. "Liquidation" PascalCase),
   cyclic_routes_rejected_with_typed_error (triangular/flashloan/stem-016 con fixture
   ciclico), cartridge_stems_and_relabels_build_probes (dex_arb/backrun/4 stems ->
   selector V2 real 0x38ed1739), unknown_open_route_kind_builds (fail-open solo para
   payload estructuralmente sano; jamas panic). non_dex_arb_rejected (viejo) fue
   REEMPLAZADO -- codificaba la taxonomia del bug.

### 2.2 backend/sim-ctl/src/sim_engine.rs
1. Reasons per-kind: strategy_not_simulatable_in_s4:KIND y
   strategy_cyclic_route_not_simulatable_in_s4:KIND (kind.as_str() = stem EXACTO).
   Via not_implemented (simulator=not_implemented) -- continuidad de metrica.
2. decode_amount_out(&Bytes): firma sin opp (la kind ya no decide el decode);
   array-primero (fix del bug amountOut=32, ver seccion 0).
3. fail_reason honesto: output_undecodable cuando el output del probe no decodifica
   (antes: slippage_too_high mendaz -- R8: None != medicion).
4. Tests br00_decode_tests: uint256 (V3), uint[] last (V2), garbage/empty -> None.

### 2.3 backend/sim-ctl/src/persistence.rs
1. is_sim_capability_gap: + prefijo strategy_cyclic_route_not_simulatable e igualdad
   exacta output_undecodable. SIN esto, los ciclicos habrian REJECTADO la oportunidad
   (regresion semantica: un limite de forma del simulador no es veredicto de mercado --
   misma doctrina SIMWIRE-02 del clasificador).
2. Test br00_structural_gap_families_are_gaps (6 reasons, incl. stems con sufijo).

## 3. Verificacion local (output REAL, ejecutado 2026-09-08)

- cargo fmt --check -p sim-ctl: LIMPIO (sin output).
- cargo check -p sim-ctl: Finished dev profile in 43.42s, 0 errores.
- cargo clippy -p sim-ctl -- -D warnings: Finished dev profile in 1m 23s, 0 warnings.
- cargo test -p sim-ctl (Windows AppControl NO bloqueo los exes de test esta vez):
  - unittests src/lib.rs: 5 passed, 0 failed -- incluye drain_guard_authorizes_per_backend_never_a_common_or y missing_executor_composes_with_drain_guard_refusal (los dos tests drain-guard exigidos por el charter: VERDES).
  - unittests src/main.rs: 44 passed, 1 ignored (37 baseline + 8 nuevos - 1 reemplazado).
  - simwire02_pel_recovery: 1 passed (33.45s).
  - simwire02_route_aware: 8 passed, 1 ignored (live-env, honesto).
  - simwire02c_redelivery_idempotency: 8 passed, 0 failed.
  - TOTAL: 66 passed, 0 failed, 2 ignored (mismos 2 ignores del baseline).

Greps RULE 00 sobre el diff (protocolo BR-00-VERIFY 2.2):
- passed:true / passed = true anadidos no derivados: 0.
- mock/stub/fake anadidos: 0.
- Ningun SimulationResult nuevo en el diff (los reason-nuevos van por Self::not_implemented,
  la via existente).
- Strings de modo (ARBX_TRADE_MODE / PAPER_SHADOW / LIVE_MAINNET / SIM_BACKEND) en el diff: 0
  (doctrina 34.1 -- hot-path mode-invariante).
- backend/relays-client/: 0 diffs (34.3 INTACTO).
- Archivos en claim de pares (searcher-rs lib.rs/main.rs/runtime_knobs.rs, frontend/**,
  docker/compose.prod.yml): 0 diffs.

## 4. Query PG de la metrica pre/post (era-separada, protocolo 1.4)

Query POST-FIX (reemplazar TS_DEPLOY por el timestamz del deploy veraz del fix -- JAMAS
promediar con era pre-fix), familia por prefijo + desglose por kind exacto:

  SELECT COALESCE(split_part(s.fail_reason, ':', 1), '(PASS)') AS reason_family,
         s.simulator, s.passed, count(*),
         round(100.0*count(*)/sum(count(*)) over (), 2) AS pct
  FROM simulations s
  WHERE s.simulated_at >= TS_DEPLOY
  GROUP BY 1,2,3 ORDER BY 4 DESC;

  SELECT s.fail_reason, count(*)
  FROM simulations s
  WHERE s.simulated_at >= TS_DEPLOY
    AND s.fail_reason LIKE 'strategy_%not_simulatable_in_s4:%'
  GROUP BY 1 ORDER BY 2 DESC;

Ejecutarlas via: ssh arbx + docker exec arbitragex-v2-postgres-1 psql -U postgres -d
arbitragex -At -F "|" -c "QUERY". PRE (baseline publicado): misma query con ventana 2h/24h
pre-deploy; BR-00-VERIFY 1.2/6 dio 66.47% de 4,014 sims en 2h post-#555, 0 passed.

Gate de exito del charter: share de la familia strategy_not_simulatable_in_s4 (prefijo) <
50% en la era post-fix. El match EXACTO (fail_reason = strategy_not_simulatable_in_s4 sin
sufijo) solo puede devolver filas HISTORICAS: 0 emision nueva de ese string exacto.

Invariantes de honestidad al medir (BR-00-VERIFY 1.4): (1) ventana post-deploy pura;
(2) conteos ABSOLUTOS por kind (query 2) ademas del share; (3) BR-04 no debe haber corrido
en la ventana (board); (4) XLEN arbx:opps:simulated como prueba de vida del terminus;
(5) comparar el mix por kind vs baseline 1.1.

Columna: simulations.fail_reason es TEXT (migracion database/migrations/004_simulations.sql
linea 14) -- los labels con sufijo (~50-90 chars) no truncan; la columna ya persiste
revert-strings de 200 chars en prod.

## 5. Caza de stem-stomping (directiva del operador #3) -- RESULTADO

Grep de TODOS los call-sites de to_contract_strategy_kind( en backend/:

- Sobre opps/candidates de cartridge: 0 SITIOS. El path de cartridge (cartridge_boot.rs:1156)
  construye su Opportunity con StrategyKind::cartridge(stem) y la pasa directo al
  ConfigAwareEvaluator -- ningun engine la re-etiqueta. El fix BR-00 tampoco: tx_builder
  solo LEE opp.strategy_kind.as_str() para el reason; jamas lo escribe.
- Los unicos sites que mutan una opp existente son flashloan_engine.rs:326,361
  (clone_for_rejection / build_wrapped_candidate): clonan candidates NATIVOS del
  scanner/orchestrator (label ya base-kind) y los envuelven como flashloan -- semantic
  documentada y pineada por tests propios. No tocan stems. No habia defecto que corregir;
  ademas searcher-rs esta en claim de CB-02 (intocable para BR-00).
- HALLAZGO para la mesa (documentado, NO corregido -- fuera de scope): inconsistencia
  HISTORICA de kinds emitidas. Algunos engines emiten kinds de CONTRATO (triangular via
  triangular_worker.rs:1520) y otros labels ANALITICOS como strategy_kind: triangular_arb
  (triangular_engine.rs:637), spanning_tree_arb (spanning_tree_engine.rs:540),
  liquidation_snipe (liquidation_snipe_engine.rs:400), cross_chain_arb
  (cross_chain_bridge_engine.rs:367); cartridge_category solo en RoutePlan
  (cartridge_boot.rs:1268). Con el gate estructural esto deja de bloquear la simulacion
  (el payload decide), pero el censo "40 kinds" mezcla dos nomenclaturas -- unificarla es
  un WO de searcher-rs (CB-02).

## 6. Limites declarados (fail-honest)

1. Probe de UN hop: rutas multihop reales no se reconstruyen (el payload no trae los hops
   intermedios). El probe mide el hop directo en dex_a; revert si no existe pool directo.
   S5 (counter-trade + quoter) sigue siendo el camino a fidelidad completa.
2. Cyclicas NO simulables hasta que el payload carrye hops (triangular/flashloan/cerradas):
   mueren con label especifico per-kind -- gap DECLARADO, no un fix.
3. mev_03_* y demas stems: admitidos SOLO si su payload es un par abierto con router
   resoluble (estructura); si no, mueren en el guard ciclico o en el catalogo de routers
   con reasons honestos. No certifico semantic per-stem de 264 cartuchos -- certifico que
   el decision procedure es estructural y fail-closed (kinds nuevas jamas panickean:
   caen en estructura o en label honesto).
4. output_undecodable es gap (no reject): el harness no pudo leer su propio probe.
5. El bug del decoder (seccion 0) se corrigio en el orden de decode; los datos HISTORICOS
   de slippage computados con amountOut=32 en probes V2 no se re-procesan.

## 7. Entrega a la mesa

- Re-verify BR-00 (cs-validator): protocolo BR-00-VERIFY 1.4 + queries seccion 4; exigir
  git diff con marcadores BR-00 (2026-09-07) en los 3 archivos; greps seccion 3 = 0.
- BR-02 / BR-03: al abrirse la compuerta de stems abiertos, missing_reserves_pool_b y los
  oraculos suben como proximos cuellos (ya advertido en VERIFY 2.4).
- BR-01: la metrica exact-match del label viejo queda congelada en historia -- actualizar
  paneles que agrupen por fail_reason exacto a split_part de primer segmento.
- Orquestador: PR unico BR-00 (3 archivos sim-ctl + este doc). NO-GIT respetado por el
  builder: el commit/PR es del orquestador.
