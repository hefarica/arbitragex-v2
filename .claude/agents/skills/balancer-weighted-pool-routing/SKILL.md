# Balancer Weighted Pool Routing

## Propósito
Modelar los pools multidimensionales de Balancer (hasta 8 tokens) donde las reservas tienen pesos asimétricos (ej. 80/20).

## Principios matemáticos
Invariante ponderada:
`prod(R_i ^ W_i) = V`
El cálculo de Spot Price y AmountOut depende de los pesos `W_in` y `W_out`.
`A_out = R_out * (1 - (R_in / (R_in + A_in * (1 - fee))) ^ (W_in / W_out))`
