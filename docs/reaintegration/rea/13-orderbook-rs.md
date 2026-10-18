# REA — joaquinbejar/OrderBook-rs (canonical for named artio/orderbook-rs)

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: joaquinbejar/OrderBook-rs (canonical for named artio/orderbook-rs) · licencia **MIT** · 545★ · último push 2026-10-13T00:00:00Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
ACTIVO — 545★, MIT, Rust; lock-free price levels. NOTA: la fuente nombrada `artio/orderbook-rs` no existe en GitHub (drift de resolución S-13).


## 2. Arquitectura
Estructuras lock-free de price levels + matching engine; tipos de orden variados.

## 3. Algoritmos
Price-level lock-free, matching con soporte de múltiples tipos de orden, acceso concurrente.

## 4. Esquemas
`PriceLevel`, `OrderBook`, `Order`.

## 5. Resiliencia
Diseñado para concurrencia; sin red.

## 6. Tests
Rust unit + benches.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de price-level lock-free y de concurrencia segura para el libro local.
- **Se descarta:** El matching engine completo (no ejecutamos matching).

## 9. Destino exacto
recon (libro local), shared-rs (estructuras concurrentes).

## 10. ADR
ADR-015 (candidato): estructura de libro local (concurrencia + cero alloc en hot path).

## 11. Gates
G-DET-01 + proptest sobre invariantes del libro (bid<ask, suma consistente).

## 12. Riesgo + revert
- **Riesgo:** Un libro no determinista rompe paridad. Revert: implementación actual conservada tras flag.
