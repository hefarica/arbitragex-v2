# REA — bluealloy/revm

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: bluealloy/revm · licencia **MIT** · 2251★ · último push 2026-10-15T03:52:37Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
ACTIVO — 2251★, MIT, Rust; ya es dependencia (v42 en workspace).
- Rust implementation of the Ethereum Virtual Machine.

## 2. Arquitectura
EVM en Rust: `Database`, `Evm`, `Inspector`; storage overrides.

## 3. Algoritmos
Ejecución EVM determinista con estado inyectable.

## 4. Esquemas
`TxEnv`, `BlockEnv`, `CfgEnv`, `CacheDB`.

## 5. Resiliencia
Determinista por diseño.

## 6. Tests
Suite amplia + fuzzing.

## 7. Licencia
`MIT`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de storage-override para simular estado on-chain sin red (ya usado en sim-core) y de inspector para trazas.
- **Se descarta:** Nada nuevo que importar (ya integrado); sólo afinar uso de inspector/overrides.

## 9. Destino exacto
sim-core, simulator-v2, searcher-rs.

## 10. ADR
ADR-017 (candidato): overrides de estado reproducibles y serializables (fixture de paridad).

## 11. Gates
G-PARITY-01 (fixture de overrides) + determinismo replay 2× => mismo hash.

## 12. Riesgo + revert
- **Riesgo:** Overrides no reproducibles => paridad falsa. Revert: fixture versionado por hash.
