# ArbitrageX v2 — Cobertura verificable de alta topología

Fecha: 11 de septiembre de 2026. Revisión estática del repositorio aportado en el ZIP. Este informe distingue código, catálogo, conexión con el runtime y ejecución comprobada. Los cambios de grafo y NSGA-II realizados en esta entrega se validan por separado; sus resultados no se presuponen aquí.

El repositorio contiene **264 IDs únicos y 264 cartuchos Rhai**, con las cantidades por familia solicitadas. El registro original de `math-engine` contiene **31 operadores** y sus identidades difieren de varios números del nuevo prompt. El catálogo no prueba 264 estrategias ejecutables: su tabla de despacho declara **79 ROUTE_READY, 174 NEEDS_ROUTE_DATA, 8 OBSERVE_ONLY y 3 NO_COMPATIBLE_ROUTE**. Tampoco se han acreditado las metas de ingresos o latencia mediante esta revisión estática.

El archivo complementario `ALTA_TOPOLOGIA_COBERTURA.json` conserva los 264 IDs, detectores, superficies, clases de ejecución, cartuchos y estados; incluye la traducción de los 32 operadores del prompt sin renumerar el catálogo establecido.

## 1. Estrategias: cantidades y significado del estado

Fuentes contrastadas: `artifacts/strategy_registry.json`, `backend/searcher-rs/src/strategy_dispatch_status.rs`, su fixture JSON y los cartuchos en `backend/searcher-rs/cartridges/strategies/`.

| Familia | Total | ROUTE_READY | NEEDS_ROUTE_DATA | OBSERVE_ONLY | NO_COMPATIBLE_ROUTE | Módulo canónico |
|---|---:|---:|---:|---:|---:|---|
| MEV-01 · Spot DEX | 36 | 36 | 0 | 0 | 0 | `route_graph_engine` |
| MEV-02 · Curvas AMM | 17 | 16 | 0 | 0 | 1 | `amm_curve_engine` |
| MEV-03 · Eventos de estado | 31 | 27 | 0 | 2 | 2 | `state_event_engine` |
| MEV-04 · Paridad y redención | 31 | 0 | 30 | 1 | 0 | `parity_redemption_engine` |
| MEV-05 · CEX–DEX | 14 | 0 | 14 | 0 | 0 | `cex_external_engine` |
| MEV-06 · Cross-chain | 30 | 0 | 30 | 0 | 0 | `cross_domain_engine` |
| MEV-07 · Derivados | 30 | 0 | 30 | 0 | 0 | `derivatives_engine` |
| MEV-08 · Crédito y liquidaciones | 25 | 0 | 25 | 0 | 0 | `credit_liquidation_engine` |
| MEV-09 · Intents | 20 | 0 | 18 | 2 | 0 | `intents_solver_engine` |
| MEV-10 · NFT | 18 | 0 | 18 | 0 | 0 | `nft_engine` |
| MEV-11 · Predicción | 12 | 0 | 9 | 3 | 0 | `prediction_engine` |
| **Total** | **264** | **79** | **174** | **8** | **3** | |

`ROUTE_READY` autoriza formar candidatos en esa tabla; no certifica liquidez, cotización exacta, simulación, contrato ni envío. `NEEDS_ROUTE_DATA` impide formar candidatos en el despacho que consulta esta tabla; `OBSERVE_ONLY` permite observación; `NO_COMPATIBLE_ROUTE` impide expansión. Otras vías heredadas poseen sus propios controles: no debe interpretarse esta tabla como un inventario de transacciones que llegan al firmante.

### Conexión real y siguiente trabajo por familia

