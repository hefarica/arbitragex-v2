# 01 — CONTRATO CANÓNICO DE IMPLEMENTACIÓN (PC-01)

Derivado de los 2 workbooks del operador (extract en `xlsx_extract/`, 20 hojas).
Clasificación: CANONICAL_WORKBOOK. Cada regla cita hoja. Fecha: 2026-09-19.

## 1. Identidad y dinamicidad (QB_00_MANUAL)

- TokenKey = (chain_id, address). **Symbol NUNCA es clave de runtime** — es metadato.
- N_c = tokens activos por cadena; P_c=C(N_c,2) pares; D_c=N_c(N_c-1) direcciones.
  Prohibido constantes 22/231/462 en lógica runtime (regla de no-hardcode).
- Grafo = MULTIGRAFO dirigido: pares paralelos (múltiples pools/DEX/fee tiers) NO se
  colapsan. Elegir QUOTE/BASE no elimina la dirección inversa.
- Invalidación por versiones: `allowed_set_version`/`topology_version` → rebuild índices;
  cambio de estado/bloque → solo dirty repricing.

## 2. Pipeline canónico (QB_00_MANUAL "PIPELINE CANÓNICO" + QB_15)

1. AllowedList → resolve chain/address → dense IDs + PairIndex → multigrafo pools/actions.
2. Evento bloque/log → dirty IDs → repricing en memoria → prefilter ineficiencia.
3. Hot seed → adjacency/bitset → máscara estrategia×hops 2..7 → expansión acotada.
4. Route skeleton → matemática EXACTA de protocolo → sizing/financing/gas → **net-profit gate**.
5. PASS → simulación (fuera del SLA <30ms) → plan de ejecución → ejecución privada/autorizada.

Latencia: <30 ms aplica SOLO a discovery/ranking pre-simulación (QB_00 "SLA").

## 3. Quote/Base (QB_05)

- QuoteScore(t)=wP·Prior+wL·Liquidity+wV·Venues+wS·Stability+wX·CrossDex; jerarquía
  dinámica > lista hardcodeada de stablecoins.
- BASE/QUOTE = 1 BASE = P unidades QUOTE; solo numeraire/UI/valoración.
- Fair rate desde anchor: r*_{A→B}=P_Q(A)/P_Q(B); factor normalizado F_e=r_e·P_Q(dst)/P_Q(src);
  **F_e>1 es señal, NO oportunidad** — la oportunidad exige beneficio neto exacto post-costos.

## 4. Edge math por adaptador — NO hay fórmula única (QB_06)

| Adaptador | Quote | Crítico |
|---|---|---|
| CPMM/V2 | Δy = y·(γΔx)/(x+γΔx) | fee γ sobre input; entero exacto |
| CLAMM/V3/V4 | tick traversal sqrtPrice/liquidity | spot price INSUFICIENTE para amount-aware |
| StableSwap | invariante D/A + Newton | balances + amplificación |
| Balancer | V=Π B_i^{W_i} outGivenIn | pesos + scaling |
| CLOB/RFQ | VWAP/depth fill | jamás top-of-book si excede profundidad |

## 5. Detectores — política (QB_13, 60 detectores)

- Dos capas: DISCOVERY (prefiltro/routing) ≠ EVALUATION (gates + sizing + EV).
- Detector DEX_AMM clave: **R_CLOSED_CYCLE** (25 estrategias):
  `Q_R(x)=q_n(...q_2(q_1(x))); Π_R(x)=Q_R(x)−x−C_R(x)`; oportunidad iff max_x Π_R(x)>0.
  Prefiltro marginal: Σ_e[−ln((1−fee_e)·rate_e)]<0. Ops primarias: op_27 Path Ordering,
  op_21 Newton, op_15 Golden, op_16 Kelly.
- R_DIRECT_INDIRECT: Δ(x)=Q_indirect(x)−Q_direct(x)−C_incr(x); mismo input/estado en ambas.
- OBSERVE (8): sin is_opportunity=true jamás — solo evidencia estructurada.
- Regla transversal: señal (log/quote/spread) NUNCA es profit; PASS solo con net exacto
  amount-aware después de TODOS los costos.

## 6. Algoritmos de discovery (ULTRA_04)

- BFM_NEG_CYCLE (señal de ciclo negativo O(V·E)) + MMBF_LINE_GRAPH: habilitados, no exhaustivos.
- BOUNDED_DFS: enumerador exhaustivo acotado a k=7 con poda (operacional).
- RICH (PVLDB 2025): priorizador del ciclo más negativo por k-hops; ~0.02–3.9% error
  relativo → candidato, NO oráculo de completitud.
- JOHNSON: solo referencia de completitud en subgrafos pruned (output-explosivo).
- CONVEX_SIZE + MPO: sizing/ranking post-discovery, jamás sustituto del discovery.

## 7. Financing (ULTRA_02)

- Modo = filtro de rutas/sizing, NO tesorería: OWN_CAPITAL (0 fee), AAVE_FL (5bps, gov-updatable
  → leer fee de cadena, no hardcodear), BALANCER_FL (0 fee), V2_FLASH_SWAP (30bps pool fee).
- Sizeable(route,mode)=MIN(required_notional, bottleneck_liquidity×util_cap, provider_capacity,
  optimal_size). EV = gross − provider_fee − gas_adj − slippage_proxy − risk_haircut.

## 8. Gates (ULTRA_07)

G0 datos (>0 pools) → G1 discovery (>0 candidatos) → G2 budget (≤ config) → G3 financing
enabled → G4_ECON (**≥1 ruta viable >0** ← el gate hoy en FAIL productivo: 0 viables) →
G5 latencia (algoritmos caben en CPU) → G6 ejecución SHADOW/PAPER canónico.
Acción si G4 FAIL: "Review EV/gas/liquidity/financing" — exactamente la pregunta PC-03.

## 9. Contrato de pasos con verificación (QB_15, resumen)

Step 8 PairPrefilter (~1–3ms, fixtures de spread conocido) · Step 9 StrategyMask
(264×6 cobertura) · Step 10 RouteExpand (~3–8ms, fixtures de ciclos) · Step 11 ExactQuote
(diferencial vs on-chain quoter) · Step 12 Sizing (tests de óptimo/propiedad) · Step 13
NetGate (~1–3ms, descomposición de costos) · Step 14 Simulación (replay/fork).

## 10. Definition of Done (QB_15)

- 264 filas de estrategia mapeadas; strategy×hop 2..7 generadas; 60 detectores representados.
- Sin constantes 22/231/462 en runtime; symbol nunca clave; pools paralelos preservados.
- Cada Strategy_ID conserva SU Discovery_Equation, ops, surface, gate y rango de hops.
- Profit truth: PASS = net exacto amount-aware post-costos; señales son solo prefilter.
