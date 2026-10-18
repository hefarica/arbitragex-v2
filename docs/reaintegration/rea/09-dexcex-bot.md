# REA — ericeric0101/DexCex_bot

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: ericeric0101/DexCex_bot · licencia **NOASSERTION** · 14★ · último push 2025-09-24T07:59:54Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
INMADURO — 14★, sin licencia, multi-chain CEX/DEX (Uniswap/PancakeSwap), directo y triangular.
- A multi-chain CEX/DEX arbitrage bot supporting Uniswap & PancakeSwap with direct and triangular strategies.

## 2. Arquitectura
Script Python: lecturas on-chain + CEX, dos estrategias (directa/triangular).

## 3. Algoritmos
Arbitraje directo y triangular CEX-DEX.

## 4. Esquemas
Sin esquema formal.

## 5. Resiliencia
Nula.

## 6. Tests
Ninguno relevante.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de la distinción estrategia directa vs triangular sobre el mismo motor — útil para el selector.
- **Se descarta:** Todo el código (sin licencia).

## 9. Destino exacto
selector-api (clasificación directa/triangular), math-engine.

## 10. ADR
ADR-011 (candidato): una sola ruta de evaluación con `strategy_kind` como discriminante.

## 11. Gates
G-LICENSE-02 (sin licencia => sólo patrón).

## 12. Riesgo + revert
- **Riesgo:** Baja calidad de fuente => no elevar a doctrina. Revert: descartar el patrón si no aporta campo observable.