| Familia | Evidencia de implementación | Brecha concreta para completar el flujo |
|---|---|---|
| MEV-01 | El cartucho `mev_01_001_dex_dex_arbitrage.rhai` compone CPMM, filtra por suma de logaritmos y optimiza tamaño por sección dorada. Existen `DexEngine`, `TriangularEngine` y despacho canónico de ciclos en `workers/route_scanner_worker.rs`. | Sus 36 IDs incluyen cuatro `R_ORDERBOOK`, dos `R_SPLIT`, dos `R_BASKET_NAV` y un `R_COW`; no todos equivalen al mismo ciclo CPMM. Verificar binding, cotización, calldata y procedencia por ID. El `build_payload` del cartucho de ejemplo declara contrato cero y calldata vacía; el ensamblado debe hacerlo el ejecutor real. |
| MEV-02 | Hay 17 cartuchos y detectores especializados declarados para CPMM, StableSwap, CLAMM, weighted, LB, PMM, TWAMM, batch y otros. | Probar la matemática exacta y estado exigido por cada adaptador. La existencia del archivo y `ROUTE_READY` no validan esos protocolos; no reutilizar la fórmula CPMM para ticks CLAMM, libros ni invariantes diferentes. |
| MEV-03 | Cartuchos con ruta y variantes `lp_event`, `oracle_round`, subastas, funding, rebase y redención. | Incorporar snapshots/eventos específicos, frescura y causalidad. El constructor general de contexto de `cartridge_boot.rs` no satisface por sí solo todos estos campos. Algunas rutas se etiquetan de forma gruesa como `TriangularArb`; preservar el ID y probar semántica particular. |
| MEV-04 | Fórmulas y cartuchos que consultan NAV, rates, PT/YT, queues, wrappers y otros estados. | Fuentes ejecutables de mint/redeem, conversión de unidades, tiempos y contratos específicos. Treinta IDs siguen en `NEEDS_ROUTE_DATA`. |
| MEV-05 | Cartuchos con `cex`/`cex_venues`; existe un worker independiente `workers/cex_dex_worker.rs`. | La categoría canónica `cex_external_engine` devuelve `cartridge_unmapped_strategy_label` en `category_to_strategy_label`. Conectar profundidad firme, inventario, fills y cobertura por ID; un worker separado no acredita los 14 cartuchos. |
| MEV-06 | Los 30 cartuchos consumen `bridge_state`; `MEV-06-003` declara inventario preposicionado y clase no atómica. Existe `CrossChainBridgeEngine`. | `scanner.rs` configura `cross_chain_engine: None`. Completar mapeo de activos por dominio, inventarios, ejecución de ambas piernas, hedge, rebalanceo, finalidad y ledger de settlement. |
| MEV-07 | Treinta cartuchos consumen `derivatives`; algunos también una ruta. | Fuentes de mercados y opciones, márgenes y financiación; categoría `derivatives_engine` sin traducción ejecutable en `category_to_strategy_label`. |
| MEV-08 | Siete cartuchos consumen `position`, nueve `lending`, cinco `claim` y cuatro `auction`. Existe `LiquidationEngine` con indexador y una vía de candidatos. | Alimentar posiciones y términos actuales por protocolo, optimizar deuda/colateral, obtener unwind y construir liquidación real. El cartucho `MEV-08-012` documenta que el feed de posición no está conectado; `liquidation_snipe_engine` se inicializa como `None`. |
| MEV-09 | Veinte cartuchos con intent o batch; 18 candidatos potenciales y dos observacionales. | Firmas/órdenes válidas, profundidad firme, reglas del solver, settlement y mapeo de categoría; `intents_solver_engine` no tiene traducción ejecutable en el puente canónico. |
| MEV-10 | Dieciocho cartuchos con `nft`, cotizaciones por denominación y comprobaciones de frescura. | Ofertas firmes por token/colección, approvals, inventario, transferencia y marketplace; categoría `nft_engine` sin traducción ejecutable. |
| MEV-11 | Doce cartuchos con `prediction`, incluyendo mercados completos y relaciones lógicas; tres son observacionales. | Outcome tokens y colateral, liquidez firme, reglas de split/merge/redeem y settlement; categoría `prediction_engine` sin traducción ejecutable. |

La función `category_to_strategy_label` en `backend/searcher-rs/src/cartridge_boot.rs` es evidencia directa de los rechazos de categoría. Los comentarios que mencionan un requisito de ciclo cerrado en G03/G04 están desactualizados frente al `match` actual, que mapea esas categorías sin ese guard; por ello este informe se basa en el código ejecutable.

## 2. Los números de los operadores no son intercambiables

Fuente original: `backend/math-engine/src/operators/mod.rs`. Todos los archivos citados en la tabla pertenecen a ese directorio. «Mismo concepto» señala identidad, no equivalencia completa de API, calibración o integración.

