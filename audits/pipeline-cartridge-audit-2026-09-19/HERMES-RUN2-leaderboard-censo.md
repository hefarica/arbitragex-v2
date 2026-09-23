# HERMES RUN 2 — Leaderboard de estrategias (PC-09d) + Censo estrategias (PC-09a/b)

Fecha: 2026-09-19 (UTC) · Agente: Hermes PhD analista de datos READ-ONLY · Checklist: CHECKLIST.md ítems #7 y #8.
Fuente: PostgreSQL del VPS, container `arbitragex-v2-postgres-1`, db `arbitragex`, tabla `opportunities`. Acceso: `ssh arbx` + `docker exec` (SELECT-only; SQL complejo pipeado por stdin con `docker exec -i`, sin archivos en el VPS). Cero escrituras en el VPS (RULE 00/R8 respetadas).

---

## 0. Resumen ejecutivo (fail-honest)

1. La ventana 7d contiene 9,116,267 detecciones, **todas `status='rejected'` (100%)**. `MAX(detected_at)` = 2026-09-19 21:00:38 UTC (pipeline vivo). Rango histórico de la tabla: 2026-07-05 → 2026-09-19.
2. El leaderboard crudo por `SUM(net_expected_profit_usd)` es **matemáticamente inválido**: 8,718,434/9,116,267 filas (95.6%) tienen `net_expected_profit_usd IS NULL`; el net USD positivo de la ventana proviene **exclusivamente** de self-pairs (`token_in = token_out`, par degenerado WETH/WETH): self-pairs $3,985,878.80 vs pares reales **−$762.33**.
3. Existe un evento de **multi-atribución masiva**: un único snapshot degenerado (pair `0xc02a…cc2/0xc02a…cc2` = WETH/WETH, 2026-09-18 13:47:12–14, ~1.2 s) emitió 34 oportunidades idénticas de net $49,844.951742, una por cartucho. Ese único evento aporta ~$1.69M del net bruto 7d y **contamina el top-20 completo del leaderboard crudo** (todas las estrategias "top" comparten exactamente ese max).
4. Estrategias de familia C-S-E reales (mev_01/mev_02 con cartridge_id) suman net positivo **solo vía self-pairs**; sobre pares reales el único net positivo es legacy `flashloan_arb` (+$8.24 en 127 filas con net computable) y `dex_arb` es **negativo** (−$770.57).
5. Censo: registry local = **264 cartuchos** `.rhai` en `backend/searcher-rs/cartridges/strategies/`. En 7d solo **48 strategy_kind distinct** han producido detecciones (45 del registry + 3 legacy sin cartucho: `dex_arb`, `triangular`, `flashloan_arb`; el 4º legacy, `backrun`, no aparece en 7d). **217 estrategias del registry NUNCA han producido una detección** (ni en toda la historia de la tabla).
6. Marco R8 verificado: `executions = 0`, `simulations WHERE passed = true = 0`. **Ninguna cifra de este leaderboard es ganancia realizada**; todo es `net_expected_profit_usd` (forecast de detección, previo a gates).

---

## 1. PARTIDO 1 — LEADERBOARD (PC-09d), ventana 7 días

### 1.1 SQL exacto usado (crudo, tal como se corrió)

Vía: `ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -A -F "|"'` (output completo conservado; el SQL complejo se pipeó por stdin con `docker exec -i`):

```sql
-- Liveness + volumen
SELECT now();
SELECT MAX(detected_at), COUNT(*) FROM opportunities WHERE detected_at > now() - interval '7 days';
SELECT COUNT(DISTINCT strategy_kind) FROM opportunities WHERE detected_at > now() - interval '7 days';

-- LEADERBOARD CRUDO TOP 20 (pedido literal PC-09d)
SELECT strategy_kind, COUNT(*) AS detections,
       ROUND(SUM(net_expected_profit_usd)::numeric,2) AS total_net_usd,
       ROUND(SUM(expected_profit_usd)::numeric,2) AS total_gross_usd,
       ROUND(AVG(net_expected_profit_usd)::numeric,6) AS avg_net_usd,
       ROUND(MAX(net_expected_profit_usd)::numeric,6) AS max_net_usd,
       MAX(detected_at) AS last_detection
FROM opportunities
WHERE detected_at > now() - interval '7 days'
GROUP BY strategy_kind
ORDER BY SUM(net_expected_profit_usd) DESC NULLS LAST
LIMIT 20;
```

