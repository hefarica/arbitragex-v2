# REA — stellar-deprecated/kelp

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: stellar-deprecated/kelp · licencia **NOASSERTION** · 1124★ · último push 2023-11-03T05:46:15Z · archived=True
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
DEPRECADO — 1124★, Go, org `stellar-deprecated`, último push 2023. Licencia NOASSERTION (Apache-2.0 según repo).
- Kelp is a free and open-source trading bot for the Stellar DEX and 100+ centralized exchanges

## 2. Arquitectura
Market-maker/bot para Stellar DEX: estrategias en capas (sell/buy walls), configuración declarativa, exchanges internos.

## 3. Algoritmos
Estrategias de cotización por niveles, 'maker' de spreads, configuración de tolerancias.

## 4. Esquemas
Config declarativa (TOML) + estado de ofertas.

## 5. Resiliencia
Reconciliación de ofertas, cancelación/reemplazo, límites de exposición.

## 6. Tests
Go tests.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de cotización por niveles declarativa (config-first) y reconciliación de órdenes abiertas.
- **Se descarta:** Todo el código Go y el dominio Stellar.

## 9. Destino exacto
sim-ctl (reconciliación de órdenes), api-server (config declarativa).

## 10. ADR
ADR-012 (candidato): config-first para parámetros de estrategia (nada hardcodeado).

## 11. Gates
G-CONFIG-01 (propuesto): todo parámetro de estrategia proviene de config tipada, con default explícito.

## 12. Riesgo + revert
- **Riesgo:** Fuente deprecada: no copiar, sólo patrón. Revert: trivial (sólo config).