| ID | Concepto en el prompt | Identidad establecida en el repositorio | Resultado de la comparación |
|---:|---|---|---|
| 01 | Gradient Descent | SVD | Conflicto; descenso existe en Op20. |
| 02 | SGD | PCA | Conflicto. |
| 03 | Coordinate Ascent | Eigen decomposition | Conflicto. |
| 04 | Bayesian Optimization | Von Neumann entropy | Conflicto; inferencia Bayes no sustituye Bayesian Optimization. |
| 05 | PDMP | PDMP | Mismo concepto. |
| 06 | Markov Chain | Markov Chain | Mismo concepto. |
| 07 | HMM | HMM | Mismo concepto. |
| 08 | Kalman | Kalman | Mismo concepto. |
| 09 | Maximum Likelihood | Lévy | Conflicto; MLE existe en Op12. |
| 10 | Welford | Welford | Mismo concepto. |
| 11 | Bayesian Inference | Bayes | Mismo concepto. |
| 12 | BFS | Maximum Likelihood | Conflicto; búsqueda de rutas vive en `route_discovery`. |
| 13 | Regression | Regression | Mismo concepto. |
| 14 | KL Divergence | KL Divergence | Mismo concepto. |
| 15 | Golden Section | Golden Section | Mismo método; el operador existente optimiza una curva primaria, no toda `Q_R`. |
| 16 | Kelly | Kelly | Mismo concepto; necesita probabilidades y resultados observados. |
| 17 | Pontryagin | Pontryagin | Mismo concepto. |
| 18 | Monte Carlo | Lagrangian | Conflicto; Monte Carlo existe en Op22. |
| 19 | Simplex | Simplex | Mismo concepto. |
| 20 | Gradient Descent con momentum | Gradient Descent | El código ajusta la media de retornos; no incorpora momentum ni `profit_fn` de ruta. |
| 21 | Newton-Raphson | Newton-Raphson | Mismo concepto. |
| 22 | MCMC | Monte Carlo GBM | Conflicto; el GBM no implementa MCMC. |
| 23 | Queueing Theory | Queueing Theory | Mismo concepto. |
| 24 | Nash | Nash | Mismo concepto. |
| 25 | Bundle Reconstruction | Bundle Reconstruction | Evidencia matemática; envío vive en `relays-client`. |
| 26 | Flash Loans | Flash Loan | El operador no sustituye liquidez, adaptador y callback. |
| 27 | Ranking profit/riesgo/latencia | Path Ordering | El existente selecciona mínimo/máximo precio entre venues; falta el ranking de rutas del prompt. |
| 28 | Shapley | JIT Liquidity | Conflicto; Shapley existe en Op29. |
| 29 | Cooperative Games | Shapley | Dominio relacionado, cobertura específica de asignación. |
| 30 | Jump Processes | GNN Encoder | Conflicto; Lévy existe en Op09. |
| 31 | Survival Analysis | DRL Agent | Conflicto; además el DRL está sin entrenar. |
| 32 | NSGA-II | Ausente del registro original 1–31 | Incorporación nueva, con validación propia de esta entrega. |

Detalles que impiden afirmar «32 operadores productivos» solo por registrar módulos:

- `op_31_drl_agent.rs` devuelve siempre cero trayectorias etiquetadas; su salida es `None` con `reason_untrained_policy`. `is_available() == true` significa que el evaluador puede emitir ese estado, no que exista una política entrenada.
- `op_30_gnn_encoder.rs` usa agregación y proyección deterministas; no evidencia pesos aprendidos ni entrenamiento.
- `MarketState.price_matrix` se documenta como **venues × activos**, pero Op20 y Op22 recorren su primera columna como serie temporal. Debe separarse historial temporal y observación simultánea antes de interpretar drift, volatilidad o media de retornos.
- `op_15_golden_section.rs` resta salida e input de un solo pool. Para monedas diferentes, `q(x)-x` requiere conversión a un numerario común; una ruta cerrada correctamente compuesta sí retorna a la unidad inicial.
- `strategies/canonical_strategy.rs` contiene un score heurístico y yield por constantes de familia. No se encontró uso de `CanonicalStrategy` fuera de su definición al buscar en los árboles `math-engine/src` y `searcher-rs/src`; no se toma como prueba del detector runtime ni de beneficio realizable.

## 3. Ejecutor, préstamos y liquidaciones

