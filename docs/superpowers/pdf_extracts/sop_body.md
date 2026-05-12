Tabla de Contenidos
4
1. Panorama Completo de Estrategias DeFi en EVM
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 4
1.1 Clasificacion General de Estrategias
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 5
1.2 Factores Criticos de Exito
6
2. Estrategia DEX-DEX Directo
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 6
2.1 Procedimiento Operativo Estandar (SOP)
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 6
2.2 Mejores Condiciones y Seleccion de Tokens
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 7
2.3 Mejores Pools y DEXs por Blockchain
8
3. Arbitraje Triangular con Flash Loans
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 8
3.1 Proveedores de Flash Loans
 .  .  .  .  .  .  .  .  .  . 9
3.2 Mejores Triples de Tokens para Arbitraje Triangular
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 10
3.3 Ejemplo de Ejecucion en Rust con Alloy
11
4. Arbitraje Cross-Chain
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 12
4.1 Panorama de Blockchains EVM
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 12
4.2 Comparativa de Puentes (Bridges)
13
5. Arbitraje CEX-DEX
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 13
5.1 Configuracion de APIs de Exchanges
13
6. Liquidaciones MEV
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 14
6.1 Protocolos de Prestamos y Umbrales


15
7. Micro-Arbitraje de Alta Frecuencia
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 15
7.1 Modelos de Seleccion de Micro-Trades
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 15
7.2 Configuracion Optima por Red
16
8. Seleccion de Tokens y Pools Optimos
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 16
8.1 Checklist de Seguridad de Tokens
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 17
8.2 Metricas de Calidad de Pools
17
9. Seguridad y Prevencion de Robos
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 18
9.1 Tipos de Ataques y Estafas
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 18
9.2 Protecciones Implementables
18
10. Como Encontrar Liquidez
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 19
10.1 Fuentes de Datos de Liquidez
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 19
10.2 Liquidez Oculta y Emergente
20
11. Sistema de Algoritmos en Rust - Arquitectura
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 20
11.1 Estructura del Workspace Cargo
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 21
11.2 Arquitectura de Modulos
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 22
11.3 Flujo de Datos End-to-End
22
12. Deteccion y Ejecucion de Rutas en Rust
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 22
12.1 Modelo de Grafo de Tokens
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 24
12.2 Algoritmo de Deteccion Paralelo


 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 25
12.3 Motor de Ejecucion con Alloy
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 26
12.4 Simulacion con revm
26
13. Configuracion y Optimizacion del Toolkit
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 27
13.1 Infraestructura de Produccion
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 28
13.2 Parametros de Optimizacion Clave
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  . 28
13.3 Checklist de Despliegue


1. Panorama Completo de Estrategias DeFi 
en EVM
El ecosistema de finanzas descentralizadas (DeFi) en redes compatibles con la
Ethereum Virtual Machine (EVM) ofrece un universo de oportunidades de arbitraje
que van desde operaciones simples de compra-venta entre exchanges descentralizados
hasta estrategias complejas de liquidaciones, arbitraje cross-chain y micro-arbitraje de
alta frecuencia. La fragmentacion de liquidez entre multiples protocolos DEX, la
latencia en la actualizacion de precios y las diferencias en los mecanismos de
formacion de precios crean ineficiencias explotables de manera sistematica. Este
capitulo presenta un panorama completo de todas las estrategias vigentes, ordenadas
por rentabilidad, riesgo y complejidad, sirviendo como mapa de navegacion para los
capitulos detallados que siguen.
La seleccion de la estrategia adecuada depende de multiples factores: el capital
disponible, la infraestructura tecnica, el apetito al riesgo y la velocidad de ejecucion
requerida. Un operador con capital limitado pero acceso a flash loans puede ejecutar
arbitraje triangular sin capital propio, mientras que un operador con infraestructura
dedicada puede enfocarse en micro-arbitraje de alta frecuencia en L2s donde el costo
de gas es marginal. La clave esta en especializarse en las estrategias que mejor se
adapten a las ventajas competitivas del operador, manteniendo siempre un sistema
robusto de gestion de riesgo que proteja el capital contra perdidas inesperadas.
1.1 Clasificacion General de Estrategias
Las estrategias de arbitraje EVM se clasifican en ocho categorias principales, cada una
con caracteristicas unicas de rentabilidad, complejidad tecnica, nivel de riesgo y
capital requerido. La siguiente tabla presenta una comparativa exhaustiva que permite
evaluar rapidamente cual estrategia se alinea mejor con los recursos y objetivos del
operador. Es importante notar que estas categorias no son mutuamente excluyentes:
un sistema maduro de arbitraje tipicamente implementa multiples estrategias
simultaneamente, distribuyendo el riesgo y maximizando la captura de oportunidades.
Estrategia
Rentabili
dad
Compleji
dad
Riesgo
Capital
Req.
Veloci
dad
Blockchain
 Ideal
ROI Est.
DEX-DEX Direc
to
Alta
Baja-Med
ia
Bajo
Bajo
< 200m
s
Ethereum,
L2s
15-40%


Triangular
(Flash Loan)
Media-Al
ta
Media
Bajo-M
edio
Cero
< 300m
s
Ethereum
5-25%
Cross-Chain
Media
Alta
Medio-
Alto
Medio
1-30 se
g
Multi-chain
3-15%
CEX-DEX
Media-Al
ta
Media-Al
ta
Medio
Alto
< 500m
s
Ethereum,
BSC
10-30%
Liquidaciones
MEV
Variable
Muy Alta
Alto
Variable
< 150m
s
Ethereum
5-50%
Micro-Arbitraje
 HFT
Baja por
trade
Alta
Muy Ba
jo
Muy Bajo
< 100m
s
Arbitrum,
Base
20-60%
Yield Arbitrage
Media
Media
Bajo
Alto
Horas
Multi-chain
8-20%
Liquidity
Migration
Media
Media-Al
ta
Medio
Medio
Minuto
s
Emergentes
5-15%
Tabla 1. Comparativa exhaustiva de estrategias de arbitraje EVM
1.2 Factores Criticos de Exito
Independientemente de la estrategia elegida, existen factores criticos que determinan
el exito a largo plazo de cualquier operacion de arbitraje en EVM. El primer factor es
la latencia del sistema: el ciclo completo desde la deteccion de la oportunidad hasta la
inclusion de la transaccion en bloque debe ser inferior a 200 milisegundos para ser
competitivo contra bots institucionales. Esto requiere servidores dedicados cercanos a
los nodos de la blockchain, conexiones WebSocket de baja latencia y procesamiento
de datos en memoria compartida sin serializacion. El segundo factor es la proteccion
MEV: sin mecanismos de proteccion como Flashbots Protect o MEV Blocker, las
operaciones seran copiadas y frontrunneadas por bots mas rapidos, eliminando
cualquier margen de profit. El tercer factor es la gestion de riesgo automatizada:
limites de perdida por operacion, stop-loss diario, exposure maximo por token y
circuit breakers en periodos de alta volatilidad son esenciales para proteger el capital.
La infraestructura tecnica es la base sobre la cual se construyen todas las demas
capacidades. Nodos RPC dedicados ejecutando Erigon en modo archive proporcionan
acceso a datos historicos y eventos en tiempo real con latencia de microsegundos.
Bases de datos analiticas como ClickHouse permiten consultas rapidas sobre series
temporales de precios y metricas de rendimiento. Sistemas de monitoreo basados en
Grafana y Prometheus proporcionan visibilidad en tiempo real del estado del bot,
permitiendo detectar y resolver problemas antes de que impacten la rentabilidad.


