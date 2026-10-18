# ADR-003 — Cobertura de gates de `omega-search` (drift D-04)

- **Estado:** ACEPTADO (P3, 2026-10-18) — ejecutado por el Jefe de equipo de Hector tras la aprobación de Hector de la secuencia P3 (ADR-001/ADR-002 + exclusión por gate).
- **Fecha:** 2026-10-18
- **Contexto de auditoría:** `arbitragex-v2` @ `fe62cdceb6dc0b18afed8db03b7dc7ab64e38d6b` (main, 14-oct-2026)
- **Decisores:** Super Agent (hallazgo D-04) · Jefe de equipo de Hector (decisión de ejecución)
- **Invariantes aplicables:** fusión ADITIVA · zero-mocks · revert de un paso por integración · cero campos sin consumidor.

## 1. Problema

`backend/omega-search` declara su **propio `[workspace]`** en su `Cargo.toml`, así que no es miembro de `backend/Cargo.toml`. Consecuencia: queda **fuera** de los gates obligatorios `cargo fmt --check`, `cargo clippy --workspace --all-targets` y `cargo test --workspace`. Un crate del sistema sin gate obligatorio es un hueco de cobertura silencioso (D-04).

El propio manifiesto del crate documenta que la decisión fue deliberada: la promoción al manifiesto compartido es un cambio de dos líneas, pero `backend/Cargo.toml` está **disputado por ramas `fix/*` vivas**, y su comentario pide explícitamente que la promoción se **reporte como PENDIENTE** en vez de editar en silencio un archivo compartido.

## 2. Opciones evaluadas

| Opción | Efecto | Riesgo |
|---|---|---|
| A. Promover al workspace compartido (2 líneas: `members += "omega-search"` y borrar su `[workspace]`) | entra en los gates `--workspace` | toca un manifiesto disputado por ramas vivas; el cambio no se puede verificar sin compilar el workspace completo; mezcla dos cambios (cobertura + layout) en un paso |
| B. Dejar el crate standalone y **cerrar el hueco con un gate propio** que lo compile y testee por sí mismo | el crate queda cubierto, con evidencia propia | ninguno sobre archivos compartidos; el gate debe declararse como deuda si falta toolchain |
| C. No hacer nada y declararlo deuda | — | deja el hueco abierto; contradice el propósito de P4 |

## 3. Decisión

**Opción B.** El crate permanece standalone (estado explícito, no accidental) y se añade el gate **`G-OMEGA-01`**, que:

1. fija que la exclusión es **deliberada** (declara `[workspace]` propio **y** no figura en `backend/Cargo.toml`; si cualquiera de los dos lados cambia, el gate falla y este ADR debe revisarse);
2. fija la propiedad que hace segura la autonomía: **cero dependencias externas** (`[dependencies]` vacío, sólo `std`) — si alguien añade una dependencia, la afirmación "se compila en segundos sin tocar el lockfile" deja de ser cierta y el gate lo dice;
3. ejecuta `cargo fmt --check` y `cargo test` **dentro del crate** cuando hay toolchain; sin toolchain imprime `DEBT` explícito (nunca un PASS silencioso).

La promoción (opción A) queda **diferida**, no descartada: es un paso propio, con su compilación completa del workspace como verificación, cuando las ramas `fix/*` dejen de disputar `backend/Cargo.toml`.

## 4. Consecuencias

- El hueco de cobertura D-04 se cierra con evidencia reproducible y sin tocar archivos compartidos.
- Queda una **diferencia de layout documentada**: `omega-search` no participa del `resolver` ni del lockfile del workspace. Es intencional y está fijado por el gate.
- Si en el futuro se promueve, el cambio son las dos líneas de la opción A **más** la actualización de este ADR y del gate (que pasará a exigir lo contrario).