Resultado (valores reales; todas las filas status=rejected, net dominado por self-pairs):

| # | strategy_kind | dets | total_net_usd | total_gross_usd | avg_net_usd | max_net_usd | last_detection |
|---|---|---|---|---|---|---|---|
| 1 | mev_01_023_path_inconsistency_arbitrage | 15026 | 131684.56 | 140168.16 | 9.116273 | 49844.951742 | 2026-09-19 16:22:13 |
| 2 | mev_02_001_constant_product_arbitrage | 15026 | 131684.56 | 140168.16 | 9.116273 | 49844.951742 | 2026-09-19 16:22:13 |
| 3 | mev_01_033_gas_adjusted_arbitrage | 15026 | 131683.67 | 140167.26 | 9.116211 | 49844.951742 | 2026-09-19 16:22:13 |
| 4 | mev_01_013_aggregator_dex_arbitrage | 15026 | 131683.66 | 140167.25 | 9.116210 | 49844.951742 | 2026-09-19 16:22:13 |
| 5 | mev_01_026_parallel_route_arbitrage | 15026 | 131683.52 | 140167.11 | 9.116201 | 49844.951742 | 2026-09-19 16:22:13 |
| 6 | mev_01_004_cross_version_arbitrage | 15026 | 131683.45 | 140167.04 | 9.116196 | 49844.951742 | 2026-09-19 16:22:13 |
| 7 | mev_01_014_router_pool_arbitrage | 15026 | 131683.29 | 140166.88 | 9.116185 | 49844.951742 | 2026-09-19 16:22:13 |
| 8 | mev_02_003_stableswap_arbitrage | 15026 | 131682.73 | 140166.32 | 9.116146 | 49844.951742 | 2026-09-19 16:22:13 |
| 9 | mev_01_031_exact_in_exact_out_arbitrage | 15026 | 131682.61 | 140166.20 | 9.116138 | 49844.951742 | 2026-09-19 16:22:13 |
| 10 | mev_02_006_liquidity_bin_arbitrage | 15026 | 131682.02 | 140165.61 | 9.116097 | 49844.951742 | 2026-09-19 16:22:13 |
| 11 | mev_02_017_cross_invariant_arbitrage | 15024 | 131671.55 | 140153.95 | 9.116634 | 49844.951742 | 2026-09-19 16:22:13 |
| 12 | mev_01_024_direct_vs_indirect_route_arbitrage | 15024 | 131671.55 | 140153.95 | 9.116634 | 49844.951742 | 2026-09-19 16:22:13 |
| 13 | mev_01_007_cross_liquidity_tier_arbitrage | 15024 | 131671.55 | 140153.95 | 9.116634 | 49844.951742 | 2026-09-19 16:22:12 |
| 14 | mev_01_005_cross_fork_arbitrage | 15024 | 131671.55 | 140153.95 | 9.116634 | 49844.951742 | 2026-09-19 16:22:13 |
| 15 | mev_01_032_fee_adjusted_arbitrage | 15024 | 131671.55 | 140153.94 | 9.116634 | 49844.951742 | 2026-09-19 16:22:13 |
| 16 | mev_02_004_weighted_pool_arbitrage | 15024 | 131671.52 | 140153.92 | 9.116633 | 49844.951742 | 2026-09-19 16:22:13 |
| 17 | mev_01_003_cross_protocol_arbitrage | 15024 | 131671.50 | 140153.89 | 9.116631 | 49844.951742 | 2026-09-19 16:22:13 |
| 18 | mev_01_034_rebate_arbitrage | 15024 | 131671.40 | 140153.80 | 9.116624 | 49844.951742 | 2026-09-19 16:22:13 |
| 19 | mev_01_001_dex_dex_arbitrage | 15024 | 131671.40 | 140153.80 | 9.116624 | 49844.951742 | 2026-09-19 16:22:13 |
| 20 | mev_01_035_dynamic_fee_arbitrage | 15024 | 131671.40 | 140153.80 | 9.116624 | 49844.951742 | 2026-09-19 16:22:13 |

(Filas 11–17 comparten total exacto por cuantización a 2 decimales; orden dentro del empate por diferencia sub-centésima. Tabla completa de las 48 en §1.3.)