Todo el sistema debe estar contenerizado con Docker y orquestado con Kubernetes
oDocker Compose para garantizar alta disponibilidad y escalabilidad horizontal.
2. Estrategia DEX-DEX Directo
El arbitraje DEX-DEX directo es la forma mas fundamental y accesible de arbitraje en
el ecosistema DeFi. Consiste en comprar un token en un exchange descentralizado
donde esta subvaluado y venderlo simultaneamente en otro donde esta sobrevaluado,
capturando el diferencial de precio como beneficio. Esta estrategia se presenta con
alta frecuencia debido a la fragmentacion natural de liquidez entre multiples
protocolos: el mismo par de tokens puede tener precios ligeramente diferentes en
Uniswap, SushiSwap, Curve, Balancer y 1inch en cualquier momento dado. El
arbitraje DEX-DEX es especialmente efectivo durante periodos de alta volatilidad,
cuando los precios se actualizan a diferentes velocidades en cada protocolo, y en
tokens con liquidez distribuida entre multiples pools de diferentes fee tiers.
2.1 Procedimiento Operativo Estandar (SOP)
El procedimiento para ejecutar arbitraje DEX-DEX directo sigue un pipeline definido
de seis pasos que debe ejecutarse de manera automatizada y atomica. El primer paso
es el monitoreo continuo de precios a traves de suscripciones WebSocket a los eventos
Swap de los contratos de los principales DEXs. Cada evento Swap contiene las
cantidades de tokens intercambiadas, permitiendo recalcular el precio spot del pool
usando la formula del AMM. El segundo paso es la deteccion de discrepancias: cuando
el precio de un token difiere entre dos o mas DEXs por un margen que supera el costo
estimado de gas mas un profit minimo configurado (tipicamente $5-$10 netos), se
identifica una oportunidad. El tercer paso es la simulacion: se ejecuta una simulacion
local (eth_call) de la transaccion de arbitraje para verificar que el profit esperado es
real y que no habra reverts por falta de liquidez. El cuarto paso es la construccion de la
transaccion, incluyendo los swaps secuenciales y el enrutamiento optimo. El quinto
paso es el envio a traves de un canal privado (Flashbots Protect o MEV Blocker) para
evitar frontrunning. El sexto paso es el monitoreo del resultado y ajuste de parametros.
2.2 Mejores Condiciones y Seleccion de Tokens
No todos los tokens son iguales para el arbitraje DEX-DEX. Los mejores candidatos
comparten caracteristicas especificas que maximizan la probabilidad de encontrar
oportunidades rentables. En primer lugar, los tokens deben tener alta liquidez en


multiples DEXs: un token listado solo en Uniswap v3 con un pool de $100,000 en
TVLraramente presentara discrepancias significativas con otros DEXs. Los
blue-chipscomo WETH, WBTC, USDC, USDT, DAI y los principales tokens DeFi (UNI,
AAVE,LINK, MKR) tienen liquidez distribuida en decenas de pools a traves de
multiplesprotocolos y fee tiers, creando frecuentes ineficiencias de precio. En
segundo lugar,los tokens con alta volatilidad relativa generan mas oportunidades
porque sus preciosse actualizan de forma asimetrica entre DEXs. Los pares estables
(USDC/USDT/DAI)son excepcionalmente rentables en terminos de volumen debido a
la alta frecuenciade oportunidades de pequeno margen (0.01%-0.05%) pero alto
volumen.
Criterio
Optimo
Aceptable
Riesgoso
Como Medir
TVL en pool
s
>$5M por pool
$500K-$5M
<$500K
The Graph, DeFi Llama
Volumen
24h
>$10M
$1M-$10M
<$1M
DEX volume aggregato
rs
DEXs listado
5+ DEXs
3-4 DEXs
1-2 DEXs
CoinGecko, DEXScreen
er
Volatilidad
24h
2-8%
0.5-2% / >8%
<0.5%
TradingView, oraculos
Slippage
0.1%
<0.02%
0.02-0.1%
>0.1%
eth_call simulacion
Auditoria
contrato
OpenZeppelin,
Trail
1 auditoria menor
Sin auditoria
DefiSafety, token.sniff
er
Tabla 2. Criterios de seleccion de tokens para arbitraje DEX-DEX
2.3 Mejores Pools y DEXs por Blockchain
La seleccion del pool adecuado es tan importante como la seleccion del token. Cada
DEX tiene caracteristicas unicas que lo hacen mas adecuado para ciertos tipos de
arbitraje. Uniswap v3 ofrece liquidez concentrada en ticks especificos, lo que permite
encontrar precios mas agresivos pero tambien genera mayor slippage en ordenes
grandes. Curve utiliza un modelo de AMM estable (StableSwap) optimizado para pares
de activos de valor similar, ofreciendo spreads ultrabajos para pares estables. Balancer
permite pools ponderados que pueden incluir hasta 8 tokens, creando oportunidades
unicas de arbitraje multilateral. SushiSwap ofrece pools de liquidez clasica (Uniswap


v2) con fee tiers flexibles y alta penetracion en cadenas alternativas. La
estrategiaoptima es mantener pools activos en multiples DEXs simultaneamente y
ejecutar lacomparacion de precios en tiempo real para cada par de tokens
monitorizado.
DEX
Modelo
Fee
Gas (est.)
Cadenas
Mejor Para
TVL Aprox.
Uniswap v3
Concentrad
o
0.01-1%
150-250K
15+
Blue chips,
volatiles
$5.2B
Curve
StableSwap
0.01-0.04%
200-350K
12+
Stables,
pegged
$3.8B
Balancer v2
Ponderado
0.01-1%
180-300K
10+
Multi-asset,
flash
$1.5B
SushiSwap
Constant
Product
0.05-0.3%
120-200K
20+
L2s, emergent
es
$800M
1inch
Aggregator
0-0.5%
Varia
15+
Ruta optima
N/A
Maverick
Dynamic
AMM
0.01-0.3%
140-220K
8+
Gamma tradin
g
$400M
Aerodrome
ve(3,3)
0.01-0.3%
120-180K
Base
Base nativa
$1.2B
Tabla 3. Comparativa de DEXs principales por caracteristicas
3. Arbitraje Triangular con Flash Loans
El arbitraje triangular con flash loans representa una de las estrategias mas elegantes
y capital-efficient del ecosistema DeFi. A diferencia del arbitraje DEX-DEX directo que
requiere capital propio, esta estrategia utiliza flash loans (prestamos sin colateral que
se solicitan y devuelven dentro de la misma transaccion) para ejecutar ciclos de tres o
mas intercambios sin necesidad de capital inicial. El concepto es simple pero
poderoso: si existen tres tokens A, B y C donde A se puede intercambiar por B a un
precio favorable, B por C favorablemente, y C de vuelta a A favorablemente, el ciclo
completo genera un beneficio neto que supera las fees del flash loan y el costo de gas.
Todo se ejecuta en una unica transaccion atomica: si cualquier paso falla, la
transaccion se revierte y no se pierde capital.
3.1 Proveedores de Flash Loans


