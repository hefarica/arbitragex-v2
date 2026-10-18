# REA — flashbots/simple-arbitrage

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: flashbots/simple-arbitrage · licencia **NOASSERTION** · 2113★ · último push 2024-02-03T18:32:10Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
DEMO — 2.1k★, sin licencia, TS, push 2024-02.
- Example arbitrage bot using Flashbots

## 2. Arquitectura
Script único: detecta arb entre dos DEX, construye bundle Flashbots, envía.

## 3. Algoritmos
Detección de precio entre pares, cálculo de profit, construcción de bundle.

## 4. Esquemas
ABI mínima de router/pair.

## 5. Resiliencia
Nula (demo).

## 6. Tests
Ninguno.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de construcción de bundle/transacción atómica y de la comprobación de profit neto de gas antes de firmar.
- **Se descarta:** Todo el código (sin licencia).

## 9. Destino exacto
relays-client (construcción de bundle — NUNCA firma fuera de relays-client §34.3), math-engine (profit neto).

## 10. ADR
ADR-014 (candidato): toda construcción de bundle pasa por relays-client (único firmante).

## 11. Gates
G-SIGNER-01 (bloqueante): `relays-client` es el ÚNICO módulo con firma; grep de claves privadas fuera de él debe ser 0.

## 12. Riesgo + revert
- **Riesgo:** Duplicar el firmante rompe §34.3. Revert: eliminar cualquier firma fuera de relays-client.
