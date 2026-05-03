import os

skills_data = {
    "uniswap-v2-cpmm-math": {
        "SKILL.md": """# Uniswap V2 CPMM Math

## Propósito
Proveer las primitivas matemáticas exactas para simular trades, calcular slippage y encontrar tamaños de trade óptimos en pools de Constant Product Market Maker (x * y = k) bajo el estándar Uniswap V2, considerando las tarifas dinámicas de los DEX forks (SushiSwap, PancakeSwap).

## Conocimiento esencial
El modelo `x * y = k` asume liquidez infinita teórica, pero un slippage asintótico. Para que un trade sea validado sin consultar el estado on-chain constantemente, el searcher debe calcular localmente el `amountOut` exacto dado un `amountIn`, considerando la comisión (fee) del pool (típicamente 0.3%).

## Principios matemáticos
Dadas reservas `R_in` y `R_out`, y un monto de entrada `A_in` con comisión `f` (ej. 0.003):
`A_in_with_fee = A_in * (1 - f)`
`A_out = (A_in_with_fee * R_out) / (R_in + A_in_with_fee)`

Para calcular el input exacto necesario para un output deseado:
`A_in = (R_in * A_out) / ((R_out - A_out) * (1 - f)) + 1`

## Integración con ARBITRAGEX
El crate `dex_math` en Rust implementará estas ecuaciones usando aritmética de punto fijo (U256) para evitar desbordamientos, garantizando consistencia absoluta con la EVM.
""",
        "algorithms.md": """## V2 Amount Out Calculator
### Pseudocódigo (Rust style)
```rust
fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee: u32) -> Result<U256, MathError> {
    let amount_in_with_fee = amount_in * (10000 - fee);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = (reserve_in * 10000) + amount_in_with_fee;
    Ok(numerator / denominator)
}
```
"""
    },
    "curve-stableswap-arbitrage-math": {
        "SKILL.md": """# Curve StableSwap Arbitrage Math

## Propósito
Modelar la invariante StableSwap de Curve Finance para detectar discrepancias sutiles de precio entre stablecoins o activos pegados (ej. stETH/ETH), donde la liquidez es extremadamente profunda cerca de la paridad.

## Conocimiento esencial
La invariante de Curve combina Constant Sum (liquidez densa en 1:1) y Constant Product (protección de liquidez en los extremos) mediante un parámetro de amplificación `A`.

## Principios matemáticos
Invariante de Curve:
`A * n^n * sum(x_i) + D = A * n^n * D + D^(n+1) / (n^n * prod(x_i))`
El cálculo iterativo (Newton-Raphson) es obligatorio para predecir los resultados de los swaps de Curve on-chain.
"""
    },
    "balancer-weighted-pool-routing": {
        "SKILL.md": """# Balancer Weighted Pool Routing

## Propósito
Modelar los pools multidimensionales de Balancer (hasta 8 tokens) donde las reservas tienen pesos asimétricos (ej. 80/20).

## Principios matemáticos
Invariante ponderada:
`prod(R_i ^ W_i) = V`
El cálculo de Spot Price y AmountOut depende de los pesos `W_in` y `W_out`.
`A_out = R_out * (1 - (R_in / (R_in + A_in * (1 - fee))) ^ (W_in / W_out))`
"""
    },
    "mixed-integer-routing-with-fixed-costs": {
        "SKILL.md": """# Mixed Integer Routing with Fixed Costs

## Propósito
Resolver el enrutamiento considerando los costos fijos (Gas de interacción de contrato por cada DEX cruzado) usando optimización lineal entera mixta (MILP).

## Conocimiento esencial
Un enrutador continuo puede sugerir dividir un trade en 5 DEXes para ganar 1 centavo en price impact, pero quemar $50 en gas por los saltos adicionales. Este modelo penaliza cada salto con un costo escalón (Step Cost).
"""
    },
    "golden-section-trade-size-optimizer": {
        "SKILL.md": """# Golden Section Trade Size Optimizer

## Propósito
Maximizar el beneficio neto encontrando el volumen de entrada exacto (Optimal Trade Size) usando búsqueda de sección dorada en una función de beneficio convexa univariada.

## Algoritmo
Dado que el profit como función del volumen de entrada `f(x)` es cóncavo (crece hasta un pico y luego cae por el slippage), se evalúan puntos iterativamente usando el radio áureo (`phi = 1.618...`) reduciendo el espacio de búsqueda.
"""
    },
    "ternary-search-trade-size-optimizer": {
        "SKILL.md": """# Ternary Search Trade Size Optimizer

## Propósito
Alternativa algorítmica a Golden Section para encontrar el trade size óptimo en funciones cóncavas puras. Evalúa dos puntos intermedios en cada iteración y descarta 1/3 del espacio de búsqueda. Ideal para optimizaciones en Rust donde las divisiones enteras son eficientes.
"""
    },
    "exact-out-and-exact-in-simulation": {
        "SKILL.md": """# Exact-Out and Exact-In Simulation

## Propósito
Diferenciar y simular transacciones donde el Searcher conoce el capital inicial exacto (Exact-In, ej. WETH) versus transacciones donde el Searcher requiere cumplir un repago exacto (Exact-Out, ej. Flashloans).

## Integración
En el motor EVM, Exact-Out requiere simulaciones inversas para pre-calcular si las reservas cubren el output demandado antes de intentar ejecutar el payload de simulación.
"""
    },
    "mev-share-backrun-searching": {
        "SKILL.md": """# MEV-Share Backrun Searching

## Propósito
Detectar y explotar eventos en redes de orderflow privado (Flashbots MEV-Share) usando hints ofuscados.

## Conocimiento esencial
A diferencia del mempool público, MEV-Share emite eventos SSE (Server-Sent Events) con información parcial (hints como el pair address, o solo un log). El Searcher debe "adivinar" ciegamente la dirección y tamaño de la transacción del usuario para backrunnearla rentablemente, optimizando el bid a ciegas.
"""
    },
    "private-orderflow-risk-and-opportunity-model": {
        "SKILL.md": """# Private Orderflow Risk and Opportunity Model

## Propósito
Modelar el perfil de riesgo-recompensa del orderflow privado. Los flujos privados son de alta calidad pero susceptibles a "Toxic Orderflow" (trampas de builders o pools que alteran estado inesperadamente).

## Señales
- Tasa de win-rate histórico del builder/relay emisor.
- Probabilidad de blind-backrun failed execution (costos de gas incurridos por reverts fallidos en la simulación oculta del builder).
"""
    },
    "searcher-builder-relay-architecture": {
        "SKILL.md": """# Searcher Builder Relay Architecture

## Propósito
Definir la topología de red completa del Proposer-Builder Separation (PBS).

## Arquitectura
1. **Searcher (ArbitrageX)**: Simula bloques, ensambla bundles y puja.
2. **Relay (Flashbots/Ultra Sound)**: Escudo DDoS y custodia de la carga útil. Valida bundles.
3. **Builder (Titan/Beaver)**: Ensambla el bloque ordenando transacciones por fee.
4. **Proposer (Validator)**: Propone el bloque blindado al consenso.
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

print(f"Generated deep content for Batch 1 (Skills 11-20).")