La seleccion del proveedor de flash loan impacta directamente en la rentabilidad de la
operacion. Cada proveedor tiene diferentes modelos de fees, limites de prestamo y
soporte de tokens. Aave v3 es el proveedor mas utilizado con un premium del 0.05% (5
basis points) sobre el monto prestado, ofreciendo los mayores limites de prestamo y la
mayor variedad de tokens soportados. Balancer v2 cobra 0% de fee en muchos de sus
pools, lo que lo convierte en la opcion mas economica cuando el token deseado esta
disponible en un pool de Balancer. dYdX ofrece flash loans sin fee pero con soporte
limitado de tokens. Uniswap v3 ofrece flash swaps que permiten pagar al final de la
transaccion, funcionando de facto como un flash loan sin fee pero limitado a los
tokens disponibles en los pools de Uniswap. La estrategia optima es implementar un
selector dinamico de proveedor que evalue todas las opciones disponibles para cada
operacion y seleccione la mas economica.
Proveedor
Fee
Token Disponi
bles
Limite Maxim
o
Ventaja
Riesgo
Aave v3
0.05%
150+ tokens
Variable por
token
Mayor liquidez,
 multi-chain
Revert si no
repay
Balancer v2
0%
Pools Balancer
TVL del pool
Sin comision
Liquidez limita
da
dYdX
0%
USD, ETH
Limitado
Fee cero
Muy pocos
tokens
Uniswap v3
0%
Cualquier pool
Reservas pool
Universal, sin
contrato extra
Slippage variab
le
MakerDAO
0.09%
DAI
Muy alto
DAI ilimitado
Solo DAI
Tabla 4. Comparativa de proveedores de flash loans
3.2 Mejores Triples de Tokens para Arbitraje Triangular
La identificacion de triples de tokens con alta probabilidad de generar ciclos rentables
es un proceso que combina analisis estadistico con monitoreo en tiempo real. Las
triples mas efectivas combinan un token de alta liquidez como ancla (WETH o USDC)
con dos tokens que tengan diferentes mecanismos de pricing en DEXs distintos. Por
ejemplo, la triple WETH-USDC-LINK es eficaz porque LINK tiene diferentes niveles de
liquidez en Uniswap v3 vs. SushiSwap, creando frecuentes discrepancias de precio. La
triple USDC-USDT-DAI es especial por su alta frecuencia y bajo riesgo: como los tres
son stablecoins, la volatilidad es minima y las discrepancias surgen por diferencias en


los mecanismos de AMM, no por movimientos de precio. Otras triples
efectivasincluyen WETH-WBTC-ARB, WETH-AAVE-DAI, y WETH-OP-USDC. El sistema
debemantener una lista caliente de las 20-30 triples con mayor frecuencia
deoportunidades y priorizar la busqueda en esas combinaciones.
3.3 Ejemplo de Ejecucion en Rust con Alloy
El siguiente fragmento muestra la estructura basica del modulo de ejecucion de
arbitraje triangular utilizando Alloy, la libreria de Paradigm que reemplaza a
ethers-rs. Alloy ofrece ventajas fundamentales para arbitraje: decodificacion
zero-copy, WebSocket nativo resiliente y compatibilidad directa con revm para
simulacion de transacciones. El patron de diseno sigue la arquitectura C-S-E
(Chain-Searcher-Execution) donde cada componente opera de manera independiente
y se comunica a traves de canales de Rust para maxima concurrencia.
// triangular_arb.rs - Triangular arbitrage with Alloy
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, WsConnect};
use alloy::transports::ws::WsConnect as AlloyWs;
use alloy::sol_types::{SolCall, sol};
use std::sync::Arc;
use tokio::sync::mpsc;
// Flash loan callback ABI
sol! {
    function executeOperation(
        address asset, uint256 amount,
        uint256 premium, address initiator,
        bytes calldata params
    ) external returns (bool);
}
#[derive(Debug, Clone)]
pub struct ArbRoute {
    pub token_in: Address,
    pub token_mid: Address,
    pub token_out: Address,  // = token_in (cierre del ciclo)
    pub pool1: Address,


    pub pool2: Address,
    pub pool3: Address,
    pub dex1: u8,   // 1=UniV3, 2=Curve, 3=Balancer
    pub dex2: u8,
    pub dex3: u8,
    pub estimated_profit: U256,
    pub gas_cost: U256,
}
pub async fn scan_triangular(
    provider: Arc<impl Provider>,
    base_tokens: Vec<Address>,
    intermediate_tokens: Vec<Address>,
    pools: Vec<PoolInfo>,
) -> mpsc::Receiver<ArbRoute> {
    let (tx, rx) = mpsc::channel(256);
    // Para cada combinacion (base, mid, base):
    // 1. Calcular rate: base -> mid en pool_a
    // 2. Calcular rate: mid -> base en pool_b
    // 3. Si (rate_a * rate_b) > (1 + fees + gas_eth),
    //    emitir ArbRoute por el canal tx
    // 4. El execution engine recibe y ejecuta
    rx
}
4. Arbitraje Cross-Chain
El arbitraje cross-chain explota las diferencias de precio de un mismo token entre
diferentes blockchains compatibles con EVM. A medida que el ecosistema se ha
fragmentado en multiples Layer 2s (Arbitrum, Optimism, Base, zkSync), sidechains
(Polygon, BNB Chain) y rollups alternativos, la misma liquidez se ha distribuido a
traves de docenas de redes, creando ineficiencias de precio que persisten durante
segundos o incluso minutos debido a la latencia inherente en la sincronizacion de
precios a traves de puentes. Un token como WETH puede valer $3,001 en Ethereum
mainnet y $3,004 en Arbitrum durante un periodo de alta volatilidad, y esta diferencia
de $3 por ETH puede ser explotada si el costo del puente es inferior al beneficio neto.


