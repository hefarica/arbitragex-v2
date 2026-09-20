# BOARD — pipeline-cartridge-audit-2026-09-19

**/GOAL (operador, 2026-09-19)**: Revisar el pipeline y cómo están construidos los cartuchos
de las estrategias de arbitraje; garantizar que estén perfectamente implementados para
descartar que errores o malos procedimientos impidan ejecutar y detectar en tiempo real
arbitrages con ganancia > 0. Guía canónica de implementación (aprender al 100%):
`C:\Users\HFRC\Downloads\ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx` (24 hojas) +
`C:\Users\HFRC\Downloads\ArbitrageX_Route_Strategy_Optimizer_264_ULTRA.xlsx` (21 hojas).
Extract canónico: `xlsx_extract/` (20 hojas).

**Perímetro**: repo local + lectura VPS (`ssh arbx`) + dominio vivo read-only. ZERO MOCKS
(RULE 00), fail-honest (R8). CERO commits/push/deploy sin gate final del operador.
Capital/flips = operador-only (§34). Cripto/matemática → vector independiente + Hermes (§16).

**Contexto**:
- 0 viables/6h hoy (703K detecciones 100% rejected; 83% = v3_quote_unavailable — clase
  cobertura de datos según LEARNINGS 09-17, no transporte).
- 2 paths de detección: orchestrator engines (sin cartridge_id) + cartridge layer (Rhai).
- Cartridges: `backend/searcher-rs/cartridges/strategies/` (264 Rhai), boot en
  `cartridge_boot.rs`, migración v3 (3886aa90).
- S1 fee dual-unit LANDED (cdb4c890): fee_fraction() divisor por tipo de pool
  (V2=bps/1e4, V3=pips/1e6).
- WO-LEGS-TRIANGULAR-01 (ab880676): per-leg ledger.
- Gang anterior run_00a4aa… FAILED por 500 del proveedor (muerte aislada, gateway healthy).

## Work Orders

| WO | Título | Dueño | Estado | Gate |
|---|---|---|---|---|
| PC-01 | Digerir los 2 workbooks (extract xlsx_extract/) y derivar el CONTRATO canónico de implementación: quote base, edge math, ineficiencia, hops 2-7, detector policy, financing, algoritmos, gates, catálogo 264×operadores | orquestador + gang | **DONE (orquestador 2026-09-19)** | `01-CANONICAL-CONTRACT.md` escrito con cita hoja por regla |
| PC-02 | Censo del cartridge layer: 264 cartuchos Rhai — estructura, parámetros, families/hops, vs catálogo ULTRA 11_STRATEGY_CATALOG + QB 11_STRATEGY_HOP_MAP; lista de desviaciones | gang (read-only) | IN PROGRESS — censo estructural orquestador: 264/264 nombres, min/max_legs, detector_id, execution_class = EXACTOS vs catálogo (0 desviaciones estructurales). Falta revisión semántica gang (math por detector) | diff canónico vs repo con evidencia |
| PC-03 | Auditoría del pipeline de detección en tiempo real: ¿por qué 0 viables? Verificar cada gate del funnel (v3_quote_unavailable 83%, spot_product_le_one, non_positive_profit) contra 07_GATES/13_DETECTOR_POLICY — ¿algún gate está mal calibrado/bug que mate arbitrages > 0 reales? | gang + orquestador | OPEN | veredicto por gate: correcto / bug / mal procedimiento, con evidencia |
| PC-04 | Matemática de quotes/edges: validar contra 04_INDEX_MATH/05_QUOTE_BASE/06_EDGE_MATH + vector independiente (Hermes §16); incluye S1 fee dual-unit | gang | OPEN | vectores de referencia reproducibles |
| PC-05 | Síntesis: plan de correcciones priorizado (si hay bugs) — SIN implementar sin gate del operador | orquestador | OPEN | informe final + veredicto Hermes |

