# ADR-001 — Resolución del Curve duplicado (drift D-03)

- **Estado:** PROPUESTO (P2) — requiere aprobación de Hector antes de tocar `contracts/`.
- **Fecha:** 2026-10-18
- **Contexto de auditoría:** `arbitragex-v2` @ `fe62cdceb6dc0b18afed8db03b7dc7ab64e38d6b` (main, 14-oct-2026)
- **Decisores:** Super Agent (propuesta) · Hector Fabio Riascos Castro (aprobación)
- **Invariantes aplicables:** fusión ADITIVA · zero-mocks · cero unwrap/expect en producción · paridad sim/live · revert de un paso por integración.

## 1. Problema

Existen DOS adaptadores Curve en el árbol de contratos, con ABIs, licencias y estado de integración distintos:

| | `contracts/src/adapters/CurveStableSwapAdapter.sol` | `contracts/src/dexes/CurveAdapter.sol` |
|---|---|---|
| LOC | 78 | 113 |
| SPDX | BUSL-1.1 | MIT |
| Interfaz | contrato suelto, sin `IDEXAdapter` | `is IDEXAdapter` |
| Entrada | `exchangeOnCurve(pool, i, j, amountIn, minOut, recipient)` | vía `extraData = abi.encode(pool, i, j)` |
| Referencias fuera del propio archivo | **0** | `interfaces/IDEXAdapter.sol`, `test/FlashLoanAdapters.t.sol` (`CurveAdapterTest`) |
| Estado | huérfano | testeado, documentado, **no cableado al hot path** (`TODO(M12)` en el propio archivo) |

Ambos cubren el mismo exchange de Curve. Ambiguo cuál es la fuente de verdad → riesgo de doble encoder y de que una ruta de ejecución use un adaptador sin tests.

## 2. Decisión

**Canónico = `contracts/src/dexes/CurveAdapter.sol`.**

Razones (verificables en HEAD `fe62cdce`):
1. **Conformidad de interfaz**: implementa `IDEXAdapter`, el contrato que `ArbitrageExecutor` documenta como la ruta tipada; `CurveStableSwapAdapter` no implementa ninguna interfaz del repo.
2. **Cobertura de tests**: `CurveAdapterTest` en `contracts/test/FlashLoanAdapters.t.sol` ejercita el canónico; el otro tiene cero tests.
3. **Documentado**: `contracts/src/interfaces/IDEXAdapter.sol` lo lista explícitamente; el otro no aparece en ninguna interfaz ni doc.
4. **Riesgo de licencia**: el canónico es MIT (coherente con el resto de `dexes/`); el huérfano es BUSL-1.1.

## 3. Acción (aditiva, un paso, revertible)

1. Anotar `contracts/src/adapters/CurveStableSwapAdapter.sol` como `@deprecated` en su NatSpec, con puntero a este ADR y al canónico. **NO se borra el archivo** (doctrina aditiva: la doctrina y el código no se eliminan en la misma iteración en que se decide).
2. **Capturar su capacidad única** como candidato P1: el huérfano resuelve índices con `coins(i)` on-chain (descubrimiento de índices) y menciona `exchange_underlying` (meta-pools). Eso se pliega al canónico **detrás de la interfaz `IDEXAdapter` existente**, con feature flag default OFF si toca wire (`extraData` no cambia de forma).
3. El cableado al hot path sigue siendo el `TODO(M12)` **del canónico**, no del huérfano: se cierra como una sola integración (una ruta, un encoder).
4. **Gate nuevo `G-CURVE-01`** (propuesto): `grep` de adaptadores Curve activos debe dar exactamente 1 con `is IDEXAdapter`; el huérfano debe estar marcado `@deprecated`. Falla si aparece un segundo adaptador activo.

## 4. Revert

- Un paso: retirar la anotación `@deprecated` (y el gate `G-CURVE-01`) restaura el estado previo exacto. No se tocó ni una línea de lógica.

## 5. Consecuencias

- **Positivas:** una sola fuente de verdad Curve; cero ambigüedad para el encoder; el `TODO(M12)` se cierra una vez.
- **Negativas:** el descubrimiento on-chain de índices (`coins(i)`) queda diferido hasta la integración P3; hasta entonces los índices siguen viniendo por `extraData` (comportamiento actual, sin regresión).
- **Deuda declarada:** `G-CURVE-01` no se ha implementado todavía (P4); la anotación `@deprecated` es el único cambio propuesto en `contracts/` y **no se aplica sin aprobación**.
