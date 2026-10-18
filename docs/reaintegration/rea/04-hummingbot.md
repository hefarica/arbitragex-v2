# REA — hummingbot/hummingbot

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: hummingbot/hummingbot · licencia **Apache-2.0** · 20531★ · último push 2026-10-15T01:37:30Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 20.5k★, Apache-2.0, comunidad grande.
- Open source software that helps you create and deploy high-frequency crypto trading bots

## 2. Arquitectura
Strategy V2 framework + connectors; `Clock` y `BudgetChecker`; market-making/arbitraje como controllers.

## 3. Algoritmos
RateOracle, budget checker, order candidate/refresh, arbitraje CEX con inventario.

## 4. Esquemas
`OrderCandidate`, `BudgetCheckResult`, `ConnectorStatus`.

## 5. Resiliencia
Reintentos, control de inventario, kill-switch por pérdida.

## 6. Tests
pytest + mocks de connectors.

## 7. Licencia
`Apache-2.0`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de budget/inventory guard pre-trade y de refresco de cotizaciones — aplicable al prioritization-spine (gate pre-ejecución).
- **Se descarta:** Los connectors y el runtime Python.

## 9. Destino exacto
prioritization-spine (budget/inventory guard), sim-ctl (refresco de candidatos).

## 10. ADR
ADR-006 (candidato): política de refresco de órdenes (cadencia + cancel-replace).

## 11. Gates
G-KILL-01: kill-switch fail-closed <10ms cubre el guard de inventario.

## 12. Riesgo + revert
- **Riesgo:** Sobre-ingeniería de guards; medir latencia. Revert: flag por guard.