### Restricciones heredadas observadas

1. `backend/relays-client/src/bundle_builder.rs::build_and_sign` solo acepta `StrategyKind::dex_arb()`; las demás clases devuelven `UnsupportedStrategy`. La mera detección de triangular, liquidation o cross-chain no las hace ejecutables por este constructor.
2. `backend/relays-client/src/live_exec_policy.rs::assert_broadcast_allowed` rechaza `chain_id == 1` **incondicionalmente**, aun si aparece en la allowlist. Es un bloqueo heredado concreto frente al objetivo mainnet, no una restricción introducida por esta auditoría. Cambiarlo requiere integrar la política canónica con las condiciones reales de ejecución, preservando simulación, validación, límites y habilitación explícita del operador.
3. El constructor sí conserva una propiedad útil: transmite el `wrapped_calldata` validado por simulación sin reconstruirlo y resuelve el contrato `FlashLoanExecutor` por cadena. También existen adaptadores Solidity Aave, Balancer, dYdX y UniV3 en `contracts/src/flashloans/`. Su existencia no prueba despliegue, fondos, callback ni round trip correctos en la configuración actual.

### Flash loans

`engines/flashloan_engine.rs` envuelve candidatos existentes. La selección de Balancer usa pertenencia de la cadena a una lista y admite cualquier activo en esas cadenas; no consulta disponibilidad ni límite de préstamo del activo. Esa rama precede a Aave y cubre las mismas cadenas. Las comisiones son constantes y, cuando falta precio, el principal puede inferirse con `gross_profit_usd × 20`; esa relación no garantiza una estimación conservadora de la comisión.

Se requiere capacidad y comisión por proveedor, cadena, activo y bloque, principal exacto, coherencia entre detector y adaptador, límites de préstamo y simulación de devolución. El préstamo flash es liquidez limitada y exigible dentro de una transacción; no es capital infinito.

### Liquidaciones

`engines/liquidation_engine.rs` toma el primer activo de deuda y colateral, usa un cap fijo de USD 250.000, produce una ruta de una pierna y escribe `amount_in_wei = debt_to_repay_usd × 1e18`, expresamente descrito en el código como placeholder. Esto no convierte USD a unidades nativas de un token y no incorpora una ruta de venta del colateral.

El cartucho `MEV-08-012` contiene una composición más detallada de incautación y unwind CPMM, pero espera `pool_data.position` y mantiene defaults para algunos términos. El próximo trabajo debe enlazar cuentas y condiciones reales del protocolo, escoger deuda/colateral, usar precio y decimales propios y simular el circuito completo. No basta con `health_factor < 1` y bonus nominal.

### Cross-chain

`engines/cross_chain_bridge_engine.rs` calcula spread absoluto, asume capturar un 50%, usa un capital fijo inicial y convierte con 18 decimales. Consulta la misma dirección de token en dos dominios y construye una única pierna bridge, aunque la estrategia económica necesita compra y venta. Si el destino cotiza más barato, el valor absoluto no cambia la dirección del bridge. La instancia está ausente del arranque observado (`None`).

Para completar `X_PREPOS` se necesitan inventarios independientes y órdenes ejecutables por dominio; para `X_BRIDGE`, además, transferencia, finalidad y exposición durante el traslado. Debe haber reconciliación de fills, balances, coste de carry/hedge y settlement. Una sola transacción EVM local no demuestra atomicidad entre dominios.

## 4. Correcciones necesarias al código ilustrativo del prompt

