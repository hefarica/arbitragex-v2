# REA — nkaz001/hftbacktest

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: nkaz001/hftbacktest · licencia **MIT** · 4917★ · último push 2025-12-23T15:57:58Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 4.9k★, MIT; Rust core + Python bindings.
- Free, open source, a high frequency trading and market making backtesting and trading bot, which accounts for limit orders, queue positions, and latencies, utilizing full tick data for trades and order books(Level-2 and Level-3), with real-world crypto trading examples for Binance and Bybit

## 2. Arquitectura
Motor de backtest L2/L3 tick-a-tick con colas de órdenes y latencias; `Bot`/`Feed` traits; estilos de market-making (Avellaneda-Stoikov, grid).

## 3. Algoritmos
Modelo de posición en cola (queue position), latencia de feed/order, maker/taker fees con rebate, A-S quoting.

## 4. Esquemas
`MarketDepth`, `Trade`, `Order`, `State`; datos en formato NPZ/parquet propio.

## 5. Resiliencia
Replay determinista; sin red en el core.

## 6. Tests
Rust unit + ejemplos Python reproducibles.

## 7. Licencia
`MIT`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de cola/latencia y contabilidad maker/taker — la pieza que falta en simulator-v2/sim-core para paridad sim/live.
- **Se descarta:** Los bindings Python y el formato NPZ propietario.

## 9. Destino exacto
sim-core (modelo de cola+latencia), simulator-v2 (fees con rebate), math-engine (A-S quoting).

## 10. ADR
ADR-004 (candidato): modelo de latencia/cola determinista y su semilla.

## 11. Gates
G-PARITY-01: misma lógica sim/live sobre fixture, reloj inyectado (determinismo replay 2× => mismo hash).

## 12. Riesgo + revert
- **Riesgo:** Introducir no-determinismo si el modelo usa RNG sin semilla. Revert: módulo `queue_model` aislado tras flag.
