# REA — alloy-rs/alloy

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: alloy-rs/alloy · licencia **Apache-2.0** · 1338★ · último push 2026-10-17T17:26:33Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
ACTIVO — 1338★, Apache-2.0, Rust, push diario.
- Rust implementation of the Ethereum Virtual Machine.

## 2. Arquitectura
Transportes, providers, middlewares y networks para Ethereum en Rust.

## 3. Algoritmos
No algorítmico; infraestructura RPC/tipos.

## 4. Esquemas
Tipos ABI (`U256`, `Address`, `TxEnvelope`), `sol!` macro.

## 5. Resiliencia
Reintentos, fallback de providers, timeouts.

## 6. Tests
Rust unit + integración.

## 7. Licencia
`MIT`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de provider con fallback/timeout y de tipos ABI fuertes (U256) en frontera.
- **Se descarta:** El crate entero (ya se usa `alloy-primitives`; no migrar toda la capa RPC).

## 9. Destino exacto
shared-rs (tipos), recon (provider fallback), searcher-rs.

## 10. ADR
ADR-016 (candidato): frontera ABI tipada — cero `f64` en el borde on-chain.

## 11. Gates
G-TYPE-01 (propuesto): sin `f64` en rutas on-chain (dinero) — sólo U256/decimal.

## 12. Riesgo + revert
- **Riesgo:** Migrar la capa RPC es un cambio grande. Revert: adopción incremental por módulo.
