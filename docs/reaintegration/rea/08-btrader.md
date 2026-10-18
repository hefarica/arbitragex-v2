# REA — gabriel-milan/btrader

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: gabriel-milan/btrader · licencia **GPL-3.0** · 331★ · último push 2021-05-30T18:29:56Z · archived=True
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
ARCHIVADO — 331★, GPL-3.0, último push 2021.
- Triangle arbitrage trading bot for Binance

## 2. Arquitectura
Arbitraje triangular en Binance (Rust) + notificaciones Telegram.

## 3. Algoritmos
Detección de triángulos (A->B->C->A) sobre book, evaluación de rentabilidad neta de fees.

## 4. Esquemas
`Market`, `Order`; modelo de grafo de pares.

## 5. Resiliencia
Débil; es una demo.

## 6. Tests
Mínimos.

## 7. Licencia
`GPL-3.0`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de búsqueda de ciclos en el grafo de pares (triangular) con coste por arista = fee.
- **Se descarta:** Código (GPL-3.0 + archivado + dependencia de Binance).

## 9. Destino exacto
math-engine (ciclos/triangular), omega-search (grafo de pares).

## 10. ADR
ADR-010 (candidato): enumeración de ciclos con cota de profundidad y poda por fee.

## 11. Gates
G-LICENSE-01 + G-DET-01 (determinismo de la enumeración).

## 12. Riesgo + revert
- **Riesgo:** Explosión combinatoria del grafo. Revert: poda por umbral de rentabilidad tras flag.