El arbitraje cross-chain es mas complejo que el DEX-DEX porque
requiereinfraestructura multi-cadena, gestion de riesgos de puentes y coordinacion
temporal.
4.1 Panorama de Blockchains EVM
Blockcha
in
Tipo
Block
Time
Gas Pro
m.
TVL DeFi
Domina
ncia
DEX Princi
pal
Mejor Para
Ethereu
m
L1
12 seg
15-50 gwe
i
$180B
58%
Uniswap v3
Máx. liquidez
Arbitrum
L2 OP
0.25 se
g
0.1-0.5
gwei
$18B
12%
Uniswap v3
HFT, bajo gas
Optimis
m
L2 OP
2 seg
0.01-0.1
gwei
$7B
5%
Velodrome
Yield, bajo
gas
Base
L2 OP
2 seg
0.01-0.05
gwei
$8B
6%
Aerodrome
Emergente,
micro-ARB
BSC
Sidech
ain
3 seg
1-5 gwei
$5B
8%
PancakeSw
ap
Alto volumen
Polygon
Sidech
ain
2 seg
30-200
gwei
$1.5B
3%
QuickSwap
Multi-DEX
zkSync
Era
L2 ZK
1 seg
0.01-0.1
gwei
$1B
2%
Uniswap v3
Ultra-bajo gas
Avalanch
e
L1
2 seg
25-100
gwei
$1.2B
2%
Trader Joe
Subnets
Tabla 5. Panorama de blockchains EVM para arbitraje
4.2 Comparativa de Puentes (Bridges)
La seleccion del puente correcto es critica para el exito del arbitraje cross-chain. Los
factores a evaluar incluyen: tiempo de transferencia (finalizacion), costo en USD,
seguridad (auditorias, historial de exploits), soporte de tokens y disponibilidad
multi-cadena. Los puentes mas seguros y utilizados son los basados en sistemas de
validacion de confianza como LayerZero, que permite configurar la latencia de
finalizacion entre instantanea (mas cara) y lenta (mas economica). Stargate ofrece
transfers instantaneas con garantia de liquidez del otro lado. Across Protocol utiliza
un modelo de optimistas con tiempos de reclamo de 2 horas pero fees muy bajos. Los


puentes de liquidez como Synapse y Hop Protocol mantienen pools de
liquidezpre-posicionados en ambas cadenas, permitiendo transfers casi instantaneas.
Elsistema debe mantener conexiones con multiples puentes y
seleccionardinamicamente el mas rapido y economico para cada operacion.
5. Arbitraje CEX-DEX
El arbitraje CEX-DEX explota las diferencias de precio entre exchanges centralizados
(Binance, Coinbase, Kraken, OKX) y exchanges descentralizados. Esta estrategia es
especialmente potente durante periodos de alta volatilidad cuando los order books de
los CEXs no pueden actualizar sus precios suficientemente rapido, o cuando eventos
de mercado generan divergencias temporales entre los precios de mercado y los
precios on-chain determinados por los AMMs. A diferencia del arbitraje puro
DEX-DEX, el CEX-DEX requiere gestionar dos sistemas diferentes: la API
REST/WebSocket del CEX para monitoreo y ejecucion de ordenes, y la interaccion
on-chain con los smart contracts de los DEXs. La latencia del lado CEX (típicamente
10-100ms para APIs) es generalmente menor que la latencia on-chain (100-500ms para
inclusion en bloque), lo que introduce un riesgo temporal que debe gestionarse
cuidadosamente.
5.1 Configuracion de APIs de Exchanges
La integracion con APIs de CEXs requiere un setup cuidadoso para minimizar la
latencia y maximizar la fiabilidad. Binance ofrece WebSocket streams con latencia de
10-30ms para datos de mercado y APIs REST con rate limits de hasta 1200
requests/minuto para cuentas VIP. Coinbase Pro tiene WebSocket feeds con latencia
similar pero con un modelo de fees basado en volumen (maker 0%, taker 0.05-0.08%).
OKX y Bybit ofrecen APIs similares con ligeras diferencias en rate limits y fee
estructuras. La configuracion optima incluye: sesiones WebSocket persistentes con
reconexion automatica, order book local mantenido en memoria para calculo
instantaneo de spreads, y sistemas de fallback a REST API cuando la conexion
WebSocket se interrumpe. Para la ejecucion de ordenes, se utilizan ordenes limit (no
market) para garantizar el precio de ejecucion y evitar slippage adverso. El nonce
management debe ser robusto para evitar rechazos por nonce duplicado.
6. Liquidaciones MEV


Las liquidaciones MEV representan una de las estrategias mas rentables pero
tecnicamente demandantes del ecosistema DeFi. Cuando el precio de un collateral cae
por debajo del umbral de liquidacion en un protocolo de prestamos (Aave, Compound,
MakerDAO), cualquier persona puede liquidar la posicion, recibiendo una
recompensa (bonus de liquidacion) tipicamente del 5-10% del monto liquidado. En
Ethereum mainnet, estas liquidaciones pueden generar beneficios de $1,000-$50,000+
por operacion, pero la competicion es feroz: bot institucionales con acceso a builders
dedicados y sistemas de deteccion avanzada compiten por cada oportunidad en
microsegundos. La estrategia requiere: monitoreo continuo de las posiciones de todos
los usuarios de los protocolos de prestamos, deteccion instantanea cuando una
posicion se vuelve elegible para liquidacion, y ejecucion de la transaccion de
liquidacion con gas bidding agresivo a traves de Flashbots u otro sistema de
proteccion MEV.
6.1 Protocolos de Prestamos y Umbrales
Cada protocolo de prestamos tiene parametros de liquidacion diferentes que afectan
directamente la rentabilidad y frecuencia de las oportunidades. Aave v3 utiliza un
factor de colateralizacion (LTV) que varia por activo (tipicamente 75-80% para ETH,
0-50% para tokens volatiles) y un factor de liquidacion del 105-110% (se puede liquidar
cuando la relacion debt/collateral supera este umbral). La recompensa de liquidacion
en Aave es del 5% del monto liquidado. Compound v3 tiene parametros similares pero
con una interface de liquidacion diferente. MakerDAO utiliza vaults con
collaterization ratios individuales y un sistema de auction para las liquidaciones, lo
que introduce un componente temporal adicional. El sistema debe monitorizar todos
los protocolos simultaneamente y calcular en tiempo real cual posicion ofrece el
mejor ratio beneficio/costo de gas para cada oportunidad.
Protocolo
TVL
Umbral Liq.
Bonus Liq.
Gas Est.
Complexidad
Aave v3
$12B
105-110%
5%
250-450K gas
Media - batch
liquidation
Compound
v3
$3B
107-112%
8%
200-350K gas
Media
MakerDAO
$8B
Variable
por vault
13% (auction)
300-500K gas
Alta - auction
system
Spark Protoc
ol
$1.5B
105-110%
5%
220-380K gas
Media


