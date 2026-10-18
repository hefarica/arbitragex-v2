# REA — nautechsystems/nautilus_trader

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: nautechsystems/nautilus_trader · licencia **LGPL-3.0** · 30275★ · último push 2026-10-18T06:25:14Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
REFERENCIA INDUSTRIAL — 30k★, LGPL-3.0, Rust nativo, push diario.


## 2. Arquitectura
Arquitectura event-driven determinista: `DataEngine`, `ExecutionEngine`, `RiskEngine`, `Portfolio`; mensajes tipados por bus; adapters por venue.

## 3. Algoritmos
Órdenes/estado tipados (typestate), reconciliación de posiciones, máquina de estados de orden, reloj inyectable.

## 4. Esquemas
`OrderEvent`, `OrderFilled`, `Position`, `Instrument` con precisión decimal explícita.

## 5. Resiliencia
Idempotencia por `client_order_id`, reconciliación al arranque, circuit-breakers de riesgo.

## 6. Tests
Suite enorme Rust+Python, property tests, harness determinista.

## 7. Licencia
`NOASSERTION`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de motor de ejecución con máquina de estados + reloj inyectable (paridad sim/live) y la separación Risk/Execution/Portfolio.
- **Se descarta:** Todo el framework y su bus (no adoptamos dependencia LGPL-3.0 en producto propietario).

## 9. Destino exacto
sim-ctl (máquina de estados), prioritization-spine (risk pre-trade), shared-rs (reloj inyectable).

## 10. ADR
ADR-005 (candidato): reloj inyectable como frontera sim/live (cierra 'paridad sim/live').

## 11. Gates
G-LICENSE-01 (bloqueante): LGPL-3.0 => sólo PATRÓN, cero código copiado (evidencia: revisión de similitud).

## 12. Riesgo + revert
- **Riesgo:** Contaminación de licencia si se copia código. Revert: reescribir desde cero; el ADR prohíbe copiar líneas.
