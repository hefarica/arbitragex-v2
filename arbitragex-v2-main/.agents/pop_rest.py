import os

skills_data = {
    "cfmm-optimal-routing": {
        "SKILL.md": """# CFMM Optimal Routing

## Propósito
Calcula la ruta matemática óptima a través de múltiples Constant Function Market Makers (CFMMs) maximizando el profit final. Transforma el ruteo DEX en un problema de optimización convexa.

## Conocimiento esencial
En un grafo de liquidez con N pools, encontrar la ruta óptima requiere modelar cada pool por su función invariante. Para Uniswap V2 (CPMM), la invariante es `x * y = k`. El routing óptimo no solo busca la mejor ruta discreta, sino que divide el capital a través de múltiples rutas paralelas para reducir el slippage global.

## Integración con ARBITRAGEX
El `GraphBuilder` en Rust traduce las reservas de memoria a un grafo. El `OptimalRouter` evalúa rutas usando aproximaciones de búsqueda de sección dorada (Golden Section Search) para encontrar el monto de entrada óptimo.
""",
        "algorithms.md": """## Convex Optimization Routing
### Problema que resuelve
Encontrar la distribución óptima de capital a través de múltiples pares paralelos para minimizar el price impact combinado.
### Pseudocódigo
```python
def optimal_split(paths, total_in):
    # Newton-Raphson para resolver multiplicadores de Lagrange o 
    # Búsqueda binaria sobre el expected marginal return.
```
"""
    },
    "modified-bellman-ford-arbitrage-detection": {
        "SKILL.md": """# Modified Bellman-Ford Arbitrage Detection

## Propósito
Detecta ineficiencias de precios cíclicas (arbitraje) en el grafo de tokens mediante la identificación de ciclos negativos, utilizando pesos logarítmicos negativos de los rates de cambio con comisiones.

## Principios matemáticos
Un ciclo `c = (v1, v2, ..., vk, v1)` es rentable si el producto de los ratios de cambio a través de los pools es > 1.
Tomando logaritmos negativos: `sum(-ln(R_i)) < 0`. 
Esto reduce el problema a encontrar ciclos negativos en un grafo dirigido.

## Algoritmos incluidos
- Bellman-Ford (Modificado para relajar aristas usando rates en lugar de distancias puras y detenerse al encontrar ciclos negativos).
- SPFA (Shortest Path Faster Algorithm) iterativo.
"""
    },
    "uniswap-v3-concentrated-liquidity-math": {
        "SKILL.md": """# Uniswap V3 Concentrated Liquidity Math

## Propósito
Calcular con precisión de wei (1e-18) el output de un swap a través de posiciones de liquidez concentrada (ticks) sin simulación on-chain costosa.

## Conocimiento esencial
A diferencia de V2, V3 distribuye liquidez `L` en rangos de precios (ticks). El precio está representado como la raíz cuadrada del precio (`sqrtPriceX96`).

## Fórmulas
`L = delta_y / delta_sqrt_P`
`L = delta_x / (1/sqrt(P1) - 1/sqrt(P2))`
"""
    },
    "gas-aware-profitability-model": {
        "SKILL.md": """# Gas-Aware Profitability Model

## Propósito
Estimar el costo exacto de ejecución en L1 Ethereum (Base Fee + Priority Fee) para descontarlo del gross profit antes de la simulación pesada.

## Conocimiento esencial
EIP-1559 hace que el base fee sea predecible para el bloque `N+1`. Sin embargo, el priority fee (o bribe al builder de Flashbots) es competitivo. El modelo calcula el umbral de gas mínimo para romper el punto de equilibrio (break-even).
"""
    },
    "flashbots-bundle-construction": {
        "SKILL.md": """# Flashbots Bundle Construction

## Propósito
Empaquetar transacciones de arbitraje con firmas de búsqueda (searcher signatures) para enviarlas a los builders/relays (Flashbots, Titan, beaverbuild) mediante RPC privado.

## Conocimiento esencial
Los bundles garantizan ejecución atómica (todo o nada) sin riesgo de reversiones en cadena que cuesten gas (a menos que el revert suceda por un uncle block, lo cual es mitigado por relés privados).
"""
    },
    "evm-state-simulation-with-revm-anvil-alloy": {
        "SKILL.md": """# EVM State Simulation with REVM/Anvil

## Propósito
Validar determinísticamente la rentabilidad de una oportunidad ejecutando los opcodes de la EVM en memoria sin transmitir la transacción.

## Conocimiento esencial
Usar `revm` embebido en Rust es órdenes de magnitud más rápido que llamar a `eth_call` en un nodo local Erigon/Geth. Permite simular miles de bundles por bloque.
"""
    },
    "builder-landing-probability-model": {
        "SKILL.md": """# Builder Landing Probability Model

## Propósito
Estimar la probabilidad estocástica de que un bundle sea incluido en el siguiente bloque en base al fee pagado y al builder que ganó el slot (Proposer-Builder Separation).
"""
    },
    "rpc-latency-routing-and-circuit-breaker": {
        "SKILL.md": """# RPC Latency Routing and Circuit Breaker

## Propósito
Mantener una conexión websocket/HTTP resiliente con múltiples nodos (Geth, Alchemy, QuickNode). Cortar automáticamente (Circuit Break) si la latencia supera el umbral crítico para MEV (<50ms).
"""
    },
    "autonomous-skill-learning-pipeline": {
        "SKILL.md": """# Autonomous Skill Learning Pipeline

## Propósito
Orquestar la recolección, síntesis y generación de nuevas "Skills" de la memoria persistente a partir de foros, papers de Arxiv e incidentes de producción (Failure Modes).
"""
    }
}

base_dir = r"C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills"

for skill_name, files in skills_data.items():
    skill_dir = os.path.join(base_dir, skill_name)
    os.makedirs(skill_dir, exist_ok=True)
    
    for filename, content in files.items():
        with open(os.path.join(skill_dir, filename), "w", encoding='utf-8') as f:
            f.write(content)

print("Generated deep content for the rest of the Top 10 skills.")
