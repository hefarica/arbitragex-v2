# G-OMEGA-01 — Cobertura de gates de `omega-search` (nuevo, cierra D-04)

- **Tipo:** gate anti-regresión (estático + build standalone).
- **Script:** `scripts/gates/g-omega-01.sh` (exit 0 = PASS, exit 1 = FAIL).
- **Estado en HEAD `fe62cdce` + P3:** **PASS** (fmt + tests ejecutados de verdad, evidencia abajo).
- **ADR:** [ADR-003](../adr/ADR-003-omega-search-workspace.md).

## Qué fija

1. **La exclusión es deliberada:** el crate declara su propio `[workspace]` **y** no figura en `backend/Cargo.toml`. Si cualquiera de los dos lados cambia, el gate falla y el ADR debe revisarse.
2. **La propiedad que hace segura la autonomía sigue en pie:** `[dependencies]` vacío (sólo `std`, cero pull de registry).
3. **El crate se compila y se testea por sí mismo:** `cargo fmt --check` y `cargo test` dentro del crate. Sin toolchain el gate imprime `DEBT` explícito — nunca un PASS silencioso.

## Primer efecto de cerrar el hueco (declarado)

Al ejecutar el gate por primera vez, `cargo fmt --check` **falló**: el crate nunca había pasado por el gate de formato, así que su código (sobre todo los tests) estaba sin formatear. Se aplicó `cargo fmt` al crate. El diff es **sólo formato**: verificable con

```
$ for f in $(git diff --name-only -- backend/omega-search); do
    a=$(git show HEAD:$f | tr -d ' \t\n\r'); b=$(tr -d ' \t\n\r' < $f); [ "$a" = "$b" ] && echo "IDENTICAL $f" || echo "DIFFERS $f"; done
```

cuyo resultado son diferencias **únicamente de comas finales** (que rustfmt añade o quita) más espacios/saltos: cero cambio semántico.

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-omega-01.sh
G-OMEGA-01 — omega-search workspace coverage gate
  PASS  omega-search declares its own [workspace] (standalone is explicit)
  PASS  omega-search is NOT a member of backend/Cargo.toml (exclusion deliberate)
  PASS  [dependencies] is empty (std only, no registry pull)
  PASS  omega-search has no external dependencies
  PASS  cargo fmt --check (omega-search standalone)
  PASS  cargo test (omega-search standalone)

G-OMEGA-01: PASS
```

`cargo test` se ejecutó dentro de `backend/omega-search` (crate `std`-only, sin dependencias externas): el hueco de cobertura queda cerrado con evidencia propia, sin tocar el manifiesto compartido disputado por las ramas `fix/*`.