Tabla 6. Protocolos de prestamos y parametros de liquidacion
7. Micro-Arbitraje de Alta Frecuencia
El micro-arbitraje de alta frecuencia (HFT) es la estrategia que transforma beneficios
pequenos pero consistentes en rentabilidad significativa a traves del volumen.
Mientras que un arbitraje tradicional busca oportunidades de $50-$500 por operacion
que ocurren decenas de veces al dia, el micro-arbitraje se enfoca en oportunidades de
$0.10-$5.00 que ocurren cientos de veces por hora en las redes Layer 2 donde el costo
de gas es marginal (Arbitrum: $0.001-$0.01, Base: $0.0005-$0.005 por transaccion). La
clave matematica es simple: si cada operacion genera un beneficio neto de $0.50
despues de gas, y el sistema ejecuta 200 operaciones exitosas por hora con una tasa de
exito del 85%, el beneficio horario es de $85, lo que equivale a $2,040 diarios. Este
modelo es escalable y potencialmente mas estable que el arbitraje de gran beneficio
pero baja frecuencia, porque las oportunidades de micro-arbitraje surgen
constantemente y son menos visibles para bots competidores.
7.1 Modelos de Seleccion de Micro-Trades
La seleccion de micro-trades requiere criterios especializados que difieren del
arbitraje tradicional. El primer criterio es el costo de gas absoluto: si el gas cuesta
$0.005 en Arbitrum, el beneficio minimo por operacion debe ser de al menos $0.01 (2x
el costo de gas como margen de seguridad). El segundo criterio es la tasa de exito: las
operaciones deben tener una probabilidad estimada de exito superior al 90% para que
el modelo matematico sea viable. Esto se logra ejecutando solo operaciones donde el
spread medido es significativamente mayor al slippage estimado (minimo 3x). El
tercer criterio es la velocidad de ejecucion: el ciclo deteccion-ejecucion debe ser
inferior a 100ms para capturar la ventana de oportunidad antes que otros bots. El
cuarto criterio es el monitoreo de densidad: el sistema debe contar las operaciones
exitosas por hora y ajustar dinamicamente los umbrales de profit minimo para
maximizar el throughput sin comprometer la tasa de exito.
7.2 Configuracion Optima por Red


Red
Gas Cost
Profit
Min
Ops/Hor
a
Volumen
Ops
Beneficio/
Hora
Estrategia Ideal
Arbitrum
$0.001-0.01
$0.01
100-500
$0.10-2.00
$50-200
Stable pools,
USDC/USDT
Base
$0.0005-0.0
05
$0.005
200-800
$0.05-1.00
$40-300
Aerodrome pools
Optimism
$0.001-0.00
5
$0.01
80-300
$0.10-3.00
$30-150
Velodrome,
wETH pairs
zkSync Era
$0.001-0.00
8
$0.01
50-200
$0.10-2.50
$20-100
Emergente, bajo
gas
Tabla 7. Configuracion optima de micro-arbitraje por red
8. Seleccion de Tokens y Pools Optimos
La seleccion rigurosa de tokens y pools es la primera linea de defensa contra perdidas
y la base de una operacion de arbitraje exitosa a largo plazo. Un token mal
seleccionado puede resultar en una transaccion revertida (con perdida de gas), una
posicion atrapada en un token iliquido, o peor aun, una interaccion con un contrato
malicioso que drena los fondos. Este capitulo establece los criterios, herramientas y
procedimientos para la evaluacion sistematica de tokens y pools antes de incluirlos en
el sistema de arbitraje.
8.1 Checklist de Seguridad de Tokens
Antes de interactuar con cualquier token ERC-20, el sistema debe ejecutar un checklist
de seguridad automatizado que verifique los siguientes puntos criticos. En primer
lugar, verificar que el contrato no contiene funciones de minteo ilimitado (supply
infinito), funciones de pausa admin, o mecanismos de fee on transfer que puedan
alterar los montos recibidos. En segundo lugar, verificar que no hay blacklists o
allowances pre-aprobados a direcciones sospechosas. En tercer lugar, verificar la
liquidez bloqueada: si la liquidez del pool no esta bloqueada en un contrato de
timelock o es removible por el owner, existe un riesgo de rug pull. En cuarto lugar,
verificar el holder distribution: si el 50%+ del supply esta en manos del top 10 holders,
el token es altamente manipulable. Herramientas como Token Sniffer, De.Fi, y
Honeypot.is automatizan gran parte de estas verificaciones a traves de APIs.


Verificacion
Paso Falle
Herramienta
Accion
No minteo infinito
RECHAZAR token
Token Sniffer API
Excluir del grafo
Sin blacklist/pausable
RECHAZAR token
Etherscan verify
Excluir del grafo
Liquidez bloqueada
>6M
ADVERTENCIA
De.Fi Scanner
Reducir max amount
Holder distribution OK
ADVERTENCIA
Etherscan holders
Reducir exposure
Auditoria (OpenZeppel
in)
INFO
DefiSafety.com
Priorizar auditados
No fee on transfer >1%
RECHAZAR token
eth_call test
Excluir del grafo
Tabla 8. Checklist de seguridad para seleccion de tokens
8.2 Metricas de Calidad de Pools
La calidad de un pool de liquidez determina directamente la rentabilidad y la
seguridad del arbitraje. Los pools de alta calidad comparten las siguientes
caracteristicas: TVL elevado (superior a $1M para L2s, $10M+ para Ethereum mainnet),
volumen de trading alto (superior a $500K/24h), fee tier apropiado para la volatilidad
del par (0.01% para stables, 0.3% para blue chips, 1% para volatiles), y una
distribucion de liquidez concentrada en rangos de precio activos (para Uniswap v3).
Los pools deben ser monitoreados continuamente para detectar cambios en TVL
(liquidez entrando o saliendo) y volumen (cambios en la actividad de trading que
puedan afectar la disponibilidad de oportunidades). Un pool que pierde liquidez
rapidamente puede volverse ineficiente para arbitraje debido al aumento del slippage.
9. Seguridad y Prevencion de Robos
La seguridad es el pilar mas critico y frecuentemente subestimado de cualquier
sistema de arbitraje DeFi. El ecosistema esta plagado de tokens fraudulentos
(honeypots), contratos maliciosos, puentes vulnerables y ataques de ingenieria social.
Un solo error de seguridad puede resultar en la perdida total del capital del bot. Este
capitulo detalla los riesgos de seguridad mas comunes, los metodos de deteccion y las
mejores practicas para proteger los fondos y la operacion del sistema de arbitraje.


