# REA — freqtrade/freqtrade

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: freqtrade/freqtrade · licencia **GPL-3.0** · 55535★ · último push 2026-10-17T18:36:58Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 55k★, GPL-3.0.
- Free, open source crypto trading bot

## 2. Arquitectura
Bot de trading con exchange abstraction, estrategias, hyperopt, dry-run.

## 3. Algoritmos
Indicadores, stake sizing, protección (stop-loss, trailing, cooldown), dry-run como shadow.

## 4. Esquemas
`Trade`, `PairLock`, `ProtectionReturn`.

## 5. Resiliencia
Persistencia SQLite, protección por cooldown, dry-run vs live con el mismo camino.

## 6. Tests
pytest + regresión con datos grabados.

## 7. Licencia
`GPL-3.0`. **Restricción copyleft / sin licencia: PROHIBIDO copiar código — sólo patrón conceptual.**

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de 'dry-run = shadow con el mismo camino' (paridad) y el modelo de protecciones/cooldown como guard de reentrada.
- **Se descarta:** GPL-3.0: cero código; sólo patrón.

## 9. Destino exacto
sim-ctl (dry-run/shadow), prioritization-spine (cooldown de reentrada).

## 10. ADR
ADR-007 (candidato): cooldown de reentrada post-ejecución.

## 11. Gates
G-LICENSE-01 (bloqueante) + G-PARITY-01.

## 12. Riesgo + revert
- **Riesgo:** GPL-3.0 contaminante si se copia. Revert: reescritura limpia.