| PC-06 | **Doctrina operador (mid-turn 2026-09-19)**: discovery debe buscar arbitrajes sobre TODO el universo de tokens de la red, SIN distinción; la lista blanca de ~22 tokens es SOLO filtro de reflejo de oportunidades (qué se muestra/emite), NUNCA restricción del grafo de búsqueda. Verificar que ninguna capa filtre el discovery por allowlist | orquestador | IN PROGRESS | mapa de puntos donde el allowlist toca discovery vs reflejo |
| PC-07 | **Orden operador (mid-turn 2026-09-19 #2)**: la lista blanca debe ofrecer preset "TOP-100" — las 100 criptos más valorizadas (mayor capitalización / mayor valor en pools), bajo riesgo. La 1ª generación de rutas sigue siendo universo-completo; el top-100 es sólo el filtro de reflejo. NOTA técnica: Zod `allowed_token_symbols` hoy `.max(64)` (trading-config.ts:146) → subir cap a ≥100. Fuente de ranking: valor on-chain en pools (reserves × torre de precios, RULE 00 verificable); market cap externo (CoinGecko) = decisión pendiente del operador | orquestador | IN PROGRESS — endpoint GET /api/tokens/top (token-top.ts) + menú UI Top 20/30/40/100 × ventana current/24h + 10 toggles anti-rug/scam (incluye detección memecoins) | diseño + endpoint de preset |
| PC-08 | **Doctrina operador (mid-turn 2026-09-19 #3)**: objetivo #1 del discovery = buscar SIEMPRE las rutas más exóticas orientadas a máxima rentabilidad; ordenar resultados por Topological Yield en USD de MAYOR a MENOR; el algoritmo debe identificar permanentemente las más exóticas/exóticas-por-dólares. Verificar el priorizador actual (RICH por ciclo más negativo, ULTRA_04) cumple: ranking por USD descendente + búsqueda continua | orquestador | OPEN | mapa del priorizador + gaps vs doctrina |
| PC-09 | **Orden operador (mid-turn 2026-09-19 #4)**: (a) verificar que las 264 estrategias + las otras 4 (~269) están muy bien armadas, documentándose en la red (world/research); (b) 32 operadores que potencian estrategias están implementados — buscar cómo hacer que UNO A UNO, por sus diferentes pipelines, vayan participando y fortalezcan la estrategia en la búsqueda de mayor ganancia; (c) empezar a detectar cuáles estrategias dan más dinero y sobre esas aplicar más y mejores rutas buscando más rentabilidad; (d) clasificación permanente de estrategias más ganadoras por mayor volumen, sin menos. NOTA: censo actual del repo = 264 cartuchos + 31 operadores en math-engine (operador dice 32 — verificar delta). **Doctrina reforzada (mid-turn #6, 2026-09-19)**: los 32 operadores son PALANCA aplicable a CUALQUIERA de los 264 cartuchos — ventaja única sobre el 95% de la competencia (matemática + velocidad + precisión). Hay que IDENTIFICAR y CERTIFICAR qué operador(es) se aplican a cada cartuco durante la detección de arbitrajes; si un cartucho no está apalancado por operadores, eso es un gap a cerrar (sujeto a gate del operador). | orquestador | OPEN | censo 269 + matriz operador→pipeline + leaderboard USD |
| PC-10 | **Orden operador (mid-turn 2026-09-19 #5)**: construir UN GRAFO POR ESTRATEGIA (≥264, uno por cartucho) + UN GRAFO POR OPERADOR (32) — nodos = componentes del pipeline real (detector → cartucho Rhai → operadores math-engine → sizing → gates → persistencia → cards), aristas = flujo de datos verificado en código. Objetivo: certificar que TODAS las fórmulas matemáticas son EXACTAS vs las hojas de los 2 Excel y que el flujo inicio-a-fin (detección → aparición en tarjetas con rentabilidad ordenada de MAYOR a MENOR) funciona por estrategia. Los cartuchos deben ser los que se activen con toda la matemática de detección. Entregable: grafo por estrategia con veredicto PASS/FAIL por nodo/arista. | gang Hermes (296 grafos = misión multi-agente) | OPEN — diseño registrado | 296 grafos + veredictos con evidencia de código |

| PC-11 | **Orden operador (mid-turn #7, 2026-09-19)**: (a) matriz de aplicabilidad estrategia×operador: no todos los 32 aplican a todas — la Estrategia 1 puede estar potenciada por 32, la 2 por 10; hay que GARANTIZAR que los aplicables (10/10, 32/32) efectivamente se apliquen y se ejecuten = máximo potencial por estrategia; (b) los 32 operadores instrumentados en el FRONTEND con toggles habilitar/deshabilitar manuales — el subconjunto habilitado es el universo que se aplica a las estrategias disponibles; (c) las estrategias/cartuchos también se habilitan/deshabilitan manualmente vía toggles (ya existe: canales pause/resume). NOTA: math-engine ya tiene toggle soft de operadores (127.0.0.1, sin auth — gap CB-02 relacionado) y los cartuchos tienen pause/resume; falta superficie frontend unificada + garantía de ejecución. | orquestador | OPEN | UI toggles operadores + matriz aplicabilidad certificada + evidencia ejecución por cartucho |

| PC-12 | **★ OBJETIVO SUPREMO DEL ORQUESTADOR (operador, 2026-09-19 — permanente, jamás olvidar)**: garantizar que TODA la información de la blockchain (Ethereum) se procese y se encuentren ARBITRAJES REALES con GANANCIAS REALES a precios actuales. Reto principal: armar muy bien las CUOTAS y los PARES, y con los precios encontrar las mejores rutas con ganancias positivas. Identificarse DESDE EL INICIO del procesamiento de la data (ingesta blockchain → pipeline completo → cards). Las TARJETAS deben salir con VALORES REALES demostrando detección al 100%. Cuando eso suceda se unirán todas las estrategias para generar la plataforma final: arbitrar EN TIEMPO REAL CON DINERO REAL generando ganancias reales. Equipo: agentes PhD especialistas (gang Hermes). Tatuado en el pecho del orquestador y de todos los agentes — no se descansa hasta cumplirlo. Gates intactos: §34/live-flip = operador-only; ZERO MOCKS; R8. | orquestador + TODOS los gangs | PERMANENTE | arbitrajes reales > 0 en cards con valores reales, E2E verificado |

## Convenciones

**Doctrina de alcance (orden del operador 2026-09-19)**: "Que todos los busquen. Sin
distinción alguna. Que encuentre arbitrajes a lo loco" — discovery = universo completo de
tokens de la red; allowlist = filtro de reflejo únicamente.
- Todo agente LEE este board y lo ACTUALIZA al terminar su WO. R8: cifra vs cifra.
- Los workbooks del operador son READ-ONLY (no modificar los XLSX).
