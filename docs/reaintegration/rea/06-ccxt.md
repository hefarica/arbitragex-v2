# REA — ccxt/ccxt

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: ccxt/ccxt · licencia **MIT** · 44437★ · último push 2026-10-17T11:53:16Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 44k★, MIT, 100+ venues.
- A unified trading API with more than 100 crypto exchanges and prediction markets in JavaScript / TypeScript / Python / C# / PHP / Go / Java / Rust

## 2. Arquitectura
Unified trading API multi-lenguaje; normalización de símbolos, precisiones, errores.

## 3. Algoritmos
No algorítmico; capa de normalización + rate limiting por venue.

## 4. Esquemas
`Market`, `OrderBook`, `Trade`, `Order`, `Position`; símbolo unificado `BASE/QUOTE`.

## 5. Resiliencia
Rate-limit/backoff, clasificación de errores (NetworkError, ExchangeNotAvailable...), reintentos.

## 6. Tests
Suite por exchange con fixtures.

## 7. Licencia
`MIT`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de clasificación de errores de exchange (retryable vs fatal) y de normalización de precisión/tick.
- **Se descarta:** Los 100+ adaptadores y el runtime JS/Python.

## 9. Destino exacto
shared-rs (clasificación de errores), recon (normalización de símbolos), token-enricher (precisiones).

## 10. ADR
ADR-008 (candidato): taxonomía de errores de venue (retryable/fatal) — única fuente.

## 11. Gates
G-ERR-01 (propuesto): todo error de venue mapea a la taxonomía; sin default silencioso.

## 12. Riesgo + revert
- **Riesgo:** Normalizar de más y perder semántica de venue. Revert: tabla de mapeo por venue, aditiva.