**Lectura honesta del crudo**: las diferencias entre posiciones (131,684.56 → 131,671.40 = ~$13 en $131k) NO reflejan ventaja estratégica: son redondeo del mismo evento degenerado replicado a 34 cartuchos más un mismo volumen base (~15,024–15,026 dets/estrategia, casi uniforme). El leaderboard crudo no discrimina estrategias: mide fan-out de un mismo detector sobre un par WETH/WETH.

### 1.2 SQL exacto del leaderboard SANEADO (excluye self-pairs y net NULL)

```sql
SELECT strategy_kind, COUNT(*) AS detections,
       ROUND(SUM(net_expected_profit_usd)::numeric,2) AS total_net_usd,
       ROUND(SUM(expected_profit_usd)::numeric,2) AS total_gross_usd,
       MAX(detected_at) AS last_detection
FROM opportunities
WHERE detected_at > now() - interval '7 days'
  AND token_in <> token_out
  AND net_expected_profit_usd IS NOT NULL
GROUP BY strategy_kind
ORDER BY SUM(net_expected_profit_usd) DESC NULLS LAST
LIMIT 20;
```

Resultado REAL completo (solo 2 estrategias cumplen el filtro en 7d):

| # | strategy_kind | dets | total_net_usd | total_gross_usd | last_detection |
|---|---|---|---|---|---|
| 1 | flashloan_arb (legacy, sin cartucho) | 127 | **+8.24** | 25.29 | 2026-09-18 05:09:02 |
| 2 | dex_arb (legacy, sin cartucho) | 2706 | **−770.57** | 305.36 | 2026-09-19 16:43:50 |

**Veredicto PC-09d**: sobre pares reales y con net computable, el sistema en 7d NO tiene ninguna estrategia de cartucho (mev_XX) con volumen USD neto positivo; el único neto positivo es una estrategia legacy sin cartucho por $8.24. Persistir el leaderboard crudo en DB/UI sería violación de R8 (presupuestar como ganancia un forecast multi-atribuido sobre un par degenerado).

### 1.3 Caracterización completa de la ventana 7d (los 48 kinds)

SQL (stdin): `SELECT strategy_kind, COUNT(*), ROUND(SUM(net_expected_profit_usd)::numeric,2), MAX(detected_at) FROM opportunities WHERE detected_at > now() - interval '7 days' GROUP BY 1 ORDER BY 3 DESC NULLS LAST;`

Tres bloques claros:

Bloque A — fan-out C-S-E uniforme (28× mev_01 + 7× mev_02, todas con `cartridge_id` seteado, 0 NULL):
15,023–15,026 dets c/u; net $131.7k c/u (95.6% de esas filas con net NULL); incluye mev_01_015_two_leg en $81.8k. Última detección 2026-09-19 16:22:13 UTC (≈4.5h antes de la consulta: el burst de detección C-S-E había cesado a esa hora; los kinds legacy seguían vivos hasta 20:46+).

Bloque B — huérfanos de un solo evento: 7 kinds (mev_01_016/018/019/020/021/022/029) con EXACTAMENTE 1 detección, net $49,844.95 c/u, todas el 2026-09-18 13:47:13–14 sobre WETH/WETH. Junto con las otras 27 filas del mismo snapshot suman 34 oportunidades idénticas (verificado: `COUNT(*) WHERE ROUND(net,6)=49844.951742` = 34).

Bloque C — legacy sin cartucho (cartridge_id NULL en 100% de sus filas): `dex_arb` 7,754,970 filas (net −$770.57), `triangular` 902,092 (net NULL), `flashloan_arb` 364 (net −$99.46 en filas self-pair; +$8.24 en pares reales).

Bloque D — mev_04 (10 kinds: 001/006/008/010/011/012/014/019/021/022): 6,352–6,575 dets c/u, net 100% NULL, última detección 2026-09-19 18:30:52 UTC.

Métricas agregadas 7d (SQL stdin, `FILTER`):
- `status`: rejected 9,141,680 = 100.00% (único valor presente).
- `net_expected_profit_usd IS NULL`: 8,718,434 (95.64%); `< 0`: 16,707.
- Totales: net $3,985,116.47 · gross $904,731,217,614.79 (novecientos cuatro mil millones USD — prueba de que `expected_profit_usd` sin gates no es tamizable como volumen).
- Descomposición por par: self-pair (token_in=token_out) 1,387,810 filas → net $3,985,878.80; pares reales → net **−$762.33**.

### 1.4 Mecanismo de la multi-atribución (evidencia)

