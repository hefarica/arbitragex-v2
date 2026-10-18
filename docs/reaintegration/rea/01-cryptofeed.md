# REA — bmoscon/cryptofeed

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: bmoscon/cryptofeed · licencia **NOASSERTION** · 2923★ · último push 2026-10-17T19:19:13Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 2.9k★, mantenido (push 2026-10-17), asyncio, multi-exchange WS.
- Cryptocurrency Exchange Websocket Data Feed Handler

## 2. Arquitectura
Backend por exchange (feed handler) + `FeedHandler` multiplexor; normalización a un `Book`/`Trade` común; callbacks y backends de salida (Redis, ZMQ, WS, Postgres).

## 3. Algoritmos
Reconstrucción de L2/L3 book con secuencias, chequeo de integridad (`check_integrity`), heartbeats/timeouts, reconexión con backoff.

## 4. Esquemas
Estructuras `OrderBook`, `Trade`, `Ticker`, `Funding`, `OpenInterest`, `Liquidation`; símbolo canónico `BASE-QUOTE`.

## 5. Resiliencia
Watchdog por conexión, reconexión exponencial, detección de gaps de secuencia que fuerza re-snapshot.

## 6. Tests
pytest extenso por backend + fixtures de mensajes grabados.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de normalización multi-exchange + política de integridad de secuencia (gaps => re-snapshot) + taxonomía de mensajes (funding/OI/liquidation).
- **Se descarta:** Python/asyncio y los 30+ conectores concretos (no reescribimos ingesta en Rust aquí; ya existe en searcher-rs).

## 9. Destino exacto
recon (normalización+integridad), token-enricher (funding/OI como campos de enriquecimiento), searcher-rs (detección de gap).

## 10. ADR
ADR-003 (candidato): taxonomía de eventos canónica — único enum de wire.

## 11. Gates
G-SEQ-01 (propuesto): gap de secuencia => None + reason, nunca book parcial (R8).

## 12. Riesgo + revert
- **Riesgo:** Alto acoplamiento si se copia el modelo de datos. Revert: una clase de normalización por PR, feature flag OFF.