9.1 Tipos de Ataques y Estafas
Los principales tipos de ataques que enfrenta un operador de arbitraje se dividen en
tres categorias: ataques al contrato del propio bot, ataques a los tokens con los que
interactua, y ataques a la infraestructura. Los ataques al contrato incluyen la
explotacion de vulnerabilidades en el contrato de arbitraje (reentrancy,
overflow/underflow, access control bypass), ataques de sandwitch donde un bot
adversario frontrunnea y backrunnea la operacion para capturar parte del profit, y
ataques de frontrunning puro donde el bot adversario copia la operacion y la ejecuta
primero. Los ataques a tokens incluyen honeypots (tokens que solo permiten comprar
pero no vender), tokens con fee on transfer oculto (que reducen el monto recibido en
cada transferencia), y rug pulls (donde el equipo remueve la liquidez repentinamente).
Los ataques a la infraestructura incluyen compromiso de claves privadas, ataque a
nodos RPC, y explotacion de vulnerabilidades en los servidores.
9.2 Protecciones Implementables
Riesgo
Proteccion
Implementacion
Honeypot tokens
Verificacion pre-trade
Token Sniffer API + eth_call test
Sandwitch attack
Private tx service
Flashbots Protect / MEV Blocker
Frontrunning
Private mempool
BloxRoute BDN / Eden Network
Rug pull
Liquidez bloqueada check
DefiSafety + Team Finance verify
Contract exploit
Auditoria + fuzz testing
Foundry fuzz + external audit
Key compromise
HSM + multi-sig
Ledger HSM + Gnosis Safe
RPC manipulation
Multiple RPCs + verify
Own Erigon + 2 backups
Fee on transfer
Balance delta check
Compare balance antes/despues eth_call
Tabla 9. Mapeo de riesgos y protecciones implementables
10. Como Encontrar Liquidez
La liquidez es el recurso fundamental que hace posible cualquier operacion de
arbitraje. Sin liquidez suficiente, los swaps generan slippage excesivo que consume
todo el margen de beneficio. Encontrar liquidez dispersa, oculta o emergente es una


ventaja competitiva significativa: pools recien creados en DEXs nuevos,
liquidezfragmentada a traves de multiples fee tiers, y liquidez temporal en pools
incentivadosrepresentan oportunidades que muchos bots no monitorizan. Este
capitulo describelos metodos, herramientas y estrategias para construir un mapa
completo de liquidezdel ecosistema DeFi.
10.1 Fuentes de Datos de Liquidez
Las fuentes primarias de datos de liquidez incluyen: eventos on-chain (eventos Swap,
Mint, Burn de los contratos de los DEXs), indexadores de datos como The Graph (con
subgraphs dedicados para cada DEX), APIs de DEX aggregators (1inch, Paraswap, 0x)
que agregan liquidez de multiples fuentes, y bases de datos on-chain como DeFi Llama
que trackean TVL y volumenes. Para datos en tiempo real, las suscripciones
WebSocket a los eventos de los contratos son la fuente mas rapida y confiable. Para
datos historicos, The Graph permite consultas GraphQL que pueden reconstruir el
estado completo de todos los pools en cualquier punto del tiempo. La combinacion de
fuentes en tiempo real e historicas permite al sistema mantener un mapa de liquidez
siempre actualizado y anticipar movimientos de liquidez antes de que se materialicen
en oportunidades de arbitraje.
10.2 Liquidez Oculta y Emergente
Existen categorias de liquidez que no son visibles en los dashboards publicos pero que
representan oportunidades significativas. La primera es la liquidez en pools de fee
tiers no convencionales en Uniswap v3: muchos operadores solo monitorizan los fee
tiers de 0.3% y 1%, pero pools con fee de 0.01% o 0.05% pueden tener liquidez
significativa y spreads mas ajustados. La segunda es la liquidez en DEXs emergentes o
de nicho: protocolos como Maverick Protocol, Algebra Finance, Curve v2 pools crypto
(no solo stable), y los DEXs nativos de cada L2 (Aerodrome en Base, Velodrome en
Optimism) tienen liquidez que muchos bots institucionales no monitorizan. La tercera
es la liquidez incentivada temporalmente: cuando un protocolo lanza un programa de
recompensas (farming), grandes cantidades de liquidez fluyen hacia sus pools,
creando oportunidades de arbitraje durante la fase de entrada. El sistema debe tener
la capacidad de descubrir automaticamente nuevos pools y agregarlos al monitoreo
con un proceso de verificacion de seguridad previo.


11. Sistema de Algoritmos en Rust - 
Arquitectura
Este capitulo presenta el diseno completo del sistema de arbitraje implementado en
Rust, utilizando Alloy (Paradigm/alloy-rs) como libreria principal para interaccion con
la EVM. Rust es el lenguaje optimo para sistemas de arbitraje de alta frecuencia
debido a su rendimiento nativo (sin garbage collector), garantias de seguridad de
memoria (zero-cost abstractions), y ecosistema de concurrencia (tokio async runtime).
Alloy es la sucesora oficial de ethers-rs, deprecada desde Octubre 2023, y ofrece
ventajas fundamentales: modularidad (importas solo lo que usas), decodificacion
zero-copy (critica al procesar miles de transacciones del mempool por segundo),
WebSocket nativo resiliente con reconexion automatica, y compatibilidad directa con
revm para simulacion de transacciones sin conversiones de tipos entre librerias.
11.1 Estructura del Workspace Cargo
# Cargo.toml - Workspace principal
[workspace]
members = [
    "crates/price-monitor",
    "crates/graph-engine",
    "crates/detector",
    "crates/executor",
    "crates/risk-manager",
    "crates/shared",
    "crates/sim-engine",
    "crates/relays-client",
]
resolver = "2"
[workspace.dependencies]
alloy = { version = "0.9", features = ["full"] }
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
alloy-provider = { version = "0.9", features = ["ws"] }
alloy-rpc-types = "0.9"


alloy-transport-ws = "0.9"
tokio = { version = "1", features = ["full"] }
revm = "3.5"
redis = { version = "0.25", features = ["tokio-comp"] }
serde = { version = "1", features = ["derive"] }
thiserror = "1"
tracing = "0.1"
11.2 Arquitectura de Modulos
El sistema sigue la arquitectura C-S-E (Chain-Searcher-Execution) propuesta por 
Paradigm, adaptada para arbitraje. Cada modulo opera como un microservicio 
independiente que se comunica a traves de canales de Rust (tokio::sync::mpsc) para 
latencia de microsegundos. El modulo price-monitor mantiene conexiones WebSocket 
con multiples DEXs y cadenas, almacenando precios en memoria compartida (Arc). El 
modulo graph-engine construye y mantiene un grafo dirigido ponderado de tokens 
donde cada nodo es un token y cada arista es un par de trading con su peso (log 
negativo del tipo de cambio neto). El modulo detector ejecuta el algoritmo de 
Bellman-Ford modificado sobre el grafo para encontrar ciclos negativos 
(oportunidades de arbitraje) en paralelo usando Rayon. El modulo executor construye 
y envia transacciones a traves de Flashbots Protect. El modulo risk-manager impone 
limites y circuit breakers. El modulo sim-engine utiliza revm para simular 
transacciones localmente antes del envio.
Modulo
Funcion
Tecnologia Clave
Latencia
Comunicacion
price-monito
r
WS price feed
alloy-provider,
WebSocket
< 20ms
mpsc -> detector
graph-engine
Token graph
petgraph, Arc
< 1ms update
shared memory
detector
Bellman-Ford +
Rayon
rayon, parallel iter
< 5ms
mpsc -> executor
executor
Build + send tx
alloy-provider,
Flashbots
< 100ms
mpsc -> risk-mgr
risk-manager
Limits + breakers
Custom rules
engine
< 1ms
Redis pub/sub
sim-engine
revm simulation
revm 3.5, alloy-pri
mitives
< 15ms
mpsc -> executor