Las 34 filas de $49,844.951742: trace_id TODOS distintos (1 fila por trace — no es fan-out por trace), cartridge_id = nombre del kind (34 kinds distintos del registry), chain_id=1, pair `0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2/0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2` (WETH→WETH), status=rejected, emitidas en 1.16 s (13:47:12.954 → 13:47:14.114 del 2026-09-18). Es un snapshot degenerado broadcast a N cartuchos: cada cartucho "detecta" el mismo no-arbitraje WETH/WETH y el sistema persiste N oportunidades con el mismo net. La atribución estratégica es, por construcción, no-informativa en este régimen.

---

## 2. PARTIDO 2 — CENSO DE ESTRATEGIAS (PC-09a/b)

### 2.1 SQL exacto usado

```sql
SELECT DISTINCT strategy_kind FROM opportunities WHERE detected_at > now() - interval '7 days' ORDER BY 1;
SELECT DISTINCT strategy_kind FROM opportunities ORDER BY 1;
```

Censo del registry (local, repo): `ls backend/searcher-rs/cartridges/strategies/*.rhai | wc -l` = **264** (nombres sin extensión comparados como conjuntos contra strategy_kind en PG).

### 2.2 Resultado

| Métrica | Valor real |
|---|---|
| Cartuchos .rhai en registry (repo local) | 264 |
| strategy_kind distinct en PG, ventana 7d | 48 |
| strategy_kind distinct en PG, histórico (desde 2026-07-05) | 51 |
| Kinds en DB que NO existen como cartucho (legacy) | 4: `backrun`, `dex_arb`, `flashloan_arb`, `triangular` |
| Del registry detectadas en 7d | 45 |
| Del registry detectadas alguna vez | 47 |
| Del registry SIN detección en 7d | 219 |
| **Del registry SIN detección JAMÁS** | **217** |
| Detectadas históricamente pero no en 7d | mev_04_002_cross_stablecoin_arbitrage, mev_04_004_collateralized_stablecoin_arbitrage |

Nota sobre el "censo 269" del enunciado: la DB no contiene 269 kinds bajo ningún conteo (48 en 7d / 51 histórico); 264 (registry) + 4 legacy + ev_02_005_v3_concentrated (hallado solo all-time, ya contado) describe el universo observable. El número 269 del checklist (264+4+1) no es reproducible desde `opportunities.strategy_kind`; si refiere a 264 cartuchos + 4 legacy + 1 adicional, ese adicional no emite a esta tabla.

Distribución por familia (registry): detectadas alguna vez — mev_01: 28/36, mev_02: 7/17, mev_04: 12/31. Familias mev_03, 05, 06, 07, 08, 09, 10, 11: **0 detecciones históricas** (219 estrategias).

### 2.3 Las 217 estrategias que NUNCA han producido una detección

mev_01 (8): amm_clob, clob_clob, amm_rfq, rfq_rfq, quadrangular, basket, index, coincidence_of_wants (009,010,011,012,017,027,028,030)
mev_02 (10): proactive_market_maker, dynamic_liquidity, dynamic_weight, bonding_curve, oracle_priced_amm, virtual_amm, hybrid_amm_clob, twamm, batch_amm, auction_managed_amm (007–016)
mev_03 (31): TODA la familia — swap_backrun, multi_swap_backrun, cross_pool_ripple, top_of_block, end_of_block, inter_block, stale_price, latency, oracle_update, oev, rebase, interest_index_update, funding_update, epoch_rollover, settlement, mint_burn_state, redemption_state, liquidity_add, liquidity_removal, pool_initialization, migration, fee_switch, parameter_update, governance_execution, keeper_triggered, auction_clearing, block_time, sequencer_latency, spam, reorg_time_bandit, fork_state (001–031)
mev_04 (19): algorithmic_stablecoin, mint_redeem_stablecoin, canonical_bridged_token, synthetic_underlying, lp_token_nav, etf_like_token, rwa_nav, rebasing_token, fee_on_transfer_token, lst_redemption, governance_wrapper, vote_escrow_derivative, tokenized_position, principal_token, yield_token, pt_yt_parity, cross_maturity_yield, fixed_yield_floating_yield, points_pre_token (003,005,007,009,013,015,016,017,018,020,023–031)
mev_05 (14): TODA la familia CEX-DEX (001–014: cex_dex_spot, dex_cex_spot, cex_dex_triangular, cex_multi_dex, multi_cex_dex, cex_order_book_amm, cex_futures_dex_spot, cex_perpetual_dex_spot, cex_price_lead_latency, dex_price_lead, otc_dex, market_maker_inventory_dex, cross_custodian, fiat_stablecoin_dex)
mev_06 (30): TODA la familia cross-chain (001–030: l1_l1 … cross_chain_liquidation)
mev_07 (30): TODA la familia derivados (001–030: spot_perpetual … leverage_token_rebalancing)
mev_08 (25): TODA la familia lending/liquidaciones (001–025: borrow_rate … interest_accrual_timing)
mev_09 (20): TODA la familia intents/COW/auction (001–020: solver … searcher_builder_vertically_integrated)
mev_10 (18): TODA la familia NFT (001–018)
mev_11 (12): TODA la familia prediction markets (001–012)