| Fragmento | Corrección antes de integrarlo |
|---|---|
| Descenso sobre `profit_fn` | `x -= η∇profit(x)` minimiza profit. Maximizar exige ascenso o minimizar su negativo, con dominio acotado y números finitos. |
| Sección dorada | Los puntos del prompt quedan invertidos. Usar `c = b - τ(b-a)`, `d = a + τ(b-a)`, con `τ = (√5−1)/2`, `c < d`; al maximizar y `f(c) < f(d)`, mover `a = c`. Fijar un máximo de iteraciones. |
| Newton para sizing | Resolver `Π′(x)=0` con segunda derivada y bracket; encontrar una raíz de `Π(x)` localiza break-even, no su máximo. |
| Welford | Varianza muestral solo con `n >= 2`; de otro modo resultado ausente. |
| Kelly y ranking | Validar probabilidades, pérdidas, denominadores, signos y finitud; una división por riesgo/latencia cero y `partial_cmp(...).unwrap()` no son admisibles. |
| Liquidación | `deuda × precio_colateral` mezcla unidades. Valor repagado = cantidad de deuda × precio de deuda; convertir colateral por su precio y bonus, aplicar límites reales y valorar unwind ejecutable. |
| SIMD AVX | Comprobar feature AVX, longitudes y remanente `n % 4`. El bucle del prompt puede leer fuera del slice; proveer ruta escalar. |
| GPU | `DirectedEdge` con `Vec`, punteros y enums Rust no es una representación válida portable al dispositivo. Usar DTO numérico/SoA y medir transferencia; el atributo ilustrativo no constituye kernel compilable. |
| Dirty flags | Un bit con `Relaxed` no publica por sí mismo un snapshot completo de reservas. Definir mecanismo de publicación, versión/bloque y toma coherente de estado. |
| NSGA-II | Exigir entradas finitas, dirección de objetivos explícita, frentes no dominados, crowding, elitismo y presupuesto acotado. Mutar una ruta requiere reparar su factibilidad y volver a cotizar; no convertir cromosomas sin validar en oportunidades. |

La cotización en `f64` puede servir para preselección, pero la construcción y simulación final deben conservar unidades enteras, decimales, redondeos y estado de los protocolos. Los 231 pares y 462 direcciones para 22 tokens son **capacidades combinatorias**; no cuentan pools existentes ni aristas de liquidez del multigrafo.

## 5. Rendimiento y criterio de cierre

Los ocho presupuestos del prompt suman **29 ms**. Esa suma no demuestra p95 <30 ms: faltan colas, adquisición/publicación de snapshots, asignaciones, contención, cargas de datos y variación por densidad del grafo. Debe identificarse exactamente qué tramo se mide como discovery antes de comparar resultados.

La búsqueda de archivos bajo `backend/` y `contracts/` no encontró kernels `.cu`/`.cuh` para la detección o simulación descrita. No se ejecutó infraestructura GPU ni se midieron 1.000 simulaciones/s, 10.000 oportunidades/s o latencia a relay. Una prueba local de grafo no valida throughput de simulación EVM ni tasa de inclusión.

Orden concreto del trabajo restante:

1. Mantener identidades canónicas y un mapeo versionado entre IDs del prompt y catálogo. Integrar Op32 como extensión; comprobar que los 264 IDs, knobs y atribución permanecen intactos.
2. Conectar discovery acotado al pipeline existente, con pools reales, fees/decimales orientados, snapshot coherente, dirty updates y ranking medido. Registrar rutas descartadas y presupuestos agotados.
3. Completar `RoutePlan → simulación → ValidatedPlan → calldata` para cada clase soportada, empezando por los ciclos ya conectados; eliminar placeholders monetarios antes de ampliar al firmante.
4. Conectar fuentes por familia y promover estados solo con pruebas de datos, detector, cotización, simulación, settlement y contabilidad. La promoción debe ser por capacidad demostrada e ID, sin activar en bloque los 174 pendientes.
5. Medir p50/p95/p99, cobertura y truncamiento sobre grafos de tamaños/densidades declarados; medir throughput de simulación por backend y hardware. Registrar win rate y Sharpe con definición, ventana, fills, costes y muestra reales.

Los USD 1.000–10.000/hora constituyen un objetivo comercial del prompt. Ningún catálogo, algoritmo ni benchmark del ZIP demuestra esa rentabilidad.

## 6. Verificación efectuada para este informe

Se ejecutó una comprobación local, sin servicios externos, que verificó: 264 registros y IDs únicos; cantidades por familia iguales a las once cifras del prompt; identidad entre el conjunto de IDs de cartuchos y registro; igualdad de las 264 filas de despacho Rust con su fixture; suma de estados 79/174/8/3. El JSON incluye hashes SHA-256 de las fuentes de conteo y 32 entradas de traducción de operadores.

No se ejecutó build completo, entrenamiento, simulación on-chain, despliegue ni envío de transacciones como parte de esta auditoría. Los resultados de pruebas de los cambios de implementación deben consultarse en el informe de cierre de la entrega.