shared
Types + utils
alloy-primitives,
serde
N/A
Shared crate
Tabla 10. Arquitectura de modulos del sistema en Rust
11.3 Flujo de Datos End-to-End
El flujo de datos del sistema opera en un pipeline continuo de cuatro fases: captura, 
procesamiento, decision, y ejecucion. En la fase de captura, el price-monitor recibe 
eventos Swap a traves de WebSocket y los decodifica usando alloy-sol-types con 
zero-copy. Cada precio actualizado se publica en un canal mpsc y tambien se actualiza 
en el grafo compartido (Arc>). En la fase de procesamiento, el detector se despierta 
con cada actualizacion del grafo y ejecuta una pasada de Bellman-Ford para cada 
token base en paralelo usando el pool de threads de Rayon. Si se detecta un ciclo 
negativo con peso acumulado inferior al umbral configurado (equivalente a un profit 
neto positivo despues de gas y fees), la ruta se reconstruye y se envia al sim-engine 
para verificacion. En la fase de decision, el sim-engine ejecuta una simulacion local 
usando revm con el estado mas reciente del blockchain. Si la simulacion confirma un 
profit positivo, la oportunidad se envia al risk-manager para validacion de limites. En 
la fase de ejecucion, el executor construye la transaccion con los parametros optimos 
(gas bidding, nonce, flash loan provider), la firma con la clave privada almacenada en 
HSM, y la envia a traves de Flashbots Protect para inclusion privada.
12. Deteccion y Ejecucion de Rutas en Rust
Este capitulo profundiza en los algoritmos nucleares de deteccion de oportunidades y
ejecucion de rutas de arbitraje. El componente de deteccion es el cerebro del sistema:
transforma el flujo continuo de actualizaciones de precios en decisiones de trading
accionables. El componente de ejecucion materializa esas decisiones en transacciones
on-chain atomicas. Ambos componentes deben operar con latencia minima y_maxima
confiabilidad, ya que cualquier retraso o error se traduce directamente en perdida de
oportunidades o perdida de capital.
12.1 Modelo de Grafo de Tokens
// graph_engine/src/lib.rs


use alloy::primitives::{Address, U256};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
pub struct TokenGraph {
    graph: DiGraph<TokenNode, TradeEdge>,
    addr_to_idx: HashMap<Address, NodeIndex>,
    base_tokens: Vec<Address>,  // WETH, USDC, USDT, DAI
}
#[derive(Debug, Clone)]
pub struct TokenNode {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub chains: Vec<u64>,  // chain IDs donde existe
}
#[derive(Debug, Clone)]
pub struct TradeEdge {
    pub pool_address: Address,
    pub dex_type: DexType,     // UniV3, Curve, Balancer, etc.
    pub reserve_in: U256,
    pub reserve_out: U256,
    pub fee: f64,              // 0.003 = 0.3%
    pub weight: f64,           // -ln(rate * (1-fee))
    pub chain_id: u64,
    pub last_update: u64,      // timestamp ms
}
#[derive(Debug, Clone, Copy)]
pub enum DexType { UniswapV3, Curve, Balancer, SushiSwap, Custom(u8) }
impl TokenGraph {
    pub fn new(base_tokens: Vec<Address>) -> Self {
        Self { graph: DiGraph::new(), addr_to_idx: HashMap::new(),
               base_tokens }


    }
    pub fn update_edge(&mut; self, from: Address, to: Address,
                       edge: TradeEdge) {
        // Actualizar peso: w = -ln(amount_out / amount_in * (1 - fee))
        let rate = edge.reserve_out.to::<f64>()
                  / edge.reserve_in.to::<f64>();
        let net_rate = rate * (1.0 - edge.fee);
        let weight = -net_rate.ln();
        // Insertar o actualizar arista en el grafo
    }
    /// Buscar ciclos negativos = oportunidades de arbitraje
    pub fn find_arbitrage_cycles(
        &self;,
        max_hops: usize,
        min_profit_bps: u64,  // basis points minimos
    ) -> Vec<ArbRoute> {
        // Bellman-Ford paralelo con Rayon para cada base_token
        // Retorna lista de rutas ordenadas por profit estimado
        vec![]  // Placeholder
    }
}
12.2 Algoritmo de Deteccion Paralelo
El algoritmo de deteccion esta optimizado para ejecutarse en paralelo sobre el pool de
threads de Rayon. Para cada token base (WETH, USDC, USDT), se ejecuta una variante
del algoritmo de Bellman-Ford que busca ciclos negativos en el grafo de tokens. La
variacion clave respecto al Bellman-Ford estandar es la limitacion del numero maximo
de saltos (max_hops = 4-6) para mantener el tiempo de ejecucion bajo los 5
milisegundos y porque en la practica, las oportunidades de arbitraje con mas de 4
saltos raramente son rentables despues de los fees acumulados. El paralelismo se
implementa a dos niveles: entre tokens base (cada base token se procesa en un thread
independiente) y dentro de cada token base (las iteraciones de Bellman-Ford se
paralelizan sobre los vecinos de cada nodo usando rayon::par_iter). Un cache de rutas
calientes almacena las 50 rutas mas frecuentes y las re-evalua instantaneamente


cuando cambian los precios, reduciendo el tiempo de deteccion de las
oportunidadesmas comunes de milisegundos a microsegundos.
12.3 Motor de Ejecucion con Alloy
// executor/src/lib.rs
use alloy::primitives::{Address, U256, Bytes};
use alloy::providers::{Provider, HttpProvider, WsConnect};
use alloy::transports::http::Http;
use alloy::rpc::types::TransactionRequest;
pub struct Executor {
    provider: Arc<HttpProvider>,
    signer: Arc<LocalSigner>,
    flashbots_relay: String,
    chain_id: u64,
}
impl Executor {
    /// Construir y enviar transaccion de arbitraje
    pub async fn execute_arbitrage(
        &self;,
        route: &ArbRoute;,
        flash_loan_provider: Address,
        gas_price: U256,
    ) -> Result<TxHash, ExecError> {
        // 1. Construir calldata del contrato de arbitraje
        let calldata = self.build_arb_calldata(route, flash_loan_provider);
        // 2. Estimar gas con eth_estimateGas
        let gas_estimate = self.provider
            .estimate_gas(&self.contract;_address, &calldata;).await?;
        // 3. Construir transaccion EIP-1559
        let tx = TransactionRequest::default()
            .to(self.contract_address)
            .input(calldata.into())
            .gas_limit(gas_estimate * 110 / 100)  // +10% buffer


            .max_fee_per_gas(gas_price * 120 / 100)
            .max_priority_fee_per_gas(gas_price * 115 / 100)
            .chain_id(self.chain_id)
            .nonce(self.provider.get_nonce(&self.signer.address;()).await?);
        // 4. Firmar y enviar via Flashbots Protect
        let signed = self.signer.sign_transaction(tx).await?;
        let tx_hash = self.send_private(&signed;).await?;
        Ok(tx_hash)
    }
    /// Enviar transaccion privada via Flashbots Protect
    async fn send_private(&self;, signed_tx: &Bytes;)
        -> Result<TxHash, ExecError> {
        // POST to https://rpc.flashbots.net/fast
        // o https://mevblocker.io/relay
        todo!("Flashbots relay integration")
    }
}
12.4 Simulacion con revm
La simulacion local de transacciones es un paso critico que filtra falsos positivos antes
de enviar transacciones on-chain, evitando la perdida de gas en operaciones que
revertirian. El modulo sim-engine utiliza revm ( Revolutionary Ethereum Virtual
Machine ) para ejecutar la transaccion de arbitraje en un entorno local con el estado
mas reciente del blockchain. La ventaja de revm sobre la simulacion via RPC (eth_call)
es la velocidad: revm puede ejecutar transacciones complejas en 5-15 milisegundos,
mientras que eth_call a traves de un nodo RPC tipicamente toma 50-200 milisegundos.
Ademas, la compatibilidad directa entre Alloy y revm (ambos usan alloy-primitives)
elimina las conversiones de tipos que serian necesarias con ethers-rs. La simulacion
incluye: ejecucion del flash loan, todos los swaps intermedios, el reembolso del
prestamo, y verificacion del balance final. Si el balance final del token base es menor
al monto prestado mas el premium, la operacion se descarta sin enviar a cadena.
13. Configuracion y Optimizacion del Toolkit


