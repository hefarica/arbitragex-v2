2026
S T A N D A R D  O P E R A T I N G  P R O C E D U R E
ArbitrageX
EVM Arbitrage
Toolkit
Sistema completo de arbitraje atomico multi-cadena EVM con ventaja
competitiva verificada. Arquitectura C-S-E basada en Alloy (Paradigm),
estrategias de alta frecuencia, deteccion de arbitraje triangular, CEX-DEX,
liquidaciones y construccion automatica de rutas de profit en tiempo real.
Stack: Rust + Alloy 0.9 + revm 19.0
Patron: Paradigm C-S-E (Compose-Simulate-Execute)
Target: Ethereum, BSC, Arbitrum, Base, Optimism
FLASH LOANS
MEV
ZERO-COPY
ATOMIC
SUB-100MS
v2.0 // classified
A R B I T R A G E X  S Y S T E M S  / /  2 0 2 6


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 1
TABLA DE CONTENIDOS
0
Placeholder for table of contents


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 2
1. RESUMEN EJECUTIVO
El arbitraje de Valor Máximo Extraíble (MEV) en la Ethereum Virtual Machine (EVM) representa una de las
oportunidades de beneficio más significativas del ecosistema DeFi. Sin embargo, la estadística es
contundente: el 95% de los competidores pierde dinero de forma sistemática. ¿Por qué? La respuesta se
encuentra en tres factores críticos: latencia inadecuada, falta de herramientas de simulación precisas, y una
arquitectura que no escala horizontalmente.
La mayoría de los buscadores (searchers) construyen sus sistemas sobre ethers-rs, una biblioteca que,
aunque funcional, introduce copias innecesarias en la decodificación de datos y carece de una integración
nativa con el motor de simulación revm. ArbitrageX resuelve este problema fundamental utilizando Alloy
v0.9 de Paradigm, que ofrece decodificación zero-copy, serialización optimizada y una API unificada para
providers, transports y tipos Solidity.
La ventaja competitiva de ArbitrageX se fundamenta en cuatro pilares: (1) ejecución atómica de
transacciones complejas a través de bundles y flash loans que eliminan la necesidad de capital inicial; (2)
escaneo sub-milisegundo del mempool y del estado on-chain mediante suscripciones WebSocket con Alloy;
(3) simulación determinista previa al envío usando revm 19.0 integrado con alloy-provider; y (4) asimetría de
información a través del seguimiento en tiempo real de precios en múltiples DEXes, CEXes y pools de
liquidez.
Nuestro patrón arquitectónico C-S-E (Compose-Simulate-Execute) garantiza que cada oportunidad se
componga como un grafo de rutas, se simule localmente con estado real del blockchain, y se ejecute
atómicamente solo si el beneficio neto supera los umbrales configurados. Este enfoque elimina las
ejecuciones fallidas y reduce los costos de gas a una fracción de lo que incurren los competidores.
El presente documento de Procedimiento Operativo Estándar (SOP) detalla cada aspecto del sistema: desde
la arquitectura de software hasta la selección de estrategias, la detección de estafas, y la gestión de riesgos.
Está diseñado para el ciclo 2026 e incorpora las últimas innovaciones en MEV-Boost, JIT liquidity, cross-chain
arbitrage y micro-beneficios de alta frecuencia. Toda la implementación de referencia está en Rust,
utilizando Alloy v0.9 como biblioteca principal de interacción con la EVM.
2. ARQUITECTURA DEL SISTEMA
2.1 Patrón C-S-E (Compose-Simulate-Execute)
La arquitectura de ArbitrageX implementa el patrón Compose-Simulate-Execute (C-S-E) propuesto por
Paradigm como modelo estándar para la construcción de buscadores MEV. Este patrón separa las
responsabilidades en tres fases distintas que se ejecutan de manera secuencial para cada oportunidad
detectada:
•
Compose (Componer): Se construye un grafo de rutas de arbitraje a partir de los pools de liquidez
disponibles. Cada nodo del grafo representa un token y cada arista representa un pool con su tasa de


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 3
cambio y costo de gas. El sistema utiliza el algoritmo de Bellman-Ford modificado para detectar ciclos de
peso negativo, que corresponden a oportunidades de arbitraje.
•
Simulate (Simular): Cada ruta candidata se simula localmente utilizando revm 19.0 con el estado real del
blockchain obtenido a través de alloy-provider. La simulación determinista incluye el cálculo preciso de
fees de LP, slippage, impacto en el precio y costos de gas. Solo las rutas que muestran beneficio neto
positivo avanzan a la siguiente fase.
•
Execute (Ejecutar): Las rutas verificadas se empaquetan en un bundle atómico que se envía a Flashbots
Protect o MEV-Boost relay. La atomicidad garantiza que o todas las transacciones del arbitraje se
ejecutan, o ninguna lo hace, eliminando el riesgo de ejecución parcial.
2.2 Estructura del Workspace Cargo
El workspace de ArbitrageX se organiza en cuatro crates principales, cada uno con una responsabilidad bien
definida. A continuación se muestra el archivo Cargo.toml raíz con todas las dependencias de Alloy y revm:
[workspace]
resolver = "2"
members = [
    "crates/searcher-rs",
    "crates/sim-ctl",
    "crates/relays-client",
    "crates/shared-rs",
]
[workspace.dependencies]
alloy = { version = "0.9", features = ["full"] }
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
alloy-provider = { version = "0.9", features = ["ws"] }
alloy-rpc-types = "0.9"
alloy-transport-ws = "0.9"
alloy-network = "0.9"
revm = "19.0"
revm-primitives = "9.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
eyre = "0.6"
tracing = "0.1"
dashmap = "6"
2.3 Loop Principal del Searcher
El searcher es el componente central que se suscribe a transacciones pendientes mediante WebSocket,
decodifica swaps relevantes, y ejecuta el pipeline C-S-E. El siguiente código muestra la implementación de
referencia usando alloy-provider con soporte WebSocket:
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc_types::{BlockNumberOrTag, Filter};
use eyre::Result;
const MIN_PROFIT: u128 = 50_000_000_000u128; // 50 GWEI minimum
async fn run_searcher(wss_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ws = WsConnect::new(wss_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    // Subscribe to pending transactions via Alloy's unified transport


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 4
    let mut sub = provider.subscribe_pending_transactions().await?;
    tracing::info!("Searcher iniciado, escuchando transacciones pendientes...");
    while let Some(tx_hash) = sub.next().await {
        let tx = provider.get_transaction_by_hash(tx_hash).await?;
        // Zero-copy decode via alloy-primitives
        if let Some(decoded) = decode_swap_tx(&tx) {
            let profit = simulate_arbitrage(&decoded).await?;
            if profit > MIN_PROFIT {
                submit_bundle(vec![tx.into()], profit).await?;
            }
        }
    }
    Ok(())
}
2.4 Responsabilidades por Crate
La siguiente tabla detalla la función de cada componente del workspace:
Crate
Responsabilidad
Dependencias Clave
searcher-rs
Loop principal: suscripción a mempool, detección de
swaps, orquestación C-S-E, envío de bundles
alloy-provider, alloy-transport-ws, tokio,
dashmap
sim-ctl
Simulación determinista con revm: estado on-chain,
ejecución de transacciones, cálculo de profit neto
revm, alloy-primitives, alloy-sol-types
relays-client
Comunicación con Flashbots/MEV-Boost:
construcción de bundles, envío, monitoreo de
inclusión
alloy-provider, reqwest, serde
shared-rs
Tipos compartidos, configuración, utilidades de
decodificación, métricas
alloy-primitives, alloy-sol-types, serde, tracing
3. MATRIZ DE ESTRATEGIAS
ArbitrageX implementa un motor de estrategias modulares que permite combinar múltiples enfoques de
extracción de valor simultáneamente. Cada estrategia tiene un perfil de riesgo-beneficio distinto, y la
selección depende de las condiciones de mercado, la disponibilidad de capital, y la latencia del sistema. La
siguiente tabla comparativa resume las diez estrategias principales soportadas por la plataforma:
Estrategia
Riesgo
Profit %
Velocidad
Capital
Dificultad
Ventaja
DEX Arbitraje Triangular
Muy Bajo
0.1-2%
<100ms
0 (Flash Loan)
Media
Alta
Cross-DEX Price Diff
Bajo
0.05-1.5
%
<200ms
0 (Flash Loan)
Baja
Muy Alta
Sandwich Attack
Medio
0.5-5%
<50ms
Variable
Alta
Media
Liquidation MEV
Bajo
2-15%
<500ms
Variable
Media
Alta


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 5
Estrategia
Riesgo
Profit %
Velocidad
Capital
Dificultad
Ventaja
JIT Liquidity
Muy Bajo
0.3-3%
<150ms
Bajo
Alta
Muy Alta
Flashbots Bundle
Muy Bajo
Variable
<100ms
0 (Flash Loan)
Media
Alta
CEX-DEX Arbitraje
Bajo
0.1-3%
<300ms
Medio
Media
Extrema
Pendle/Temporal AMM
Medio
1-10%
<1s
Medio
Alta
Extrema
Cross-Chain Bridge Arb
Medio
0.2-5%
1-30s
Medio
Alta
Muy Alta
MEV-Boost Block Build
Alto
Variable
<12s
Alto
Muy Alta
Extrema
3.1 Análisis de Ventaja Competitiva
Las estrategias con ventaja competitiva "Extrema" (CEX-DEX Arbitraje, Pendle/Temporal AMM, Cross-Chain
Bridge Arb, y MEV-Boost Block Building) representan las oportunidades donde el 99% de los competidores
no puede operar eficazmente. El CEX-DEX arbitraje, por ejemplo, requiere integración simultánea con APIs
de exchanges centralizados y nodes blockchain, lo que crea una barrera técnica que elimina a la mayoría de
los buscadores.
La estrategia de CEX-DEX Arbitraje es particularmente poderosa porque la asimetría de información entre
los order books centralizados y los AMMs on-chain es persistente y no puede ser eliminada por la
competencia. Mientras más participantes intentan cerrar el spread, más liquidez fluye en ambas direcciones,
pero la latencia inherente a la comunicación cross-system garantiza que los spreads reaparezcan
constantemente.
El arbitraje cross-chain a través de bridges representa otra ventaja extrema debido a la fragmentación
natural de la liquidez entre L1s y L2s. Los precios de tokens en Ethereum, Arbitrum, Base y BSC
frecuentemente divergen significativamente, y los bridges introduce retrasos adicionales que amplían las
ventanas de oportunidad.
Para el operador promedio, recomendamos comenzar con DEX Triangular Arbitrage y Liquidation MEV, que
ofrecen el mejor balance entre riesgo y complejidad. Una vez dominadas, las estrategias de ventaja extrema
generan el "1000% de ventaja injusta" que separa a los profesionales de los aficionados.
4. ESTRATEGIA 1: ARBITRAJE TRIANGULAR DEX
4.1 Concepto y Modelo Matemático
El arbitraje triangular DEX es la forma más pura y fundamental de extracción de valor en DeFi. Consiste en
ejecutar un ciclo de tres swaps a través de tres tokens distintos en diferentes pools de liquidez, de forma que
el monto final sea mayor al monto inicial. El ciclo clásico es: WETH → USDC → UNI → WETH.


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 6
El modelo matemático es directo. Sea f₁, f₂, f₃ las fees de cada pool (típicamente 0.3% para Uniswap V2 o
variables para V3). El beneficio se calcula como:
profit = amount × (1 - f₁) × (1 - f₂) × (1 - f₃) - amount
Para que el arbitraje sea rentable, el producto de las tasas de cambio netas debe ser mayor que 1. Esto
ocurre cuando existe una discrepancia de precios entre los pools que supera la suma acumulada de las tres
fees. En condiciones de alta volatilidad, estas discrepancias aparecen cientos de veces por minuto.
4.2 Condiciones Óptimas
•
Alta volatilidad: Los eventos de noticias, listings de tokens, y grandes movimientos de precio generan
spreads temporales entre pools.
•
Baja liquidez relativa: Pools con TVL inferior a $1M son más susceptibles a desequilibrios de precio que
se pueden explotar.
•
Listings frescos: Los tokens recién listados en DEXes tienen pools desequilibrados que ofrecen
oportunidades de arbitraje triangular significativas.
•
Criterios de selección de tokens: Alto volumen de trading, liquidez profunda en al menos dos pools,
pares correlacionados (ej: WETH/USDC, WETH/WBTC, WBTC/USDC).
4.3 Implementación con Alloy
El siguiente código muestra cómo consultar precios on-chain utilizando el contrato Quoter de Uniswap V3 a
través de alloy-sol-types para la decodificación zero-copy de las llamadas:
use alloy::primitives::{address, U256};
use alloy::sol_types::SolCall;
use alloy::providers::Provider;
// Uniswap V3 Quoter interface - auto-generado con alloy-sol-types
#[derive(SolCall)]
#[sol(name = "Quoter")]
interface IQuoter {
    function quoteExactInputSingle(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint160 sqrtPriceLimitX96
    ) external returns (uint256 amountOut);
}
async fn check_triangular_arb(
    provider: &impl Provider,
    token_a: Address, token_b: Address, token_c: Address,
    amount: U256,
) -> Option<U256> {
    // Step 1: A -> B
    let out_ab = quoter_quote(provider, token_a, token_b, amount).await?;
    // Step 2: B -> C
    let out_bc = quoter_quote(provider, token_b, token_c, out_ab).await?;
    // Step 3: C -> A (close the triangle)
    let out_ca = quoter_quote(provider, token_c, token_a, out_bc).await?;
    if out_ca > amount {
        return Some(out_ca - amount);
    }
    None


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 7
}
async fn quoter_quote(
    provider: &impl Provider,
    token_in: Address,
    token_out: Address,
    amount_in: U256,
) -> Option<U256> {
    let quoter = address!("b27308f9F90D607463bb33eA1BeBb41C27CE5AB6");
    let call = IQuoter::quoteExactInputSingleCall {
        tokenIn: token_in,
        tokenOut: token_out,
        fee: 3000, // 0.3% fee tier
        amountIn: amount_in,
        sqrtPriceLimitX96: U256::ZERO,
    };
    let result = provider.call(&call).into_transaction_request()
        .to(quoter).call().await.ok()?;
    Some(result._0)
}
4.4 Selección de DEXes
ArbitrageX monitorea los siguientes DEXes principales por cadena, priorizando aquellos con mayor liquidez y
menor latencia de respuesta:
DEX
Cadenas
Fee Típico
TVL Aprox.
Uniswap V3/V4
ETH, ARB, OP, BASE, MATIC
0.01-1%
$6.5B
SushiSwap
ETH, BSC, ARB, MATIC
0.3%
$1.2B
Curve Finance
ETH, ARB, MATIC
0.04%
$3.8B
Balancer V2
ETH, ARB, MATIC
0.01-1%
$2.1B
1inch (aggregator)
Multi-chain
Variable
Aggregator
PancakeSwap
BSC, ETH, ARB
0.01-0.25%
$2.5B
TraderJoe
ARB, AVAX
0.3%
$0.8B
Raydium
SOL (EVM bridge)
0.25%
$1.5B
4.5 Mitigación de Riesgos
•
Guardas de slippage: Establecer un máximo de desviación del 0.5% entre el precio esperado y el
ejecutado.
•
Enforcement de deadline: Cada swap incluye un deadline de 30 segundos para evitar ejecuciones
retrasadas.
•
Umbral de costo de gas: Solo ejecutar si el beneficio neto es al menos 3x el costo de gas estimado.
•
Simulación previa: Toda transacción se simula con revm antes del envío para verificar el resultado
exacto.


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 8
5. ESTRATEGIA 2: CEX-DEX ARBITRAJE
5.1 Por Qué Es la Ventaja Competitiva Máxima
El arbitraje CEX-DEX es, sin lugar a dudas, la estrategia con la ventaja competitiva más alta y más
persistente en todo el ecosistema cripto. La razón fundamental es que opera en la intersección de dos
mundos con velocidades, estructuras y mecánicas de formación de precios completamente diferentes.
En un CEX (como Binance, OKX o Bybit), los precios se forman a través de un order book centralizado con
latencias de microsegundos. En un DEX, los precios son función de la razón de reservas en los pools (x × y = k
para Uniswap V2), y la ejecución ocurre on-chain con latencias de 100ms a varios segundos. Esta asimetría
estructural es permanente y no puede ser eliminada por la competencia.
5.2 Mecánica de Detección
La velocidad es el factor determinante. El sistema debe detectar la discrepancia de precio y ejecutar la
operación en menos de 300ms. Esto se logra manteniendo una conexión WebSocket permanente con los
feeds de precio del CEX y comparando en tiempo real con los precios on-chain obtenidos a través de Alloy. El
spread se calcula como:
spread = |precio_cex - precio_dex| / min(precio_cex, precio_dex)
Cuando el spread supera el umbral mínimo (configurable, típicamente 0.15%), el sistema determina la
dirección del trade: comprar en DEX y vender en CEX si el precio CEX es mayor, o viceversa.
5.3 Implementación con Alloy + Binance WS
use alloy::providers::Provider;
use tokio::sync::mpsc;
use std::time::Duration;
const MIN_SPREAD_THRESHOLD: f64 = 0.0015; // 0.15%
#[derive(Debug, Clone)]
enum TradeDirection {
    BuyDexSellCex,
    BuyCexSellDex,
}
async fn cex_dex_arb_loop(
    binance_ws: &str,
    provider: &impl alloy::providers::Provider,
) {
    let (tx, mut rx) = mpsc::channel::<(String, f64)>(1024);
    // Spawn Binance price feed listener
    spawn_binance_price_feed(binance_ws, tx).await;
    tracing::info!("CEX-DEX arb loop iniciado");
    while let Some((symbol, cex_price)) = rx.recv().await {
        // Query on-chain price via Alloy
        let dex_price = get_on_chain_price(provider, &symbol).await;


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 9
        let spread = (cex_price - dex_price).abs()
            / dex_price.min(cex_price);
        if spread > MIN_SPREAD_THRESHOLD {
            let direction = if cex_price > dex_price {
                TradeDirection::BuyDexSellCex
            } else {
                TradeDirection::BuyCexSellDex
            };
            tracing::info!(
                "Spread detectado: {} -> {:.4}%",
                symbol, spread * 100.0
            );
            execute_cex_dex_trade(direction, spread).await;
        }
    }
}
async fn get_on_chain_price(
    provider: &impl alloy::providers::Provider,
    symbol: &str,
) -> f64 {
    // Route to appropriate pool and query price
    let pool = match symbol {
        "ETHUSDT" => address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
        // ... more mappings
        _ => return 0.0,
    };
    // Query via Alloy - returns raw price, convert to f64
    let (reserve0, reserve1, _) = get_reserves(provider, pool).await;
    reserve1.to_f64_lossy() / reserve0.to_f64_lossy()
}
5.4 Por Qué el 95% Fracasa
La mayoría de los competidores fracasa en CEX-DEX arbitraje por tres razones fundamentales:
•
Latencia excesiva: Usar REST APIs en lugar de WebSockets introduce latencias de 50-200ms que
eliminan cualquier oportunidad. El sistema debe mantener conexiones WS persistentes y procesar datos
en un solo thread dedicado.
•
Desajuste de profundidad del order book: Comprar $1M en un CEX a precio de mercado puede deslizar
el precio significativamente. El sistema debe calcular el impacto real usando la profundidad del order
book, no solo el precio top-of-book.
•
Timing de retiros y depósitos: Mover fondos entre CEXes y wallets on-chain toma minutos. La solución
es mantener capital pre-posicionado en ambos lados y operar exclusivamente con el capital disponible.
6. ESTRATEGIA 3: LIQUIDACIONES DE PRÉSTAMOS
6.1 Mecánica de Liquidación en DeFi
Los protocolos de préstamos descentralizados como Aave, Compound y MakerDAO permiten a los usuarios
tomar préstamos respaldados por colateral. Cuando el valor del colateral cae por debajo de un umbral, la


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 10
posición se vuelve elegible para liquidación. El liquidador puede comprar el colateral con descuento
(típicamente 5-15%) y cerrar la deuda del prestatario, obteniendo un beneficio inmediato.
El Health Factor (Factor de Salud) es la métrica clave: valores por debajo de 1.0 indican que la posición es
elegible para liquidación. En Aave, el health factor se calcula como la suma ponderada del valor del colateral
dividida por la suma ponderada de la deuda. Los factores de peso dependen del tipo de activo y su
volatilidad.
6.2 Monitoreo de Health Factor con Alloy
use alloy::primitives::{address, U256};
use alloy::providers::Provider;
use alloy::sol_types::SolCall;
use std::time::Duration;
const LIQUIDATION_THRESHOLD: f64 = 1.05; // Margin of safety
const MIN_LIQUIDATION_PROFIT: u128 = 500_000_000_000u128; // 500 USDC
#[derive(SolCall)]
#[sol(name = "LendingPool")]
interface ILendingPool {
    function getUserAccountData(address user)
        external returns (
            uint256 totalCollateralBase,
            uint256 totalDebtBase,
            uint256 availableBorrowsBase,
            uint256 currentLiquidationThreshold,
            uint256 ltv,
            uint256 healthFactor
        );
}
async fn monitor_liquidations(provider: &impl Provider) {
    let aave_lending_pool =
        address!("7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9");
    loop {
        let user_accounts = get_at_risk_users(
            provider, aave_lending_pool
        ).await;
        for user in user_accounts {
            let health = compute_health_factor(
                provider, aave_lending_pool, user
            ).await;
            if health < LIQUIDATION_THRESHOLD {
                tracing::warn!(
                    "Posición en riesgo: {:?} HF={:.4}",
                    user, health
                );
                let profit = simulate_liquidation(
                    provider, aave_lending_pool, user
                ).await;
                if profit > MIN_LIQUIDATION_PROFIT {
                    execute_liquidation(
                        provider, aave_lending_pool, user, profit
                    ).await;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 11
6.3 Riesgos y Competencia
Las liquidaciones presentan riesgos únicos que deben gestionarse cuidadosamente. El riesgo principal es la
competencia con otros liquidadores: cuando una posición se vuelve elegible, múltiples bots compiten por ser
el primero en ejecutar la liquidación, lo que puede resultar en guerras de gas que eliminan el beneficio.
ArbitrageX mitiga esto mediante Flashbots bundles que garantizan la ejecución sin guerras de gas, y al
mantener múltiples RPC endpoints para redundancia.
•
Colateral subacuático: Si el precio del colateral cae demasiado rápido, la liquidación puede resultar en
pérdida.
•
Gueras de gas: Usar private mempools (Flashbots) para evitar subastas de prioridad.
•
Fallas del contrato: Verificar siempre que los contratos de lending pool no hayan sido actualizados
recientemente.
7. ESTRATEGIA 4: JIT (JUST-IN-TIME) LIQUIDITY
7.1 Concepto: La Joya Oculta del MEV
La liquidez Just-In-Time (JIT) es posiblemente la estrategia más sofisticada y menos comprendida del
ecosistema MEV. Consiste en proporcionar liquidez a un pool exactamente en el momento en que un swap
pendiente la necesita, capturando el beneficio de la tasa de cambio y luego retirando la liquidez
inmediatamente después.
El mecanismo funciona así: cuando un swap grande aparece en el mempool, el searcher analiza el impacto
que tendrá en el precio del pool. Si el swap moverá el precio significativamente, el searcher puede
proporcionar liquidez en el rango de precio afectado justo antes de que el swap se ejecute, capturando las
fees del LP. Luego, en la misma transacción, retira la liquidez. Todo esto ocurre atómicamente dentro de un
Flashbots bundle.
7.2 Ventajas Competitivas
Muy pocos competidores implementan JIT liquidity eficazmente. La razón es que requiere una comprensión
profunda de cómo funcionan los concentradores de liquidez (Uniswap V3/V4), la capacidad de calcular la
posición óptima de liquidez en milisegundos, y una ejecución bundle perfectamente sincronizada. Los
beneficios por operación pueden ser del 0.3% al 3%, con riesgo virtualmente nulo ya que la liquidez se
proporciona y retira en la misma transacción.
Las condiciones óptimas para JIT liquidity son: swaps grandes ($100K+) en pools con liquidez concentrada,
alta volatilidad del precio del token subyacente, y un mempool con suficiente tiempo de propagación para
detectar y reaccionar a los swaps pendientes. Esta estrategia brilla especialmente en pares como
WETH/USDC en Uniswap V3 con fee tier de 0.05%, donde el volumen diario supera los $500M.


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 12
Para implementar JIT, el sistema debe analizar cada transacción pendiente, reconstruir la posición de
liquidez después del swap, calcular el rango óptimo para la posición JIT, simular el resultado con revm, y
enviar un bundle que incluya: (1) mint de la posición de liquidez, (2) el swap original, y (3) burn de la
posición. Si alguno de estos pasos falla en simulación, el bundle se descarta completamente.
8. ESTRATEGIA 5: SANDWICH ATTACKS DEFENSIVOS
8.1 Consideraciones Éticas
ArbitrageX adopta una postura estrictamente defensiva respecto a los sandwich attacks. Un sandwich attack
ocurre cuando un buscador front-runnea una transacción de swap (comprando antes para inflar el precio) y
luego back-runnea la misma transacción (vendiendo después del precio inflado), capturando el spread a
expensas del usuario original.
Si bien esta técnica es técnicamente posible y rentable (0.5-5% por operación), ArbitrageX NO la implementa
de forma ofensiva. En su lugar, utilizamos nuestro conocimiento de los mecanismos de sandwich para
proteger nuestras propias operaciones y las de nuestros usuarios.
8.2 Protección Anti-MEV para Tus Trades
Para proteger las operaciones de ArbitrageX contra sandwich attacks, implementamos las siguientes
medidas defensivas:
•
Flashbots Protect RPC: Todas nuestras transacciones se envían a través del RPC privado de Flashbots,
que las excluye del mempool público y las hace invisibles para los sandwich bots.
•
Slippage mínimo: Configurar el slippage al mínimo posible (0.1% o menos) para que cualquier intento de
front-running haga la transacción revert.
•
Atomic execution: Los bundles de transacciones se ejecutan de forma atómica, lo que significa que si un
sandwich bot intenta insertarse entre nuestras operaciones, todo el bundle falla y ninguno se ejecuta.
•
Private mempool alternatives: Además de Flashbots, utilizamos MEV Blocker y Titan Builder como
alternativas para el envío de transacciones privadas.
9. ESTRATEGIA 6: FLASHBOTS Y BUNDLE
CONSTRUCTION
9.1 Flashbots Protect y RPC Privado


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 13
Flashbots es la infraestructura estándar para la ejecución privada de transacciones y bundles en Ethereum. A
través de su RPC protegido (https://rpc.flashbots.net), los usuarios pueden enviar transacciones que nunca
aparecen en el mempool público, lo que elimina la posibilidad de front-running y sandwich attacks.
Para los buscadores MEV, Flashbots ofrece la capacidad de enviar bundles: secuencias de transacciones que
se ejecutan de forma atómica dentro de un bloque. Si alguna transacción del bundle falla, ninguna se
ejecuta. Esta atomicidad es fundamental para el arbitraje: permite combinar flash loans, swaps, y
liquidaciones en una única operación sin riesgo de ejecución parcial.
9.2 Construcción de Bundles con Alloy
use alloy::providers::{ProviderBuilder, Http};
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc_types::TransactionRequest;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
struct FlashbotsBundle {
    txs: Vec<BundleTransaction>,
    block_number: u64,
    min_timestamp: u64,
    max_timestamp: u64,
    reverting_tx_hashes: Vec<B256>,
}
#[derive(Debug, Serialize, Deserialize)]
struct BundleTransaction {
    tx: Bytes,
    can_revert: bool,
}
async fn submit_flashbots_bundle(
    bundle_txs: Vec<BundleTransaction>,
    target_block: u64,
) -> Result<B256, Box<dyn std::error::Error>> {
    // Use Flashbots relay via Alloy HTTP provider
    let relay_url = "https://relay.flashbots.net";
    let provider = ProviderBuilder::new()
        .on_http(relay_url.parse()?);
    let bundle = FlashbotsBundle {
        txs: bundle_txs,
        block_number: target_block,
        min_timestamp: 0,
        max_timestamp: 0,
        reverting_tx_hashes: vec![],
    };
    // Send bundle to Flashbots relay
    let bundle_hash = provider
        .raw_request::<_, B256>(
            "eth_sendBundle".into(),
            [bundle]
        )
        .await?;
    tracing::info!(
        "Bundle enviado: hash={:?} block={}",
        bundle_hash, target_block
    );
    Ok(bundle_hash)
}
// Atomic flash-loan arbitrage bundle
async fn build_arb_bundle(
    flash_loan_amount: U256,


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 14
    route: Vec<Address>,
    provider: &impl alloy::providers::Provider,
) -> Vec<BundleTransaction> {
    // 1. Flash loan from Aave/dYdX
    let loan_tx = build_flash_loan_tx(flash_loan_amount).await;
    // 2. Execute swaps along route
    let swap_tx = build_swap_tx(&route, flash_loan_amount).await;
    // 3. Repay flash loan + collect profit
    let repay_tx = build_repay_tx().await;
    vec![
        BundleTransaction { tx: loan_tx, can_revert: false },
        BundleTransaction { tx: swap_tx, can_revert: true },
        BundleTransaction { tx: repay_tx, can_revert: false },
    ]
}
9.3 Monitoreo de Inclusión
Después de enviar un bundle, es crucial monitorear si fue incluido en un bloque. ArbitrageX implementa un
sistema de monitoreo que: (1) verifica cada nuevo bloque para confirmar la inclusión del bundle hash, (2)
rastrea el status de la transacción en caso de revert, (3) ajusta la estrategia de envío si los bundles son
consistentemente ignorados (por ejemplo, incrementando el coinbase payment al builder), y (4) mantiene
estadísticas de tasa de inclusión por builder para optimizar el enrutamiento de bundles.
10. SELECCIÓN DE TOKENES Y POOLS
10.1 Criterios de Selección
La selección adecuada de tokens y pools de liquidez es fundamental para el éxito del arbitraje. No todos los
tokens son adecuados: los tokens de baja liquidez pueden ser honeypots, y los pools con volumen
insuficiente no ofrecen oportunidades rentables. Los criterios principales son:
•
Capitalización de mercado: Mínimo $10M para tokens principales, $1M para oportunidades de alto
riesgo.
•
Volumen diario: Mínimo $1M de volumen 24h para asegurar liquidez suficiente.
•
Profundidad de liquidez: Evaluar el impacto en el precio para swaps de $10K-$100K.
•
Volatilidad: Mayor volatilidad = más oportunidades, pero también más riesgo.
•
Correlación de pares: Buscar pares que operen en múltiples DEXes para maximizar las rutas disponibles.
10.2 Pares Recomendados por Cadena


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 15
Cadena
Pares Principales
TVL Total
Vol. 24h
Ethereum L1
WETH/USDC, WBTC/ETH, USDC/USDT,
WETH/USDT, LINK/ETH
$95B
$4.2B
BSC
WBNB/USDT, CAKE/BNB, BUSD/USDT, ETH/BTC
$5.8B
$1.8B
Arbitrum
WETH/USDC, GMX/ETH, ARB/ETH, RDNT/ETH
$3.2B
$0.9B
Optimism
WETH/USDC, OP/ETH, SNX/ETH, DAI/USDC
$1.5B
$0.5B
Base
WETH/USDC, BASE/ETH, AERO/ETH, USDC/USDB
$2.1B
$0.7B
Polygon
WMATIC/USDC, WETH/USDC, QUICK/ETH,
DAI/USDC
$1.8B
$0.6B
10.3 Métricas de Selección de Pools
Para cada pool candidato, ArbitrageX calcula las siguientes métricas: TVL (valor total bloqueado) para
evaluar la profundidad, Ratio volumen/TVL para identificar pools con alta actividad relativa, Fee tier para
determinar el costo por operación, Spread promedio para cuantificar la volatilidad del precio, y Historial de
exploits para descartar pools con vulnerabilidades conocidas. Los pools con ratio volumen/TVL superior al
30% son los más prometedores, ya que indican alta rotación de capital y frecuentes desequilibrios de precio.
11. DETECCIÓN DE ESTAFAS Y PROTECCIÓN
11.1 Detección de Honeypots
Un honeypot es un token que se puede comprar pero no vender. Es una de las estafas más comunes en DeFi,
y perder capital en un honeypot durante un intento de arbitraje puede ser catastrófico. ArbitrageX
implementa múltiples capas de detección:
•
Verificación de código bytecode: Analizar el bytecode del contrato para identificar patrones
sospechosos como funciones de transferencia modificadas.
•
Simulación de venta: Ejecutar una simulación de venta con revm para verificar que la transacción no
revierte.
•
Análisis de blacklist/mint: Detectar funciones de blacklisting y minteo ilimitado que pueden manipular
el supply.
•
Cálculo de impuesto de transferencia: Estimar el tax de compra/venta simulando transacciones y
comparando montos.
11.2 Indicadores de Rug Pull


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 16
Los rug pulls ocurren cuando los desarrolladores de un token retiran toda la liquidez o transfieren los fondos
a su control. Los indicadores clave son: liquidez no bloqueada (sin timelock), allocation excesiva al equipo
(más del 20% del supply), contratos actualizables (owner con privilegios de upgrade), y holders top-10
concentrando más del 50% del supply.
11.3 Checklist de Análisis de Contratos
Antes de operar con cualquier token nuevo, ArbitrageX ejecuta automáticamente el siguiente checklist de
seguridad:
11.4 Implementación: Verificación de Seguridad con Alloy
use alloy::primitives::Address;
use alloy::providers::Provider;
const MAX_ACCEPTABLE_TAX: f64 = 0.05; // 5% max transfer tax
async fn is_token_safe(
    provider: &impl Provider,
    token: Address,
) -> bool {
    // Step 1: Check contract exists
    let code = provider.get_code_at(token).await.unwrap_or_default();
    if code.is_empty() {
        tracing::warn!("Token {:?} no tiene código", token);
        return false;
    }
    // Step 2: Check for honeypot - can we sell?
    let owner = get_token_owner(provider, token).await;
    if is_blacklisted(provider, token, owner).await {
        tracing::warn!("Token {:?} tiene blacklist activo", token);
        return false;
    }
    // Step 3: Check transfer tax
    let tax = estimate_sell_tax(provider, token).await;
    if tax > MAX_ACCEPTABLE_TAX {
        tracing::warn!(
            "Token {:?} tax de venta {:.2}% excede máximo",
            token, tax * 100.0
        );
        return false;
    }
    // Step 4: Check liquidity lock
    let liq_locked = is_liquidity_locked(provider, token).await;
    if !liq_locked {
        tracing::warn!("Token {:?} liquidez no bloqueada", token);
        return false;
    }
    // Step 5: Check for mint function
    let has_mint = has_unrestricted_mint(provider, token).await;
    if has_mint {
        tracing::warn!("Token {:?} tiene mint sin restricción", token);
        return false;
    }
    tracing::info!("Token {:?} pasó todas las verificaciones", token);
    liq_locked && tax < MAX_ACCEPTABLE_TAX && !has_mint
}


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 17
12. ALGORITMO DE BÚSQUEDA DE LIQUIDEZ
12.1 Agregación Multi-Chain
La búsqueda de liquidez es un componente crítico del pipeline de arbitraje. No basta con conocer un solo
pool; el sistema debe encontrar el mejor precio disponible entre todos los DEXes y pools de todas las
cadenas relevantes. ArbitrageX implementa un motor de agregación de liquidez que monitorea
continuamente los precios y disponibilidades en todos los pools activos.
El algoritmo utiliza una estrategia de split-routing: cuando el monto del arbitraje es grande, el sistema divide
la operación entre múltiples pools para minimizar el impacto en el precio. Por ejemplo, un swap de 100
WETH se podría dividir en 60 WETH en Uniswap V3 (fee 0.05%) y 40 WETH en Curve (fee 0.04%), obteniendo
un precio promedio ponderado mejor que el de cualquier pool individual.
12.2 Implementación del Buscador de Liquidez
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
#[derive(Debug, Clone)]
struct LiquidityRoute {
    dex: String,
    pool: Address,
    amount_in: U256,
    amount_out: U256,
    gas_cost: U256,
    profit: U256,
}
async fn find_best_liquidity(
    provider: &impl Provider,
    token_in: Address,
    token_out: Address,
    amount: U256,
) -> Vec<LiquidityRoute> {
    // Query all known DEX protocols
    let dexes = get_dex_list();
    // Uniswap, SushiSwap, Curve, Balancer,
    // 1inch, PancakeSwap, etc.
    let mut routes = vec![];
    for dex in &dexes {
        if let Some(pool) = find_pool(
            provider, dex, token_in, token_out
        ).await {
            let quote = get_quote(provider, pool, amount).await;
            let gas = estimate_gas(
                provider, pool, amount
            ).await;
            if quote > U256::ZERO {
                routes.push(LiquidityRoute {
                    dex: dex.name.clone(),
                    pool,
                    amount_in: amount,
                    amount_out: quote,
                    gas_cost: gas,
                    profit: quote.saturating_sub(amount),
                });
            }


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 18
        }
    }
    // Sort by profit descending
    routes.sort_by(|a, b| b.profit.cmp(&a.profit));
    routes
}
async fn compute_split_route(
    routes: &[LiquidityRoute],
    total_amount: U256,
) -> Vec<(String, U256)> {
    // Proportional split based on depth
    let total_depth: U256 = routes.iter()
        .map(|r| r.amount_out)
        .fold(U256::ZERO, |acc, x| acc + x);
    routes.iter()
        .map(|r| {
            let split = total_amount * r.amount_out / total_depth;
            (r.dex.clone(), split)
        })
        .collect()
}
13. SISTEMA DE MICRO-BENEFICIOS DE ALTA
FRECUENCIA
13.1 Filosofía: Volume sobre Home Runs
La estrategia de micro-beneficios de alta frecuencia es la que mejores resultados consistentes produce en el
ecosistema MEV. En lugar de buscar operaciones con beneficios extraordinarios (0.5-5%) que ocurren
raramente, el sistema apunta a beneficios pequeños (0.01-0.1%) pero con alta frecuencia: entre 100 y 1,000
operaciones por día.
La matemática es simple pero poderosa. Con un beneficio promedio del 0.05% por operación y 500
operaciones diarias exitosas, el beneficio diario es del 25% del capital rotado. Con capital de $100K rotado a
través de flash loans (capital cero), esto equivale a $25K diarios, o $750K mensuales. El riesgo por operación
es virtualmente cero porque cada operación se simula antes de ejecutarse.
Esta estrategia supera consistentemente a los enfoques de "home run" porque: (1) las oportunidades de
micro-beneficio son mucho más frecuentes; (2) la competencia por operaciones pequeñas es menor (los
bots grandes las ignoran); (3) el efecto compuesto de cientos de operaciones diarias genera rendimientos
exponenciales; y (4) la varianza es mucho menor, lo que permite una planificación financiera predecible.
13.2 Implementación del Scanner de Micro-Arbitrajes
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
const GAS_PRICE_MULTIPLIER: u128 = 3;
const MAX_OPPORTUNITIES_PER_BLOCK: usize = 50;


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 19
#[derive(Debug)]
struct MicroArb {
    route: Vec<Address>,
    expected_profit: U256,
    gas_cost: U256,
    net_profit: U256,
    confidence: f64,
}
async fn scan_micro_arbs(
    provider: &impl Provider,
) -> Vec<MicroArb> {
    let pairs = get_watched_pairs();
    let mut opportunities = vec![];
    for pair in &pairs {
        let routes = compute_all_routes(
            provider, pair
        ).await;
        for route in routes {
            // Simulate the route with revm
            let profit = simulate_route(
                provider, &route
            ).await;
            let gas = estimate_route_gas(
                provider, &route
            ).await;
            // Only include if profit exceeds gas * 3x
            if profit > gas * GAS_PRICE_MULTIPLIER {
                opportunities.push(MicroArb {
                    route: route.tokens.clone(),
                    expected_profit: profit,
                    gas_cost: gas,
                    net_profit: profit - gas,
                    confidence: route.confidence,
                });
            }
        }
    }
    // Sort by net profit, take top 50
    opportunities.sort_by(|a, b| {
        b.net_profit.cmp(&a.net_profit)
    });
    opportunities.truncate(MAX_OPPORTUNITIES_PER_BLOCK);
    opportunities
}
async fn execute_micro_arbs(
    provider: &impl Provider,
    arbs: Vec<MicroArb>,
) {
    let mut total_profit = U256::ZERO;
    for arb in &arbs {
        match execute_single_arb(provider, arb).await {
            Ok(profit) => {
                total_profit += profit;
                tracing::info!(
                    "Micro-arb exitoso: profit={:?} confidence={:.2}",
                    profit, arb.confidence
                );
            }
            Err(e) => {
                tracing::debug!(
                    "Micro-arb fallido: {} (route={:?})",
                    e, arb.route
                );
            }


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 20
        }
    }
    tracing::info!(
        "Batch completado: {} arbs, profit total={:?}",
        arbs.len(), total_profit
    );
}
14. CONSTRUCCIÓN ATÓMICA DE RUTAS DE
ARBITRAJE
14.1 Optimización Basada en Grafos
La construcción de rutas de arbitraje se modela como un problema de teoría de grafos. Cada token es un
nodo, cada pool de liquidez es una arista ponderada por la tasa de cambio (o más precisamente, por el
logaritmo negativo de la tasa). Un ciclo en el grafo con peso total negativo corresponde a una oportunidad
de arbitraje.
ArbitrageX utiliza una variante del algoritmo de Bellman-Ford para detectar ciclos de peso negativo. A
diferencia de Dijkstra, Bellman-Ford puede manejar pesos negativos, lo que lo hace ideal para la detección
de arbitraje. El algoritmo se ejecuta en tiempo O(V × E), donde V es el número de tokens monitoreados y E
es el número de pools activos.
14.2 Implementación del Grafo de Arbitraje
use std::collections::HashMap;
use alloy::primitives::Address;
/// Arbitrage graph where nodes are tokens and edges
/// are pools with exchange rates.
struct ArbitrageGraph {
    nodes: HashMap<Address, Vec<(Address, f64, Address)>>,
    // token -> [(other_token, exchange_rate, pool_addr)]
}
impl ArbitrageGraph {
    fn new() -> Self {
        ArbitrageGraph {
            nodes: HashMap::new(),
        }
    }
    fn add_edge(
        &mut self,
        from: Address,
        to: Address,
        rate: f64,      // e.g. 1.002 for 0.2% gain
        pool: Address,
    ) {
        self.nodes.entry(from).or_default()
            .push((to, rate, pool));
        // Also add reverse edge
        self.nodes.entry(to).or_default()
            .push((from, 1.0 / rate, pool));


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 21
    }
    /// Find negative-weight cycles using Bellman-Ford
    /// variant. A negative cycle = arbitrage opportunity.
    fn find_arbitrage_cycle(
        &self,
        start: Address,
        max_depth: usize,
    ) -> Option<Vec<Address>> {
        let mut dist: HashMap<Address, f64> = HashMap::new();
        let mut pred: HashMap<Address, Address> = HashMap::new();
        // Initialize distances
        for node in self.nodes.keys() {
            dist.insert(*node, f64::INFINITY);
        }
        dist.insert(start, 0.0);
        // Relax edges up to max_depth times
        for _ in 0..max_depth {
            for (u, edges) in &self.nodes {
                for (v, rate, _pool) in edges {
                    // Use -ln(rate) as edge weight
                    let weight = -(rate.ln());
                    if let Some(&d_u) = dist.get(u) {
                        if d_u + weight < *dist.get(v).unwrap_or(&f64::INFINITY) {
                            dist.insert(*v, d_u + weight);
                            pred.insert(*v, *u);
                        }
                    }
                }
            }
        }
        // Check for negative cycle (arbitrage)
        for (u, edges) in &self.nodes {
            for (v, rate, _pool) in edges {
                let weight = -(rate.ln());
                if let (Some(&d_u), Some(&d_v)) = (dist.get(u), dist.get(v)) {
                    if d_u + weight < d_v {
                        // Negative cycle found! Reconstruct.
                        return self.reconstruct_cycle(
                            *v, &pred, start
                        );
                    }
                }
            }
        }
        None
    }
    fn reconstruct_cycle(
        &self,
        node: Address,
        pred: &HashMap<Address, Address>,
        start: Address,
    ) -> Option<Vec<Address>> {
        let mut path = vec![node];
        let mut current = node;
        let max_steps = pred.len();
        for _ in 0..max_steps {
            if let Some(&prev) = pred.get(&current) {
                path.push(prev);
                current = prev;
                if current == start {
                    path.reverse();
                    return Some(path);
                }
            } else {
                break;
            }


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 22
        }
        None
    }
}
14.3 Garantía de Ejecución Atómica
Una vez que el grafo identifica una ruta de arbitraje, esta se traduce en una secuencia de transacciones que
se empaquetan en un bundle atómico. La atomicidad se garantiza a través de Flashbots: si cualquier paso de
la ruta falla (por ejemplo, el pool cambia de precio entre la detección y la ejecución), todo el bundle se
descarta y el beneficio potencial se pierde, pero nunca se incurrirá en pérdidas.
El sistema soporta rutas multi-hop de hasta 5 saltos, aunque las rutas más rentables suelen ser de 3-4 saltos.
Más saltos significan más fees acumuladas y mayor probabilidad de que el mercado se mueva antes de la
ejecución. El límite de profundidad se configura dinámicamente basándose en la volatilidad actual del
mercado: mayor volatilidad permite rutas más profundas ya que los spreads son más amplios.
15. GESTIÓN DE RIESGOS
15.1 Principios Fundamentales
La gestión de riesgos es el pilar que diferencia a un sistema MEV rentable de uno que pierde dinero.
ArbitrageX implementa múltiples capas de protección que operan de forma autónoma y no pueden ser
desactivadas, ni siquiera manualmente:
15.2 Tamaño de Posición
Nunca arriesgar más del 2% del capital total en una sola operación. Para operaciones con flash loans, este
límite se traduce en un límite de 2% del beneficio potencial, ya que el capital es prestado. El sistema calcula
automáticamente el tamaño óptimo de la operación basándose en la profundidad del pool, la volatilidad
reciente, y el ratio beneficio/riesgo histórico de la ruta.
15.3 Protección de Costos de Gas
•
Umbral mínimo de beneficio: Solo ejecutar si el beneficio neto es al menos 3x el costo de gas estimado.
Este multiplicador se ajusta dinámicamente según las condiciones de congestión de la red.
•
Gas price oracle: Mantener un oracle de gas price actualizado y rechazar operaciones cuando el gas
price supera un umbral máximo configurable.
•
Gas estimation precisa: Usar alloy-provider para estimaciones de gas reales antes de cada operación.
15.4 Protección de Slippage


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 23
El slippage máximo permitido es del 0.5% por swap individual. Esto se implementa a nivel de contrato
inteligente con un amountOutMin calculado dinámicamente. Si el precio se mueve más de 0.5% entre la
simulación y la ejecución, la transacción revierte automáticamente.
15.5 Stop-Loss Automático
•
Abort por simulación negativa: Si la simulación con revm muestra pérdida en lugar de beneficio, la
operación se cancela inmediatamente.
•
Límite diario de pérdida: Si las pérdidas acumuladas en un período de 1 hora superan un umbral
(configurable, por defecto 0.5% del capital), el sistema entra en modo de protección y solo ejecuta
operaciones de micro-beneficio.
•
Protección contra reentrancia: Todos los contratos inteligentes propios implementan el patrón
check-effects-interactions y el modificador nonReentrant de OpenZeppelin.
15.6 Private Mempool Usage
Todas las transacciones de ArbitrageX se envían exclusivamente a través de mempools privados: Flashbots
Protect, MEV Blocker, y Titan Builder. Esto elimina completamente el riesgo de front-running por parte de
otros bots MEV y garantiza que las operaciones se ejecuten bajo nuestras condiciones exactas o no se
ejecuten en absoluto.
16. DESPLIEGUE Y OPERACIONES
16.1 Infraestructura Requerida
El despliegue de ArbitrageX requiere una infraestructura dedicada optimizada para latencia mínima. Los
requisitos técnicos son los siguientes:
Componente
Especificación
Propósito
VPS Principal
4 vCPU, 16GB RAM, NVMe SSD
Ejecución del searcher y sim-ctl
VPS Backup
2 vCPU, 8GB RAM
Failover y monitoreo
VPS CEX Feed
2 vCPU, 4GB RAM, cercano a exchange
WebSocket price feeds
RPC Nodes
Dedicados (Alchemy/QuickNode)
Latencia <10ms para llamadas on-chain
Red
Dedicada, baja latencia
Comunicación entre componentes
16.2 Ubicación del VPS


ArbitrageX — SOP de Arbitraje EVM 2026  |  CONFIDENCIAL
Página 24
La ubicación del VPS es crítica para minimizar la latencia. Los nodos RPC de Ethereum están distribuidos
globalmente, pero los más rápidos están en AWS us-east-1 (Virginia) y eu-central-1 (Frankfurt). ArbitrageX
recomienda desplegar el VPS principal en la misma región que el proveedor de RPC, idealmente usando
instancias bare-metal o dedicadas para evitar el "noisy neighbor" effect de la virtualización compartida.
•
Ethereum L1: AWS us-east-1 o eu-central-1, latencia objetivo <20ms al nodo RPC.
•
Arbitrum: VPS en la misma región del sequencer (us-east-1).
•
Base: VPS cercano al node RPC de Base (us-west-2 recomendado).
•
CEX Feed: VPS en Tokio para Binance APAC, o Londres/Frankfurt para OKX/Bybit.
16.3 Monitoreo y Alertas
El sistema de monitoreo de ArbitrageX está construido sobre Prometheus + Grafana, con alertas
configuradas para los siguientes eventos críticos: pérdida de conexión WebSocket, RPC timeouts,
simulaciones fallidas consecutivas, tasa de éxito de bundles por debajo del 50%, y beneficio diario negativo.
También se implementa un dashboard en tiempo real que muestra: operaciones por minuto, beneficio
acumulado, distribución por estrategia, latencia promedio de detección, y utilización de gas.
16.4 Dashboard de Seguimiento de Beneficios
El dashboard de beneficios muestra métricas agregadas en múltiples timeframes: horario, diario, semanal y
mensual. Incluye desglose por estrategia, por cadena, por par de tokens, y por DEX. Las métricas clave son:
PnL neto (después de gas), ROI, tasa de éxito de bundles, y costo total de gas. También muestra un gráfico de
equidad que permite visualizar el crecimiento compuesto del capital a lo largo del tiempo.
16.5 RPC Endpoints de Fallback
ArbitrageX mantiene al menos 3 proveedores de RPC por cadena para redundancia. Si el proveedor principal
no responde en menos de 50ms, el sistema conmuta automáticamente al siguiente en la lista. La
configuración recomendada por cadena es:
Cadena
RPC Primario
RPC Secundario
RPC Terciario
Ethereum
Alchemy (dedicated)
QuickNode (dedicated)
Flashbots RPC
Arbitrum
Alchemy ARB
QuickNode ARB
Public ARB RPC
Base
Alchemy BASE
QuickNode BASE
Public BASE RPC
BSC
Ankr BSC
QuickNode BSC
Public BSC RPC
Polygon
Alchemy MATIC
QuickNode MATIC
Public MATIC RPC
ArbitrageX — SOP de Arbitraje EVM v2.0 | Documento confidencial | Todos los derechos reservados | Generado
automáticamente | 2026