(Lista completa exacta por nombre en `never_detected.txt`, temp local; generada por diff de conjuntos registry vs DISTINCT histórico.)

### 2.4 Lectura del censo

- La detección real del sistema se concentra en: 28 cartuchos mev_01 + 7 mev_02 (ambos con `cartridge_id` = fan-out C-S-E uniforme sobre el mismo flujo de ciclos) + 10 mev_04 (solo net NULL, sin profit computable) + 3 writers legacy sin cartucho que producen el 94.7% del volumen de filas (8.66M de 9.14M).
- "Concentrar más rutas sobre las estrategias top" (doctrina PC-09) es indistinguible de "concentrar sobre UN detector" mientras el fan-out siga asignando ~15,024 dets idénticas a cada cartucho del bloque A: el ranking dentro del bloque mide ruido de redondeo.
- 8 de 11 familias del registry (217/264 = 82.2% del arsenal) no tienen wired ningún detector que escriba a `opportunities`, o sus detectores no cumplen Preconditions (muchas requieren venue-types que el pipeline actual no escanea: CLOB, RFQ, CEX, cross-chain, derivados, NFT, prediction). El gap es estructural (alcance del scanner), no de calibración.

---

## 3. GAPS accionables derivados (para el checklist maestro)

G1 — **Leaderboard crudo no persistible** (bloquea cierre #7): persistirlo validaría $3.98M de net multi-atribuido sobre self-pairs. Requiere: excluir `token_in = token_out` en el writer o en el leaderboard, des-duplicar por snapshot (34 filas = 1 evento), y etiquetar como forecast (0 executions, 0 sims passed).
G2 — **Gate de self-pair ausente**: 1,387,810 filas 7d con token_in=token_out pasaron a PG; un par degenerado no es oportunidad. Corresponde al path de detección (skill arbitragex-defect-hardening si se confirma defect), no al leaderboard.
G3 — **Multi-atribución cartucho↔evento**: sin clave de agrupación (trace_id es por fila), todo ranking por estrategia es fan-out disfrazado. Necesita correlation-id de snapshot/round para poder atribuir 1 evento → 1 estrategia ganadora.
G4 — **Cobertura 217/264**: 8 familias sin ninguna detección histórica; la matriz operador→pipeline (PC-09b) no puede certificarse para ellas sin expandir los venue-types del scanner o declararlas fuera de alcance vigente.
G5 — **Net NULL 95.6%**: el writer deja net vacío en la gran mayoría de filas (coincide con bloque C/D); sin net, el orden USD desc no es computable — pre-requisito del propio leaderboard.
G6 — **Kinds legacy sin cartucho** (dex_arb/triangular/flashloan_arb/backrun, cartridge_id NULL): conviven con el registry y distorsionan cualquier censo de cartuchos; decidir migración o retiro (decisión gated del operador).

## 4. Errores encontrados durante la corrida (transparencia)

- Dos comandos ssh con SQL anidado en literales fallaron con `bash: eval: unexpected EOF while looking for matching quote` / `syntax error near unexpected token '('` (limitación de quoting ssh+docker+psql, no de la DB). Resolución: pipear SQL por stdin (`docker exec -i`). Ningún query de datos falló en ejecución; no hay resultados fabricados.

## 5. Reproducibilidad

Comandos base (todo read-only):
`ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -A -F "|"'` con los SQL de §1.1/§1.2/§1.3/§2.1 (los literales `interval '7 days'` van dentro del único nivel de comillas del `-c` o por stdin).
Censo registry: `ls backend/searcher-rs/cartridges/strategies/*.rhai | sed 's/\.rhai$//' | sort` → diff de conjuntos vs `DISTINCT strategy_kind`.