La configuracion y el despliegue del toolkit de arbitraje es el paso final que transforma
el codigo en un sistema de produccion que genera beneficios reales. Este capitulo
cubre la infraestructura de servidores, la configuracion de nodos RPC, el monitoreo y
alertas, los parametros de optimizacion, y el checklist de despliegue. Un despliegue
exitoso requiere atencion a cada detalle de la infraestructura, desde la ubicacion fisica
de los servidores hasta la configuracion de los canales de alerta que notificaran al
operador cuando el sistema necesite intervencion humana.
13.1 Infraestructura de Produccion
La infraestructura de produccion sigue los principios de los sistemas de trading de alta
frecuencia: baja latencia, alta disponibilidad, redundancia y escalabilidad horizontal.
Los servidores principales deben ubicarse en centros de datos de AWS en eu-central-1
(Frankfurt) o us-east-1 (Virginia) para minimizar la latencia con los nodos de
Ethereum y los builders de bloques. Cada componente del sistema se despliega como
un contenedor Docker independiente, orquestado con Docker Compose para
simplicidad o Kubernetes para escalabilidad avanzada. La comunicacion entre
componentes utiliza gRPC con protocol buffers para serializacion binaria de baja
latencia, complementada por Redis Pub/Sub para notificaciones de eventos en tiempo
real. ClickHouse sirve como base de datos analitica para consultas de alto rendimiento
sobre series temporales de precios y metricas de rendimiento.
Componente
Recursos
Configuracion
Costo/Mes
Proveedor Alt.
Price Monitor
2 vCPU, 4GB
RAM
Docker, Alpine
$40-80
Hetzner Cloud
Detection Engine
4 vCPU, 16GB
RAM
Rust native, release
$100-200
AWS c6i.xlarge
Execution Engine
2 vCPU, 8GB
RAM
Docker + HSM
$80-150
Ledger + Docker
Redis (Pub/Sub)
2 vCPU, 8GB
RAM
Redis 7+, AOF
$50-100
ElastiCache
ClickHouse
4 vCPU, 32GB
RAM
Cluster, replica
$150-300
TimescaleDB
RPC Node propio
16 vCPU, 64GB
RAM
Erigon, archive,
NVMe
$400-600
QuickNode
Enterprise
Tabla 11. Infraestructura de produccion recomendada


13.2 Parametros de Optimizacion Clave
Parametro
Default
Rango Seguro
Accion si Excede
Ajuste Dinamico
Profit min/trade
$5.00
$3-$50
Reject
Ajustar por
volatilidad
Slippage max
0.5%
0.1-1.0%
Revert tx
Reducir en alta
vol
Loss max diaria
$200
$50-$500
Pause bot
Reset diario
Exposure max/token
20%
10-30%
Reject trades
Ajustar por TVL
Max ops/bloque
3
1-5
Queue delay
Ajustar por gas
Timeout operacion
3 bloques
1-5 bloques
Cancel tx
Ajustar por chain
Gas price max
Auto
30-100 gwei (L1)
Skip opp
Base fee + margin
Cache routes size
50
20-100
Evict LRU
Frecuencia de
hits
Tabla 12. Parametros de configuracion y ajuste dinamico
13.3 Checklist de Despliegue
Antes de activar el sistema en produccion, es esencial completar un checklist
exhaustivo que verifique cada componente y configuracion. El checklist incluye: (1)
Verificar que todos los contratos inteligentes han sido auditados y desplegados en las
cadenas objetivo. (2) Verificar que las claves privadas estan almacenadas en HSM y
nunca expuestas en variables de entorno o logs. (3) Verificar que existen al menos 2
conexiones RPC redundantes por cadena con failover automatico. (4) Verificar que los
canales de alerta (Telegram, Slack) estan configurados y funcionando. (5) Ejecutar el
sistema en modo paper-trading (simulacion sin transacciones reales) durante al
menos 24 horas para validar la deteccion y ejecucion. (6) Activar el sistema con capital
minimo ($500-$1000) y operar durante 48 horas monitoreando metricas clave: tasa de
exito, profit por operacion, latencia total, y gas gastado. (7) Si las metricas son
positivas (tasa de exito mayor al 70%, profit neto positivo), escalar el capital
gradualmente. (8) Configurar backup automatico de la base de datos y logs cada 6
horas.
El monitoreo continuo es esencial para la operacion a largo plazo. Un dashboard de
Grafana muestra: profit/loss acumulado (diario, semanal, mensual), tasa de exito de


transacciones, gas gastado vs. profit generado, oportunidades detectadas
vs.ejecutadas, latencia promedio de deteccion-ejecucion, y exposure por token.
Lasalertas se configuran para eventos criticos: el bot se pausa automaticamente, la
tasa deexito cae por debajo del 60%, el gas acumulado supera el limite diario, un nodo
RPCdeja de responder, o el balance de la wallet cae por debajo de un umbral minimo.
Lacapacidad de iterar rapidamente sobre parametros (fee tiers, tokens
monitorizados,umbrales de profit) sin necesidad de redeploy es una ventaja
arquitectonica quedistingue a los sistemas exitosos de los que se estancan. El
ecosistema DeFi evolucionaconstantemente: nuevos DEXs aparecen, mecanismos de
pricing cambian, y lasestrategias de los competidores se adaptan. Un toolkit que no
evoluciona se vuelveobsoleto en cuestiones de semanas.
