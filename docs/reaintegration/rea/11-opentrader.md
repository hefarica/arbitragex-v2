# REA — Open-Trader/opentrader

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: Open-Trader/opentrader · licencia **Apache-2.0** · 2890★ · último push 2025-06-29T16:24:33Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
ACTIVO — 2.9k★, Apache-2.0, TS, push 2025-06.
- 🤖 Open-source crypto trading bot | 📈 DCA & GRID strategies | ✨ UI | ⭐ Star to support the project!

## 2. Arquitectura
Monorepo TS: bots, UI, exchange connectors, strategy runner.

## 3. Algoritmos
Estrategias (grid, RSI, etc.) como módulos; backtest y paper trading.

## 4. Esquemas
Tipos TS compartidos por paquete.

## 5. Resiliencia
Reconexión de WS, persistencia de estado.

## 6. Tests
vitest/jest.

## 7. Licencia
`Apache-2.0`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de separación bot/UI/connector en monorepo TS y de paper-trading con el mismo runner.
- **Se descarta:** El código y los conectores.

## 9. Destino exacto
frontend/ (organización), api-server (runner), shared-ts.

## 10. ADR
ADR-013 (candidato): un runner único para paper y live.

## 11. Gates
G-PARITY-01 + tsc --noEmit en frontend/api-server.

## 12. Riesgo + revert
- **Riesgo:** Reorganizar el frontend rompe rutas SSR. Revert: por-paquete, sin mover rutas.
