# REA — solidquant/whack-a-mole

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: solidquant/whack-a-mole · licencia **NOASSERTION** · 275★ · último push 2024-04-26T23:07:52Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
DORMIDO — 275★, sin licencia declarada (push 2024-04), CEX-DEX arb puro Python.
- CEX-DEX arbitrage bot written in pure Python

## 2. Arquitectura
Bot CEX-DEX: monitor de CEX + simulación on-chain, cálculo de tamaño, ejecución.

## 3. Algoritmos
Cálculo de tamaño óptimo CEX-DEX con coste de gas/slippage; comparación CEX vs DEX.

## 4. Esquemas
Estructuras de oportunidad CEX-DEX; sin esquema formal.

## 5. Resiliencia
Básica (sin framework de resiliencia).

## 6. Tests
Escasos.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de la fórmula CEX-DEX (precio CEX vs getAmountsOut, coste gas, umbral) y su orden de precedencia on-chain.
- **Se descarta:** Todo el código (sin licencia => no se puede copiar nada, ni siquiera con atribución).

## 9. Destino exacto
math-engine (fórmula CEX-DEX), omega-search (precedencia on-chain).

## 10. ADR
ADR-009 (candidato): precedencia on-chain ABSOLUTA sobre CEX en el scoring.

## 11. Gates
G-LICENSE-02 (bloqueante): SIN LICENCIA => sólo patrón conceptual, cero código.

## 12. Riesgo + revert
- **Riesgo:** Reproducir la fórmula sin verificar unidad/token decimals => valor fabricado. Revert: fórmula tras flag, con test de unidades.
